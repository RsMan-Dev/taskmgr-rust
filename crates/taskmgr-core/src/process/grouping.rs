//! Folding processes into application groups.

use std::collections::{BTreeMap, HashMap};

use super::{ProcessGroup, ProcessRow};
use crate::catalog::Catalog;
use crate::identity::{self, Group, Kind, Role};
use crate::windows::Window;

pub(super) struct Entry {
    pub group: Group,
    pub row: ProcessRow,
    pub ppid: u32,
}

/// Without cgroups, a process belongs to its ancestor started by init: a
/// daemon with its children, a login session, a terminal.
pub(super) fn group_by_tree(entries: &mut [Entry]) {
    let by_pid: HashMap<u32, (u32, String, String)> = entries
        .iter()
        .map(|e| (e.row.pid, (e.ppid, e.row.name.clone(), e.row.user.clone())))
        .collect();
    for e in entries.iter_mut().filter(|e| e.group.key == identity::TREE) {
        let mut top = e.row.pid;
        while let Some((parent, ..)) = by_pid.get(&top) {
            if *parent <= 1 || !by_pid.contains_key(parent) {
                break;
            }
            top = *parent;
        }
        let (_, name, user) = by_pid.get(&top).cloned().unwrap_or_default();
        e.group.key = format!("tree:{top}");
        e.group.name = Some(name);
        e.group.kind = if user == "root" || top == 1 {
            Kind::System
        } else {
            Kind::Background
        };
    }
}

/// A window whose `.desktop` id names another application moves its process
/// and descendants there (a browser started from a terminal). Sandboxes are
/// left alone: everything inside a Flatpak or Wine prefix belongs to it.
pub(super) fn apply_windows(
    entries: &mut [Entry],
    windows: &HashMap<u32, Vec<Window>>,
    catalog: &Catalog,
) {
    let mut moves: Vec<(u32, Group)> = Vec::new();
    for e in entries.iter_mut() {
        let Some(list) = windows.get(&e.row.pid) else {
            continue;
        };
        e.row.windows = list
            .iter()
            .map(|w| w.caption.clone())
            .filter(|c| !c.is_empty())
            .collect();
        let sandboxed = e.group.key.starts_with("steam:")
            || e.group
                .via
                .iter()
                .any(|v| matches!(*v, "Flatpak" | "AppImage" | "Wine" | "Proton"));
        if sandboxed {
            continue;
        }
        let Some((id, desktop)) = list.iter().find_map(|w| {
            let id = if w.desktop.is_empty() {
                &w.class
            } else {
                &w.desktop
            };
            Some((id.clone(), catalog.desktop(id)?.clone()))
        }) else {
            continue;
        };
        let key = format!("app:{id}");
        if !e.group.key.eq_ignore_ascii_case(&key) {
            let group = Group {
                key,
                name: Some(desktop.name),
                icon: desktop.icon,
                kind: Kind::App,
                via: Vec::new(),
            };
            moves.push((e.row.pid, group));
        }
    }
    for (root_pid, group) in moves {
        let Some(origin) = entries
            .iter()
            .find(|e| e.row.pid == root_pid)
            .map(|e| e.group.key.clone())
        else {
            continue;
        };
        let mut subtree = vec![root_pid];
        let mut i = 0;
        while i < subtree.len() {
            let parent = subtree[i];
            subtree.extend(
                entries
                    .iter()
                    .filter(|e| e.ppid == parent && e.group.key == origin)
                    .map(|e| e.row.pid),
            );
            i += 1;
        }
        for e in entries.iter_mut().filter(|e| subtree.contains(&e.row.pid)) {
            e.group = group.clone();
        }
    }
}

/// Sums the rows of each group. With the window list, "application" means
/// "has a window", as on Windows.
pub(super) fn collect(entries: Vec<Entry>, with_windows: bool) -> Vec<ProcessGroup> {
    let mut groups: HashMap<String, ProcessGroup> = HashMap::new();
    for Entry { group, row, .. } in entries {
        let g = groups
            .entry(group.key.clone())
            .or_insert_with(|| ProcessGroup {
                key: group.key.clone(),
                name: group.name.clone().unwrap_or_default(),
                icon: group.icon.clone(),
                kind: group.kind,
                via: Vec::new(),
                cpu: 0.0,
                memory: 0,
                disk_bps: None,
                windows: Vec::new(),
                net: None,
                gpu: BTreeMap::new(),
                gpu_memory: BTreeMap::new(),
                processes: Vec::new(),
            });
        for v in &group.via {
            if !g.via.contains(v) {
                g.via.push(v);
            }
        }
        g.cpu += row.cpu;
        g.memory += row.memory;
        if let Some(d) = row.disk_bps {
            g.disk_bps = Some(g.disk_bps.unwrap_or(0.0) + d);
        }
        g.windows.extend(row.windows.iter().cloned());
        if let Some((rx, tx)) = row.net {
            let (grx, gtx) = g.net.get_or_insert((0.0, 0.0));
            *grx += rx;
            *gtx += tx;
        }
        for (i, u) in &row.gpu {
            *g.gpu.entry(*i).or_default() += u;
        }
        for (i, m) in &row.gpu_memory {
            *g.gpu_memory.entry(*i).or_default() += m;
        }
        g.processes.push(row);
    }

    let mut out: Vec<ProcessGroup> = groups.into_values().collect();
    for g in &mut out {
        if g.name.is_empty() {
            g.name = g
                .processes
                .iter()
                .filter(|p| p.role == Role::Normal)
                .max_by_key(|p| p.memory)
                .map_or_else(|| "Application".to_owned(), |p| p.name.clone());
        }
        g.processes.sort_by_key(|p| p.pid);
        if g.key.starts_with("system:docker-") {
            name_container(g);
        }
        if with_windows {
            g.kind = match (g.kind, g.windows.is_empty()) {
                (Kind::App, true) => Kind::Background,
                (_, false) => Kind::App,
                (k, true) => k,
            };
        }
    }
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

/// Init shims (docker-init, tini) only start the real program.
fn name_container(g: &mut ProcessGroup) {
    const SHIMS: [&str; 4] = ["docker-init", "tini", "catatonit", "dumb-init"];
    let pids: Vec<u32> = g.processes.iter().map(|p| p.pid).collect();
    let root = g.processes.iter().find(|p| !pids.contains(&p.ppid));
    let main = root.and_then(|r| {
        if SHIMS.contains(&r.name.as_str()) {
            g.processes.iter().find(|p| p.ppid == r.pid).or(Some(r))
        } else {
            Some(r)
        }
    });
    if let Some(main) = main {
        g.name = format!("Docker container ({})", main.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(pid: u32, ppid: u32, key: &str, via: &[&'static str]) -> Entry {
        Entry {
            group: Group {
                key: key.into(),
                name: Some(key.into()),
                icon: None,
                kind: Kind::App,
                via: via.to_vec(),
            },
            row: ProcessRow {
                pid,
                name: String::new(),
                role: Role::Normal,
                cpu: 0.0,
                memory: 0,
                disk_bps: None,
                threads: 1,
                command: String::new(),
                net: None,
                ppid,
                start: 0,
                user: String::new(),
                state: 'S',
                nice: 0,
                exe: None,
                windows: Vec::new(),
                gpu: BTreeMap::new(),
                gpu_memory: BTreeMap::new(),
            },
            ppid,
        }
    }

    fn window(pid: u32, desktop: &str, class: &str) -> (u32, Vec<Window>) {
        (
            pid,
            vec![Window {
                pid,
                caption: "t".into(),
                class: class.into(),
                desktop: desktop.into(),
            }],
        )
    }

    #[test]
    fn tree_groups_without_cgroups() {
        let mut entries = vec![
            entry(1, 0, identity::TREE, &[]),
            entry(100, 1, identity::TREE, &[]),
            entry(101, 100, identity::TREE, &[]),
            entry(102, 101, identity::TREE, &[]),
            entry(200, 1, identity::TREE, &[]),
        ];
        for (e, name) in entries
            .iter_mut()
            .zip(["init", "sshd", "sshd", "ash", "chronyd"])
        {
            e.row.name = name.into();
            e.row.user = "root".into();
        }
        group_by_tree(&mut entries);
        let keys: Vec<&str> = entries.iter().map(|e| e.group.key.as_str()).collect();
        assert_eq!(
            keys,
            ["tree:1", "tree:100", "tree:100", "tree:100", "tree:200"]
        );
        assert_eq!(entries[3].group.name.as_deref(), Some("sshd"));
    }

    #[test]
    fn windows_split_apps_out_of_terminals_but_not_sandboxes() {
        let catalog = Catalog::with_desktop(&[
            ("firefox", "Firefox", "firefox"),
            ("org.kde.konsole", "Konsole", "k"),
        ]);
        let mut entries = vec![
            entry(10, 1, "app:org.kde.konsole", &[]),
            entry(11, 10, "app:org.kde.konsole", &[]),
            entry(12, 11, "app:org.kde.konsole", &[]),
            entry(13, 12, "app:org.kde.konsole", &[]),
            entry(20, 1, "app:org.vinegarhq.Vinegar", &["Flatpak", "Wine"]),
        ];
        let windows = HashMap::from([
            window(10, "org.kde.konsole", "org.kde.konsole"),
            window(12, "firefox", "firefox"),
            window(20, "", "steam_proton"),
        ]);
        apply_windows(&mut entries, &windows, &catalog);
        let keys: Vec<&str> = entries.iter().map(|e| e.group.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "app:org.kde.konsole",
                "app:org.kde.konsole",
                "app:firefox",
                "app:firefox",
                "app:org.vinegarhq.Vinegar"
            ]
        );
        assert_eq!(entries[2].group.name.as_deref(), Some("Firefox"));
        assert_eq!(entries[4].row.windows, vec!["t"]);
    }
}

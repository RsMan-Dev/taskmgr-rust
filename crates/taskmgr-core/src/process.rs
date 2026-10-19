use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::catalog::Catalog;
use crate::gpu::GpuSnapshot;
use crate::identity::{self, ENV_KEYS, Group, Identity, Kind, ProcFacts, Role};
use crate::util::Root;
use crate::windows::Window;

#[derive(Clone, Debug, Serialize)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub role: Role,
    pub cpu: f64,
    pub memory: u64,
    pub disk_bps: Option<f64>,
    pub threads: u64,
    pub command: String,
    pub ppid: u32,
    pub user: String,
    /// `R` running, `S` sleeping, `D` waiting on I/O, `T` stopped, `Z` zombie.
    pub state: char,
    pub nice: i32,
    pub exe: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<String>,
    /// Received and sent bytes per second; only while the root helper runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<(f64, f64)>,
    /// Keyed by GPU index, dedicated GPUs only.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub gpu: BTreeMap<usize, f64>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub gpu_memory: BTreeMap<usize, u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcessGroup {
    pub key: String,
    pub name: String,
    pub icon: Option<String>,
    pub kind: Kind,
    pub via: Vec<&'static str>,
    pub cpu: f64,
    pub memory: u64,
    pub disk_bps: Option<f64>,
    pub windows: Vec<String>,
    pub net: Option<(f64, f64)>,
    pub gpu: BTreeMap<usize, f64>,
    pub gpu_memory: BTreeMap<usize, u64>,
    pub processes: Vec<ProcessRow>,
}

struct Stat {
    comm: String,
    state: char,
    nice: i32,
    ppid: u32,
    ticks: u64,
    threads: u64,
    start: u64,
}

fn parse_stat(text: &str) -> Option<Stat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let f: Vec<&str> = text[close + 1..].split_whitespace().collect();
    let n = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok());
    Some(Stat {
        comm: text[open + 1..close].to_owned(),
        state: f.first()?.chars().next()?,
        nice: f.get(16)?.parse().ok()?,
        ppid: n(1)? as u32,
        ticks: n(11)? + n(12)?,
        threads: n(17)?,
        start: n(19)?,
    })
}

fn status_kb(status: &str, key: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|l| l.strip_prefix(key))
        .and_then(|v| {
            v.trim_start_matches(':')
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn io_bytes(io: &str) -> Option<u64> {
    let get = |k: &str| {
        io.lines()
            .find_map(|l| l.strip_prefix(k)?.trim().parse::<u64>().ok())
    };
    Some(get("read_bytes:")? + get("write_bytes:")?)
}

fn split_nul(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

struct Prev {
    start: u64,
    ticks: u64,
    io: Option<u64>,
}

/// uid to login name, from /etc/passwd (cached; NSS users fall back to the uid).
#[derive(Default)]
struct Users(HashMap<u32, String>);

impl Users {
    fn name(&mut self, root: &Root, uid: u32) -> String {
        if self.0.is_empty() {
            for line in root.read("/etc/passwd").unwrap_or_default().lines() {
                let f: Vec<&str> = line.split(':').collect();
                if let (Some(name), Some(Ok(id))) = (f.first(), f.get(2).map(|s| s.parse::<u32>()))
                {
                    self.0.insert(id, (*name).to_owned());
                }
            }
        }
        self.0.get(&uid).cloned().unwrap_or_else(|| uid.to_string())
    }
}

pub struct ProcessSampler {
    users: Users,
    catalog: Catalog,
    prev: HashMap<u32, Prev>,
    identities: HashMap<(u32, u64, String), Identity>,
    tick_hz: f64,
    cpus: f64,
}

impl ProcessSampler {
    pub fn new(root: &Root) -> Self {
        // SAFETY: sysconf has no preconditions.
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        Self {
            users: Users::default(),
            catalog: Catalog::load(root),
            prev: HashMap::new(),
            identities: HashMap::new(),
            tick_hz: if hz > 0 { hz as f64 } else { 100.0 },
            cpus: std::thread::available_parallelism().map_or(1, |n| n.get()) as f64,
        }
    }

    fn facts(root: &Root, pid: u32, comm: &str) -> ProcFacts {
        let dir = format!("/proc/{pid}");
        let env = std::fs::read(root.path(format!("{dir}/environ")))
            .map(|b| {
                split_nul(&b)
                    .into_iter()
                    .filter_map(|kv| {
                        let (k, v) = kv.split_once('=')?;
                        ENV_KEYS.contains(&k).then(|| (k.to_owned(), v.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        ProcFacts {
            comm: comm.to_owned(),
            cmdline: std::fs::read(root.path(format!("{dir}/cmdline")))
                .map(|b| split_nul(&b))
                .unwrap_or_default(),
            exe: std::fs::read_link(root.path(format!("{dir}/exe")))
                .ok()
                .map(|p| p.to_string_lossy().into_owned()),
            cgroup: root
                .read(format!("{dir}/cgroup"))
                .and_then(|c| {
                    c.lines()
                        .find_map(|l| l.strip_prefix("0::").map(str::to_owned))
                })
                .unwrap_or_default(),
            env,
        }
    }

    pub fn sample(
        &mut self,
        root: &Root,
        secs: f64,
        windows: Option<&HashMap<u32, Vec<Window>>>,
        gpus: &[GpuSnapshot],
        net: Option<&HashMap<u32, (f64, f64)>>,
    ) -> Vec<ProcessGroup> {
        let mut gpu_use: HashMap<u32, Vec<(usize, f64, Option<u64>)>> = HashMap::new();
        for g in gpus.iter().filter(|g| g.dedicated) {
            for p in &g.processes {
                gpu_use
                    .entry(p.pid)
                    .or_default()
                    .push((g.index, p.usage, p.memory));
            }
        }
        let mut entries: Vec<Entry> = Vec::new();
        let mut seen = HashMap::new();
        for entry in root.list("/proc") {
            let Ok(pid) = entry.parse::<u32>() else {
                continue;
            };
            let Some(stat) = root
                .read(format!("/proc/{pid}/stat"))
                .and_then(|s| parse_stat(&s))
            else {
                continue;
            };
            if pid == 2 || stat.ppid == 2 {
                continue;
            }
            let status = root.read(format!("/proc/{pid}/status")).unwrap_or_default();
            let io = root
                .read(format!("/proc/{pid}/io"))
                .and_then(|s| io_bytes(&s));
            let (cpu, disk_bps) = match self
                .prev
                .get(&pid)
                .filter(|p| p.start == stat.start && secs > 0.0)
            {
                Some(p) => (
                    stat.ticks.saturating_sub(p.ticks) as f64 / self.tick_hz / secs / self.cpus
                        * 100.0,
                    io.zip(p.io).map(|(c, o)| c.saturating_sub(o) as f64 / secs),
                ),
                None => (0.0, io.map(|_| 0.0)),
            };
            seen.insert(
                pid,
                Prev {
                    start: stat.start,
                    ticks: stat.ticks,
                    io,
                },
            );

            let key = (pid, stat.start, stat.comm.clone());
            let id = match self.identities.get(&key) {
                Some(id) => id.clone(),
                None => {
                    let facts = Self::facts(root, pid, &stat.comm);
                    if facts.cmdline.is_empty() && facts.cgroup.is_empty() {
                        continue;
                    }
                    let id = identity::resolve(&facts, &mut self.catalog, root);
                    self.identities.insert(key, id.clone());
                    id
                }
            };
            let command = std::fs::read(root.path(format!("/proc/{pid}/cmdline")))
                .map(|b| split_nul(&b).join(" "))
                .unwrap_or_default();
            let row = ProcessRow {
                pid,
                name: id.name.clone(),
                role: id.role,
                cpu,
                memory: (status_kb(&status, "RssAnon").unwrap_or(0)
                    + status_kb(&status, "RssShmem").unwrap_or(0))
                    * 1024,
                disk_bps,
                threads: stat.threads,
                command: command.chars().take(400).collect(),
                ppid: stat.ppid,
                user: status_kb(&status, "Uid")
                    .map(|uid| self.users.name(root, uid as u32))
                    .unwrap_or_default(),
                state: stat.state,
                nice: stat.nice,
                exe: std::fs::read_link(root.path(format!("/proc/{pid}/exe")))
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned()),
                windows: Vec::new(),
                net: net.map(|n| n.get(&pid).copied().unwrap_or((0.0, 0.0))),
                gpu: gpu_use
                    .get(&pid)
                    .map(|v| v.iter().map(|(i, u, _)| (*i, *u)).collect())
                    .unwrap_or_default(),
                gpu_memory: gpu_use
                    .get(&pid)
                    .map(|v| v.iter().filter_map(|(i, _, m)| Some((*i, (*m)?))).collect())
                    .unwrap_or_default(),
            };
            entries.push(Entry {
                group: id.group,
                row,
                ppid: stat.ppid,
            });
        }
        self.identities
            .retain(|(pid, start, _), _| seen.get(pid).is_some_and(|p| p.start == *start));
        self.prev = seen;
        if let Some(windows) = windows {
            apply_windows(&mut entries, windows, &self.catalog);
        }

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
                let pids: Vec<u32> = g.processes.iter().map(|p| p.pid).collect();
                if let Some(main) = g.processes.iter().find(|p| !pids.contains(&p.ppid)) {
                    g.name = format!("Docker container ({})", main.name);
                }
            }
            // Like Windows: an application is something with a window.
            if windows.is_some() {
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
}

struct Entry {
    group: Group,
    row: ProcessRow,
    ppid: u32,
}

/// A window whose `.desktop` id names another application moves its process
/// and descendants there (a browser started from a terminal). Sandboxes are
/// left alone: everything inside a Flatpak or Wine prefix belongs to it.
fn apply_windows(entries: &mut [Entry], windows: &HashMap<u32, Vec<Window>>, catalog: &Catalog) {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Terminate,
    Kill,
    Suspend,
    Resume,
}

impl Signal {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "term" => Self::Terminate,
            "kill" => Self::Kill,
            "stop" => Self::Suspend,
            "cont" => Self::Resume,
            _ => return None,
        })
    }

    fn number(self) -> i32 {
        match self {
            Self::Terminate => libc::SIGTERM,
            Self::Kill => libc::SIGKILL,
            Self::Suspend => libc::SIGSTOP,
            Self::Resume => libc::SIGCONT,
        }
    }
}

pub fn terminate(pids: &[i32], force: bool) -> bool {
    send(
        pids,
        if force {
            Signal::Kill
        } else {
            Signal::Terminate
        },
    )
}

/// Signals every pid. Returns false if any of them was denied; processes
/// that already exited are not an error.
pub fn send(pids: &[i32], signal: Signal) -> bool {
    let signal = signal.number();
    let mut ok = true;
    for &pid in pids.iter().filter(|p| **p > 0) {
        // SAFETY: kill has no memory preconditions.
        if unsafe { libc::kill(pid, signal) } != 0 {
            ok &= std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        }
    }
    ok
}

#[derive(Debug, PartialEq, Eq)]
pub enum PriorityError {
    /// Raising priority (lower nice) needs root.
    Denied,
    Gone,
    Failed(String),
}

/// Sets the nice value (-20 highest to 19 lowest).
pub fn set_priority(pid: i32, nice: i32) -> Result<(), PriorityError> {
    // SAFETY: setpriority has no memory preconditions.
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, nice) } == 0 {
        return Ok(());
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => Err(PriorityError::Denied),
        Some(libc::ESRCH) => Err(PriorityError::Gone),
        _ => Err(PriorityError::Failed(
            std::io::Error::last_os_error().to_string(),
        )),
    }
}

/// Same through pkexec and renice, for priorities only root may set.
/// Blocks until the password dialog is answered.
pub fn set_priority_as_admin(pid: i32, nice: i32) -> Result<(), String> {
    let pkexec =
        crate::energy::find_program("pkexec", &["/run/wrappers/bin/pkexec", "/usr/bin/pkexec"])
            .ok_or("pkexec not found (polkit package).")?;
    let renice = crate::energy::find_program(
        "renice",
        &[
            "/run/current-system/sw/bin/renice",
            "/usr/bin/renice",
            "/bin/renice",
        ],
    )
    .ok_or("renice not found.")?;
    let status = std::process::Command::new(pkexec)
        .args([
            renice.as_str(),
            "-n",
            &nice.to_string(),
            "-p",
            &pid.to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    match status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) => Err("Authorization denied or cancelled.".to_owned()),
        _ => Err(format!("The command failed ({status}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspend_resume_and_priority_on_own_process() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        let state = || {
            let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
            parse_stat(&s).unwrap()
        };
        assert!(send(&[pid], Signal::Suspend));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(state().state, 'T');
        assert!(send(&[pid], Signal::Resume));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_ne!(state().state, 'T');
        assert_eq!(set_priority(pid, 5), Ok(()));
        assert_eq!(state().nice, 5);
        assert!(send(&[pid], Signal::Kill));
        let _ = child.wait();
    }

    #[test]
    fn terminate_kills_own_process_and_reports_denial() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        assert!(terminate(&[child.id() as i32], false));
        assert!(!child.wait().unwrap().success());
        // In a build sandbox pid 1 may be ours: only check denial when it is not.
        let uid = |pid: &str| {
            std::fs::read_to_string(format!("/proc/{pid}/status"))
                .ok()
                .and_then(|s| status_kb(&s, "Uid"))
        };
        if uid("1").is_some() && uid("1") != uid("self") {
            assert!(!terminate(&[1], false));
        }
        assert!(terminate(&[child.id() as i32], true));
    }

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

    #[test]
    fn stat_with_spaces_in_comm() {
        let s = parse_stat(
            "42 (Web Content) S 1 42 42 0 -1 4194560 1 0 0 0 30 12 0 0 20 0 7 0 555 0 0",
        )
        .unwrap();
        assert_eq!(s.comm, "Web Content");
        assert_eq!((s.ppid, s.ticks, s.threads, s.start), (1, 42, 7, 555));
    }

    #[test]
    fn io_and_status() {
        assert_eq!(
            io_bytes("rchar: 9\nread_bytes: 4096\nwrite_bytes: 512\n"),
            Some(4608)
        );
        assert_eq!(status_kb("RssAnon:\t  2048 kB\n", "RssAnon"), Some(2048));
    }
}

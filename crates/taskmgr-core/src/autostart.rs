//! Applications started at login (XDG autostart). A system entry is turned
//! off by a user copy with `Hidden=true`, as the specification prescribes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::util::Root;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AutostartEntry {
    /// File name, the entry's identity across directories.
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub command: String,
    pub enabled: bool,
    /// False when OnlyShowIn/NotShowIn exclude the current desktop.
    pub applies: bool,
    pub system: bool,
    pub path: String,
}

fn user_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"))
        .join("autostart")
}

fn system_dirs() -> Vec<String> {
    let dirs = std::env::var("XDG_CONFIG_DIRS").unwrap_or_default();
    let mut out: Vec<String> = dirs
        .split(':')
        .filter(|d| !d.is_empty())
        .map(|d| format!("{d}/autostart"))
        .collect();
    if out.is_empty() {
        out.push("/etc/xdg/autostart".to_owned());
    }
    out
}

fn keys(text: &str) -> BTreeMap<String, String> {
    let mut in_entry = false;
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry && let Some((k, v)) = line.split_once('=') {
            map.entry(k.trim().to_owned())
                .or_insert_with(|| v.trim().to_owned());
        }
    }
    map
}

fn applies_to(desktops: &[String], k: &BTreeMap<String, String>) -> bool {
    let list = |key: &str| {
        k.get(key).map(|v| {
            v.split(';')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
    };
    if let Some(only) = list("OnlyShowIn") {
        return only.iter().any(|d| desktops.contains(d));
    }
    if let Some(not) = list("NotShowIn") {
        return !not.iter().any(|d| desktops.contains(d));
    }
    true
}

fn entry(
    id: &str,
    path: &str,
    text: &str,
    system: bool,
    desktops: &[String],
    locales: &[String],
) -> AutostartEntry {
    let k = keys(text);
    let name = locales
        .iter()
        .find_map(|l| k.get(&format!("Name[{l}]")))
        .or(k.get("Name"))
        .cloned()
        .unwrap_or_else(|| id.trim_end_matches(".desktop").to_owned());
    let hidden = k.get("Hidden").is_some_and(|v| v == "true");
    let gnome_off = k
        .get("X-GNOME-Autostart-enabled")
        .is_some_and(|v| v == "false");
    AutostartEntry {
        id: id.to_owned(),
        name,
        icon: k.get("Icon").cloned().filter(|s| !s.is_empty()),
        command: k.get("Exec").cloned().unwrap_or_default(),
        enabled: !hidden && !gnome_off,
        applies: applies_to(desktops, &k),
        system,
        path: path.to_owned(),
    }
}

pub fn entries(root: &Root) -> Vec<AutostartEntry> {
    let desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .map(str::to_owned)
        .collect();
    let locales = crate::catalog::locales();
    let mut by_id: BTreeMap<String, AutostartEntry> = BTreeMap::new();
    for dir in system_dirs().iter().rev() {
        for f in root
            .list(dir)
            .into_iter()
            .filter(|f| f.ends_with(".desktop"))
        {
            let path = format!("{dir}/{f}");
            if let Some(text) = root.read(&path) {
                by_id.insert(
                    f.clone(),
                    entry(&f, &path, &text, true, &desktops, &locales),
                );
            }
        }
    }
    let user = user_dir().to_string_lossy().into_owned();
    for f in root
        .list(&user)
        .into_iter()
        .filter(|f| f.ends_with(".desktop"))
    {
        let path = format!("{user}/{f}");
        let Some(text) = root.read(&path) else {
            continue;
        };
        let mut e = entry(&f, &path, &text, false, &desktops, &locales);
        // A user override of a system entry keeps the system name and icon
        // when it only carries Hidden=.
        if let Some(sys) = by_id.get(&f) {
            e.system = true;
            if !keys(&text).contains_key("Name") {
                e.name = sys.name.clone();
                e.icon = e.icon.or(sys.icon.clone());
                e.command = sys.command.clone();
            }
        }
        by_id.insert(f, e);
    }
    let mut list: Vec<AutostartEntry> = by_id.into_values().collect();
    list.sort_by_key(|e| e.name.to_lowercase());
    list
}

/// Rewrites or creates the user file so the entry is (not) started at login.
pub fn set_enabled(root: &Root, id: &str, enabled: bool) -> std::io::Result<()> {
    if id.contains('/') || !id.ends_with(".desktop") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "bad id",
        ));
    }
    let dir = user_dir();
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(id);
    let text = match std::fs::read_to_string(&target) {
        Ok(t) => t,
        Err(_) => system_dirs()
            .iter()
            .find_map(|d| root.read(format!("{d}/{id}")))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "unknown entry"))?,
    };
    let mut out = String::new();
    let mut in_entry = false;
    let mut written = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if in_entry && !written {
                out.push_str(&format!("Hidden={}\n", !enabled));
                written = true;
            }
            in_entry = t == "[Desktop Entry]";
        }
        if in_entry && (t.starts_with("Hidden=") || t.starts_with("X-GNOME-Autostart-enabled=")) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !written {
        out.push_str(&format!("Hidden={}\n", !enabled));
    }
    std::fs::write(target, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_a_system_entry_writes_a_user_override() {
        let base = std::env::temp_dir().join(format!("taskmgr-autostart-{}", std::process::id()));
        let (system, user) = (base.join("xdg"), base.join("config"));
        std::fs::create_dir_all(system.join("autostart")).unwrap();
        std::fs::write(
            system.join("autostart/demo.desktop"),
            "[Desktop Entry]\nName=Demo\nExec=demo\n",
        )
        .unwrap();
        // SAFETY: no other test reads these variables.
        unsafe {
            std::env::set_var("XDG_CONFIG_DIRS", &system);
            std::env::set_var("XDG_CONFIG_HOME", &user);
        }
        let root = Root::system();
        let find = || {
            entries(&root)
                .into_iter()
                .find(|e| e.id == "demo.desktop")
                .unwrap()
        };
        assert!(find().enabled);
        set_enabled(&root, "demo.desktop", false).unwrap();
        let off = find();
        assert!(!off.enabled && off.system);
        assert_eq!(off.name, "Demo");
        set_enabled(&root, "demo.desktop", true).unwrap();
        assert!(find().enabled);
        assert!(set_enabled(&root, "../x.desktop", true).is_err());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn hidden_and_desktop_filters() {
        let kde = vec!["KDE".to_owned()];
        let e = entry(
            "a.desktop",
            "/x",
            "[Desktop Entry]\nName=A\nExec=a\nOnlyShowIn=GNOME;\n",
            true,
            &kde,
            &[],
        );
        assert!(e.enabled && !e.applies);
        let e = entry(
            "b.desktop",
            "/x",
            "[Desktop Entry]\nName=B\nHidden=true\nNotShowIn=XFCE;\n",
            false,
            &kde,
            &[],
        );
        assert!(!e.enabled && e.applies);
        let e = entry(
            "c.desktop",
            "/x",
            "[Desktop Entry]\nName=C\nX-GNOME-Autostart-enabled=false\n",
            false,
            &kde,
            &[],
        );
        assert!(!e.enabled);
    }
}

//! Names and icons for applications, systemd units and Steam games.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::util::Root;

mod exec;

use exec::Launch;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Default)]
pub struct Catalog {
    desktop: HashMap<String, Entry>,
    desktop_lower: HashMap<String, String>,
    user_unit_dirs: Vec<String>,
    system_unit_dirs: Vec<String>,
    units: HashMap<(bool, String), Option<String>>,
    steam_libraries: Vec<String>,
    icon_dirs: Vec<String>,
    steam: HashMap<String, Option<String>>,
    /// Desktop entries by the file name of the program they start.
    launches: HashMap<String, Vec<Launch>>,
}

pub(crate) fn locales() -> Vec<String> {
    let lang = std::env::var("LC_MESSAGES")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    let full = lang.split(['.', '@']).next().unwrap_or("").to_owned();
    let short = full.split('_').next().unwrap_or("").to_owned();
    [full, short]
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect()
}

/// The entry, and its `Exec=` line when it is a visible graphical one.
fn parse_desktop(text: &str, locales: &[String]) -> Option<(Entry, Option<String>)> {
    let mut in_entry = false;
    let mut keys: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry && let Some((k, v)) = line.split_once('=') {
            keys.entry(k.trim()).or_insert(v.trim());
        }
    }
    let name = locales
        .iter()
        .find_map(|l| keys.get(format!("Name[{l}]").as_str()))
        .or(keys.get("Name"))?;
    let shown = ["NoDisplay", "Hidden", "Terminal"]
        .iter()
        .all(|k| keys.get(k) != Some(&"true"));
    let entry = Entry {
        name: (*name).to_owned(),
        icon: keys
            .get("Icon")
            .map(|s| (*s).to_owned())
            .filter(|s| !s.is_empty()),
    };
    Some((
        entry,
        keys.get("Exec").filter(|_| shown).map(|s| (*s).to_owned()),
    ))
}

impl Catalog {
    pub fn load(root: &Root) -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let user = std::env::var("USER").unwrap_or_default();
        let mut data_dirs =
            vec![std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"))];
        data_dirs.extend(
            std::env::var("XDG_DATA_DIRS")
                .unwrap_or_default()
                .split(':')
                .filter(|d| !d.is_empty())
                .map(str::to_owned),
        );
        data_dirs.extend([
            format!("{home}/.local/share/flatpak/exports/share"),
            "/var/lib/flatpak/exports/share".to_owned(),
            "/var/lib/snapd/desktop".to_owned(),
            format!("/etc/profiles/per-user/{user}/share"),
            "/run/current-system/sw/share".to_owned(),
            "/usr/local/share".to_owned(),
            "/usr/share".to_owned(),
        ]);
        let mut c = Catalog::default();
        let locales = locales();
        for dir in data_dirs.iter().rev() {
            c.scan_applications(root, &format!("{dir}/applications"), "", &locales);
        }
        let unit_dirs = |kind: &str| -> Vec<String> {
            let mut dirs = vec![
                format!("{home}/.config/systemd/{kind}"),
                format!("/etc/systemd/{kind}"),
                format!("/run/systemd/{kind}"),
            ];
            dirs.extend(data_dirs.iter().map(|d| format!("{d}/systemd/{kind}")));
            dirs.push(format!("/usr/lib/systemd/{kind}"));
            dirs
        };
        c.user_unit_dirs = unit_dirs("user");
        c.system_unit_dirs = unit_dirs("system");
        c.steam_libraries = steam_libraries(root, &home);
        c.icon_dirs = data_dirs
            .iter()
            .map(|d| format!("{d}/icons/hicolor"))
            .collect();
        c
    }

    fn scan_applications(&mut self, root: &Root, dir: &str, prefix: &str, locales: &[String]) {
        for name in root.list(dir) {
            let path = format!("{dir}/{name}");
            if let Some(id) = name.strip_suffix(".desktop") {
                if let Some((e, exec)) = root.read(&path).and_then(|t| parse_desktop(&t, locales)) {
                    let id = format!("{prefix}{id}");
                    for list in self.launches.values_mut() {
                        list.retain(|l| l.id != id);
                    }
                    if let Some(l) = exec.and_then(|x| exec::launch(&id, &x)) {
                        self.launches.entry(l.program.clone()).or_default().push(l);
                    }
                    self.desktop_lower.insert(id.to_lowercase(), id.clone());
                    self.desktop.insert(id, e);
                }
            } else if root.path(&path).is_dir() {
                self.scan_applications(root, &path, &format!("{prefix}{name}-"), locales);
            }
        }
    }

    #[cfg(test)]
    pub fn with_desktop(entries: &[(&str, &str, &str)]) -> Self {
        let mut c = Catalog::default();
        for (id, name, icon) in entries {
            c.desktop_lower.insert(id.to_lowercase(), (*id).to_owned());
            c.desktop.insert(
                (*id).to_owned(),
                Entry {
                    name: (*name).into(),
                    icon: Some((*icon).into()),
                },
            );
        }
        c
    }

    #[cfg(test)]
    pub fn with_launch(mut self, id: &str, exec: &str) -> Self {
        if let Some(l) = exec::launch(id, exec) {
            self.launches.entry(l.program.clone()).or_default().push(l);
        }
        self
    }

    /// Desktop entry id of the application this program was started as.
    pub fn launched_as(&self, exe: Option<&str>, cmdline: &[String]) -> Option<String> {
        let mut names: Vec<String> = exe.into_iter().map(exec::program_name).collect();
        if let Some(arg0) = cmdline.first().map(|a| exec::program_name(a))
            && !names.contains(&arg0)
        {
            names.push(arg0);
        }
        names.iter().find_map(|n| {
            let id = exec::best(self.launches.get(n)?, cmdline)?;
            self.desktop(id).is_some().then(|| id.to_owned())
        })
    }

    pub fn desktop(&self, id: &str) -> Option<&Entry> {
        self.desktop.get(id).or_else(|| {
            self.desktop
                .get(self.desktop_lower.get(&id.to_lowercase())?)
        })
    }

    pub fn unit_description(&mut self, root: &Root, unit: &str, user: bool) -> Option<String> {
        let key = (user, unit.to_owned());
        if let Some(d) = self.units.get(&key) {
            return d.clone();
        }
        let (template, instance) = match unit.split_once('@') {
            Some((base, rest)) => (
                format!("{base}@.{}", rest.rsplit('.').next().unwrap_or("service")),
                rest.rsplit_once('.').map_or(rest, |(i, _)| i),
            ),
            None => (unit.to_owned(), ""),
        };
        let dirs = if user {
            &self.user_unit_dirs
        } else {
            &self.system_unit_dirs
        };
        let desc = dirs.iter().find_map(|d| {
            [unit, template.as_str()].iter().find_map(|u| {
                root.read(format!("{d}/{u}"))?
                    .lines()
                    .find_map(|l| l.strip_prefix("Description="))
                    .map(|s| s.trim().replace("%i", instance).replace("%I", instance))
            })
        });
        self.units.insert(key, desc.clone());
        desc
    }

    pub fn steam_game(&mut self, root: &Root, app_id: &str) -> Option<String> {
        if let Some(n) = self.steam.get(app_id) {
            return n.clone();
        }
        let name = self.steam_libraries.iter().find_map(|lib| {
            let acf = root.read(format!("{lib}/steamapps/appmanifest_{app_id}.acf"))?;
            vdf_values(&acf, "name").into_iter().next()
        });
        self.steam.insert(app_id.to_owned(), name.clone());
        name
    }
}

impl Catalog {
    /// `steam_icon_<id>` exists only for games with a desktop shortcut; the
    /// client icon in Steam's library cache is the fallback (a file path).
    pub fn steam_icon(&self, root: &Root, app_id: &str) -> Option<String> {
        let name = format!("steam_icon_{app_id}");
        let themed = self.icon_dirs.iter().any(|d| {
            root.list(d)
                .iter()
                .any(|size| root.exists(format!("{d}/{size}/apps/{name}.png")))
        });
        if themed {
            return Some(name);
        }
        self.steam_libraries.iter().find_map(|lib| {
            let dir = format!("{lib}/appcache/librarycache/{app_id}");
            root.list(&dir)
                .into_iter()
                .filter(|f| f.ends_with(".jpg") || f.ends_with(".png") || f.ends_with(".ico"))
                .map(|f| format!("{dir}/{f}"))
                .filter(|p| root.path(p).is_file())
                .min_by_key(|p| {
                    std::fs::metadata(root.path(p))
                        .map(|m| m.len())
                        .unwrap_or(u64::MAX)
                })
                .map(|p| root.path(p).to_string_lossy().into_owned())
        })
    }
}

fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let quoted = format!("\"{key}\"");
    text.lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix(&quoted)?.trim();
            Some(rest.trim_matches('"').replace("\\\\", "\\"))
        })
        .collect()
}

fn steam_libraries(root: &Root, home: &str) -> Vec<String> {
    let mut libs: Vec<String> = Vec::new();
    for base in [
        format!("{home}/.local/share/Steam"),
        format!("{home}/.steam/steam"),
    ] {
        let vdf = root
            .read(format!("{base}/steamapps/libraryfolders.vdf"))
            .unwrap_or_default();
        for lib in std::iter::once(base.clone()).chain(vdf_values(&vdf, "path")) {
            let canonical = root.canonical(&lib).unwrap_or_else(|| PathBuf::from(&lib));
            let s = canonical.to_string_lossy().into_owned();
            if root.exists(&s) && !libs.contains(&s) {
                libs.push(s);
            }
        }
    }
    libs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_desktop_name() {
        let text = "[Desktop Entry]\nName=Files\nName[fr]=Fichiers\nIcon=folder\n[Desktop Action new]\nName=New\n";
        let (e, exec) = parse_desktop(text, &["fr_FR".into(), "fr".into()]).unwrap();
        assert_eq!(exec, None);
        assert_eq!(
            e,
            Entry {
                name: "Fichiers".into(),
                icon: Some("folder".into())
            }
        );
        assert_eq!(parse_desktop(text, &[]).unwrap().0.name, "Files");
    }

    #[test]
    fn vdf_paths() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"/mnt/Steam\"\n\t}\n}\n";
        assert_eq!(vdf_values(vdf, "path"), vec!["/mnt/Steam"]);
    }
}

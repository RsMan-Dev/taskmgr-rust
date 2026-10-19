//! Resolves which application a process belongs to, through sandboxes and
//! launchers (Flatpak, Wine/Proton, Steam, AppImage, NixOS wrappers).

use std::collections::HashMap;

use serde::Serialize;

use crate::catalog::Catalog;
use crate::util::Root;

mod naming;
#[cfg(test)]
mod tests;
mod units;

pub use naming::process_name;
use naming::role;
use units::{Location, app_unit_id, appimage_name, humanize, locate, mount_point, unit_name};

pub const TREE: &str = "tree";

pub const ENV_KEYS: [&str; 6] = [
    "SteamAppId",
    "SteamGameId",
    "STEAM_COMPAT_DATA_PATH",
    "APPIMAGE",
    "FLATPAK_ID",
    "WINEPREFIX",
];

#[derive(Clone, Debug, Default)]
pub struct ProcFacts {
    pub comm: String,
    pub cmdline: Vec<String>,
    pub exe: Option<String>,
    pub cgroup: String,
    pub env: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    App,
    Background,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Normal,
    Wrapper,
    Infra,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub key: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub kind: Kind,
    pub via: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub group: Group,
    pub name: String,
    pub role: Role,
    /// Group found from the program and a desktop entry, not from systemd:
    /// its descendants join it.
    pub launched: bool,
}

fn app_group(id: &str, catalog: &Catalog, via: &mut Vec<&'static str>) -> Group {
    if id.contains('/') {
        via.push("AppImage");
        return Group {
            key: format!("app:{id}"),
            name: Some(appimage_name(id)),
            icon: None,
            kind: Kind::App,
            via: vec![],
        };
    }
    let (id, flatpak) = match id.strip_prefix("flatpak-") {
        Some(rest) => (rest, true),
        None => (id, false),
    };
    if flatpak {
        via.push("Flatpak");
    }
    let entry = catalog
        .desktop(id)
        .or_else(|| catalog.desktop(id.split_once('-')?.1))
        .cloned();
    Group {
        key: format!("app:{id}"),
        name: Some(
            entry
                .as_ref()
                .map_or_else(|| humanize(id), |e| e.name.clone()),
        ),
        icon: entry.and_then(|e| e.icon),
        kind: Kind::App,
        via: vec![],
    }
}

pub fn resolve(f: &ProcFacts, catalog: &mut Catalog, root: &Root) -> Identity {
    let name = process_name(f);
    let role = role(f, &name);
    let mut via: Vec<&'static str> = Vec::new();
    let steam_id = ["SteamAppId", "SteamGameId"]
        .iter()
        .filter_map(|k| f.env.get(*k))
        .find(|v| v.parse::<u64>().is_ok_and(|n| n > 0))
        .cloned();
    let location = locate(&f.cgroup);
    let launch = match location {
        Location::Session(_) | Location::Other(_) => {
            catalog.launched_as(f.exe.as_deref(), &f.cmdline)
        }
        _ => None,
    };
    let launched = steam_id.is_none() && launch.is_some();
    let mut group = if let Some(id) = steam_id {
        via.push("Steam");
        let name = catalog.steam_game(root, &id);
        Group {
            key: format!("steam:{id}"),
            name,
            icon: catalog.steam_icon(root, &id),
            kind: Kind::App,
            via: vec![],
        }
    } else if let Some(id) = launch {
        app_group(&id, catalog, &mut via)
    } else {
        match location {
            Location::AppUnit(unit) => app_group(&app_unit_id(&unit), catalog, &mut via),
            Location::Snap(id) => {
                via.push("Snap");
                let mut g = app_group(&id, catalog, &mut via);
                if catalog.desktop(&id).is_none() {
                    g.name = Some(humanize(id.split('_').next().unwrap_or(&id)));
                }
                g
            }

            Location::DBus(id) => Group {
                key: format!("dbus:{id}"),
                name: Some(
                    catalog
                        .desktop(&id)
                        .map_or_else(|| id.clone(), |e| e.name.clone()),
                ),
                icon: catalog.desktop(&id).and_then(|e| e.icon.clone()),
                kind: Kind::Background,
                via: vec![],
            },
            Location::UserUnit(unit) => Group {
                key: format!("user:{}", unit_name(&unit)),
                name: Some(
                    catalog
                        .unit_description(root, &unit, !unit.starts_with("user@"))
                        .unwrap_or_else(|| unit_name(&unit)),
                ),
                icon: None,
                kind: Kind::Background,
                via: vec![],
            },
            Location::Session(scope) => Group {
                key: format!("session:{scope}"),
                name: Some("Login session".to_owned()),
                icon: None,
                kind: Kind::Background,
                via: vec![],
            },
            Location::SystemUnit(unit) => {
                let name = if unit.starts_with("docker-") && unit.ends_with(".scope") {
                    "Docker container".to_owned()
                } else if unit == "init.scope" {
                    "systemd".to_owned()
                } else if let Some(point) = mount_point(&unit) {
                    format!("Mount {point}")
                } else {
                    catalog
                        .unit_description(root, &unit, false)
                        .unwrap_or_else(|| unit_name(&unit))
                };
                Group {
                    key: format!("system:{}", unit_name(&unit)),
                    name: Some(name),
                    icon: None,
                    kind: Kind::System,
                    via: vec![],
                }
            }
            // No cgroup hierarchy (OpenRC, containers): grouped later by
            // process tree, see process::group_by_tree.
            Location::Other(path) if path.is_empty() || path == "/" => Group {
                key: TREE.to_owned(),
                name: None,
                icon: None,
                kind: Kind::System,
                via: vec![],
            },
            Location::Other(path) => Group {
                key: format!("other:{path}"),
                name: Some(path),
                icon: None,
                kind: Kind::System,
                via: vec![],
            },
        }
    };
    if f.env.contains_key("FLATPAK_ID") && !via.contains(&"Flatpak") {
        via.push("Flatpak");
    }
    if f.env.contains_key("APPIMAGE") && !via.contains(&"AppImage") {
        via.push("AppImage");
    }
    if f.env.contains_key("STEAM_COMPAT_DATA_PATH") {
        via.push("Proton");
    } else if f.env.contains_key("WINEPREFIX") || name.to_lowercase().ends_with(".exe") {
        via.push("Wine");
    }
    group.via = via;
    Identity {
        group,
        name,
        role,
        launched,
    }
}

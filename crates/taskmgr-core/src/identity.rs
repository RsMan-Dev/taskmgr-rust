//! Resolves which application a process belongs to, through sandboxes and
//! launchers (Flatpak, Wine/Proton, Steam, AppImage, NixOS wrappers).

use std::collections::HashMap;

use serde::Serialize;

use crate::catalog::Catalog;
use crate::util::Root;

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
}

const WRAPPERS: [&str; 9] = [
    "bwrap",
    "xdg-dbus-proxy",
    "appimage-run",
    "steam-run",
    "reaper",
    "srt-logger",
    "steam-launch-wrapper",
    "wine-preloader",
    "wine64-preloader",
];

const WINE_INFRA: [&str; 10] = [
    "wineserver",
    "services.exe",
    "winedevice.exe",
    "svchost.exe",
    "plugplay.exe",
    "rpcss.exe",
    "tabtip.exe",
    "conhost.exe",
    "start.exe",
    "rundll32.exe",
];

const INTERPRETERS: [&str; 9] = [
    "python", "python3", "perl", "node", "ruby", "bash", "sh", "zsh", "java",
];

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn dewrap(name: &str) -> String {
    let n = name.strip_prefix('.').unwrap_or(name);
    n.strip_suffix("-wrapped").unwrap_or(n).to_owned()
}

fn is_interpreter(name: &str) -> bool {
    INTERPRETERS.contains(&name)
        || name
            .strip_prefix("python3.")
            .is_some_and(|v| v.bytes().all(|b| b.is_ascii_digit()))
}

/// Display name of a process. `comm` is unreliable for Wine (thread names
/// such as `Main`) and truncated to 15 bytes.
pub fn process_name(f: &ProcFacts) -> String {
    let args: Vec<&str> = match f.cmdline.as_slice() {
        [single] if single.contains(' ') && single.starts_with('/') => {
            single.split_whitespace().collect()
        }
        other => other.iter().map(String::as_str).collect(),
    };
    if let Some(exe) = args
        .iter()
        .take(2)
        .find(|a| a.to_lowercase().ends_with(".exe"))
    {
        return basename(exe).to_owned();
    }
    let argv0 = args.first().copied().unwrap_or("");
    let base = if !argv0.is_empty() && !argv0.contains(' ') && !argv0.ends_with(':') {
        dewrap(basename(argv0))
    } else {
        dewrap(&f.comm)
    };
    let base = if base.is_empty() {
        dewrap(&f.comm)
    } else {
        base
    };
    if is_interpreter(&base) {
        // Options whose value is not the program (java -cp, --module-path).
        const VALUED: [&str; 6] = [
            "-cp",
            "-classpath",
            "--class-path",
            "-p",
            "--module-path",
            "-jar",
        ];
        let mut rest = args.iter().skip(1);
        while let Some(a) = rest.next() {
            if *a == "-jar" {
                if let Some(jar) = rest.next() {
                    return format!("{} ({base})", basename(jar));
                }
                break;
            }
            if VALUED.contains(a) {
                rest.next();
                continue;
            }
            if *a == "-c" || *a == "-m" {
                if *a == "-m"
                    && let Some(m) = rest.next()
                {
                    return format!("{m} ({base})");
                }
                break;
            }
            if !a.starts_with('-') {
                return format!("{} ({base})", basename(a));
            }
        }
    }
    base
}

fn role(f: &ProcFacts, name: &str) -> Role {
    let lower = name.to_lowercase();
    if WRAPPERS.contains(&lower.as_str()) || lower.starts_with("pressure-vessel") {
        Role::Wrapper
    } else if WINE_INFRA.contains(&lower.as_str())
        || (lower == "explorer.exe" && f.cmdline.iter().any(|a| a.eq_ignore_ascii_case("/desktop")))
    {
        Role::Infra
    } else {
        Role::Normal
    }
}

enum Location {
    AppUnit(String),
    DBus(String),
    UserUnit(String),
    Session(String),
    SystemUnit(String),
    Other(String),
}

fn unescape_unit(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("\\x") {
        out.push_str(&rest[..i]);
        match rest
            .get(i + 2..i + 4)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            Some(b) => {
                out.push(b as char);
                rest = &rest[i + 4..];
            }
            None => {
                out.push_str("\\x");
                rest = &rest[i + 2..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn locate(cgroup: &str) -> Location {
    let parts: Vec<&str> = cgroup.split('/').filter(|p| !p.is_empty()).collect();
    let after = |slice: &str| {
        let i = parts.iter().position(|p| *p == slice)?;
        parts[i + 1..].iter().find(|p| !p.ends_with(".slice"))
    };
    if let Some(unit) = after("app.slice") {
        return if let Some(rest) = unit.strip_prefix("dbus-:") {
            let name = rest.split_once('-').map_or(rest, |(_, n)| n);
            Location::DBus(name.split_once('@').map_or(name, |(n, _)| n).to_owned())
        } else if unit.starts_with("app-") {
            Location::AppUnit((*unit).to_owned())
        } else {
            Location::UserUnit((*unit).to_owned())
        };
    }
    for slice in ["session.slice", "background.slice"] {
        if let Some(unit) = after(slice) {
            return Location::UserUnit((*unit).to_owned());
        }
    }
    if let Some(s) = parts
        .iter()
        .find(|p| p.starts_with("session-") && p.ends_with(".scope"))
    {
        return Location::Session((*s).to_owned());
    }
    if let Some(u) = parts
        .iter()
        .find(|p| p.starts_with("user@") && p.ends_with(".service"))
    {
        return Location::UserUnit((*u).to_owned());
    }
    if let Some(unit) = after("system.slice") {
        return Location::SystemUnit((*unit).to_owned());
    }
    match parts.as_slice() {
        [unit] => Location::SystemUnit((*unit).to_owned()),
        _ => Location::Other(cgroup.to_owned()),
    }
}

/// `app-[<launcher>-]<id>-<random>.scope` or `app-[<launcher>-]<id>@<random>.service`.
fn app_unit_id(unit: &str) -> String {
    let body = unit.strip_prefix("app-").unwrap_or(unit);
    let id = if let Some(s) = body.strip_suffix(".service") {
        s.split_once('@').map_or(s, |(a, _)| a)
    } else {
        let s = body.strip_suffix(".scope").unwrap_or(body);
        match s.rsplit_once('-') {
            Some((head, tail))
                if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                head
            }
            _ => s,
        }
    };
    unescape_unit(id)
}

fn appimage_name(path: &str) -> String {
    let file = basename(path);
    let stem = file
        .len()
        .checked_sub(9)
        .filter(|&i| file[i..].eq_ignore_ascii_case(".appimage"))
        .map_or(file, |i| &file[..i]);
    let stem = ["-x86_64", "-x86-64", "-amd64", "-aarch64"]
        .iter()
        .find_map(|s| stem.strip_suffix(s))
        .unwrap_or(stem);
    stem.to_owned()
}

fn humanize(id: &str) -> String {
    let last = id.rsplit('.').next().unwrap_or(id);
    let mut c = last.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn unit_name(unit: &str) -> String {
    let n = unit.rsplit_once('.').map_or(unit, |(n, _)| n);
    n.split_once('@').map_or(n, |(base, _)| base).to_owned()
}

/// `mnt-MASS_HDD.mount` is the mount point `/mnt/MASS_HDD`.
fn mount_point(unit: &str) -> Option<String> {
    let path = unit.strip_suffix(".mount")?;
    Some(format!("/{}", unescape_unit(&path.replace('-', "/"))))
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
    } else {
        match locate(&f.cgroup) {
            Location::AppUnit(unit) => app_group(&app_unit_id(&unit), catalog, &mut via),
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
    Identity { group, name, role }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(comm: &str, cmdline: &[&str], cgroup: &str, env: &[(&str, &str)]) -> ProcFacts {
        ProcFacts {
            comm: comm.into(),
            cmdline: cmdline.iter().map(|s| (*s).into()).collect(),
            exe: None,
            cgroup: cgroup.into(),
            env: env
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        }
    }

    const APP: &str = "/user.slice/user-1000.slice/user@1000.service/app.slice/";

    fn catalog() -> Catalog {
        Catalog::with_desktop(&[
            ("org.vinegarhq.Vinegar", "Vinegar", "org.vinegarhq.Vinegar"),
            ("com.google.Chrome", "Google Chrome", "com.google.Chrome"),
            ("org.kde.konsole", "Konsole", "utilities-terminal"),
            ("steam", "Steam", "steam"),
        ])
    }

    fn resolve_one(f: &ProcFacts) -> Identity {
        resolve(f, &mut catalog(), &Root::new("/nonexistent"))
    }

    #[test]
    fn vinegar_wine_process_is_named_after_the_exe() {
        let cg = format!("{APP}app-flatpak-org.vinegarhq.Vinegar-3825891864.scope");
        let env = [
            ("FLATPAK_ID", "org.vinegarhq.Vinegar"),
            ("WINEPREFIX", "/p"),
        ];
        let id = resolve_one(&facts(
            "Main",
            &[
                "/home/u/.var/app/org.vinegarhq.Vinegar/data/vinegar/versions/v/RobloxStudioBeta.exe",
            ],
            &cg,
            &env,
        ));
        assert_eq!(id.name, "RobloxStudioBeta.exe");
        assert_eq!(id.role, Role::Normal);
        assert_eq!(id.group.name.as_deref(), Some("Vinegar"));
        assert_eq!(id.group.kind, Kind::App);
        assert_eq!(id.group.via, vec!["Flatpak", "Wine"]);

        let crash = resolve_one(&facts(
            "Main",
            &[r"Z:\home\u\RobloxCrashHandler.exe", "--no-rate-limit"],
            &cg,
            &env,
        ));
        assert_eq!(crash.name, "RobloxCrashHandler.exe");
        assert_eq!(crash.group.key, id.group.key);

        let infra = resolve_one(&facts(
            "services.exe",
            &[r"C:\windows\system32\services.exe"],
            &cg,
            &env,
        ));
        assert_eq!(infra.role, Role::Infra);
        let desktop = resolve_one(&facts(
            "explorer.exe",
            &[r"C:\windows\system32\explorer.exe", "/desktop"],
            &cg,
            &env,
        ));
        assert_eq!(desktop.role, Role::Infra);
    }

    #[test]
    fn bwrap_is_a_wrapper_inside_the_flatpak_group() {
        let cg = format!("{APP}app-flatpak-com.google.Chrome-12.scope");
        let id = resolve_one(&facts("bwrap", &["bwrap", "--args", "40"], &cg, &[]));
        assert_eq!(id.role, Role::Wrapper);
        assert_eq!(id.group.name.as_deref(), Some("Google Chrome"));
        let chrome = resolve_one(&facts(
            "chrome",
            &["/app/extra/chrome --type=renderer --lang=fr"],
            &cg,
            &[],
        ));
        assert_eq!(chrome.name, "chrome");
    }

    #[test]
    fn appimage_unit_carries_the_file_path() {
        let cg =
            format!("{APP}app-\\x2fhome\\x2fu\\x2fBureau\\x2fTMOG.AppImage@3b0be05c4f.service");
        let id = resolve_one(&facts(
            "AppRun.wrapped",
            &["/tmp/.mount/AppRun.wrapped"],
            &cg,
            &[],
        ));
        assert_eq!(id.group.name.as_deref(), Some("TMOG"));
        assert!(id.group.via.contains(&"AppImage"));
    }

    #[test]
    fn kde_scopes_and_services() {
        let konsole = resolve_one(&facts(
            "cargo",
            &["cargo", "build"],
            &format!("{APP}app-org.kde.konsole-12345.scope/tab(21089).scope"),
            &[],
        ));
        assert_eq!(konsole.group.name.as_deref(), Some("Konsole"));
        let steam = resolve_one(&facts(
            "steam",
            &["/s/steam"],
            &format!("{APP}app-steam@9b9badae4f5a.service"),
            &[],
        ));
        assert_eq!(steam.group.name.as_deref(), Some("Steam"));
        let unknown = resolve_one(&facts(
            "x",
            &["x"],
            &format!("{APP}app-org.example.Thing@1.service"),
            &[],
        ));
        assert_eq!(unknown.group.name.as_deref(), Some("Thing"));
    }

    #[test]
    fn steam_game_overrides_the_steam_scope() {
        let cg = format!("{APP}app-steam@9b9badae4f5a.service");
        let id = resolve_one(&facts(
            "Main",
            &[r"Z:\games\Game.exe"],
            &cg,
            &[("SteamAppId", "228980"), ("STEAM_COMPAT_DATA_PATH", "/c")],
        ));
        assert_eq!(id.group.key, "steam:228980");
        assert_eq!(id.group.via, vec!["Steam", "Proton"]);
        assert_eq!(id.name, "Game.exe");
    }

    #[test]
    fn services_and_system() {
        let pw = resolve_one(&facts(
            "pipewire",
            &["/nix/store/x/bin/pipewire"],
            "/user.slice/user-1000.slice/user@1000.service/session.slice/pipewire.service",
            &[],
        ));
        assert_eq!(pw.group.kind, Kind::Background);
        assert_eq!(pw.group.name.as_deref(), Some("pipewire"));
        let nm = resolve_one(&facts(
            "NetworkManager",
            &["NetworkManager"],
            "/system.slice/NetworkManager.service",
            &[],
        ));
        assert_eq!(nm.group.kind, Kind::System);
        let cups = resolve_one(&facts(
            "cupsd",
            &["cupsd"],
            "/system.slice/system-cups.slice/cups.service",
            &[],
        ));
        assert_eq!(cups.group.key, "system:cups");
        let ntfs = resolve_one(&facts(
            "ntfs-3g",
            &["ntfs-3g"],
            "/system.slice/mnt-MASS_HDD.mount",
            &[],
        ));
        assert_eq!(ntfs.group.name.as_deref(), Some("Mount /mnt/MASS_HDD"));
        let wallet = resolve_one(&facts(
            "kwalletd6",
            &["kwalletd6"],
            &format!(
                "{APP}app-dbus\\x2d:1.2\\x2dorg.kde.kwalletd6.slice/dbus-:1.2-org.kde.kwalletd6@0.service"
            ),
            &[],
        ));
        assert_eq!(wallet.group.key, "dbus:org.kde.kwalletd6");
        assert_eq!(wallet.group.kind, Kind::Background);
        let init = resolve_one(&facts(
            "systemd",
            &["/run/current-system/systemd/lib/systemd/systemd"],
            "/init.scope",
            &[],
        ));
        assert_eq!(init.group.name.as_deref(), Some("systemd"));
    }

    #[test]
    fn names() {
        let n = |comm: &str, cmd: &[&str]| process_name(&facts(comm, cmd, "/", &[]));
        assert_eq!(
            n(
                ".firefox-wrapp",
                &["/nix/store/x/bin/.firefox-wrapped", "-P"]
            ),
            "firefox"
        );
        assert_eq!(
            n("python3", &["/usr/bin/python3", "-u", "/opt/tool/run.py"]),
            "run.py (python3)"
        );
        assert_eq!(
            n("python3.13", &["python3.13", "-m", "http.server"]),
            "http.server (python3.13)"
        );
        assert_eq!(n("postgres", &["postgres: checkpointer "]), "postgres");
        assert_eq!(n("bash", &["bash"]), "bash");
        assert_eq!(
            n("java", &["java", "-Xmx1g", "-cp", "*", "org.Main"]),
            "org.Main (java)"
        );
        assert_eq!(
            n("java", &["/usr/bin/java", "-jar", "/opt/app/server.jar"]),
            "server.jar (java)"
        );
        assert_eq!(appimage_name("/a/Foo-1.2-x86_64.AppImage"), "Foo-1.2");
    }
}

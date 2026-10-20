//! What a process's launcher set up for it. Each item has the same three
//! sources, in order: the process environment, readable for the user's own
//! processes outside sandboxes; its command line, readable from anywhere;
//! its parent, the way the environment itself is inherited.

use super::ProcFacts;
use super::naming::basename;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    /// Steam app id of the game this process belongs to.
    pub steam_app: Option<String>,
    pub proton: bool,
    pub wine: bool,
    pub flatpak: bool,
    pub appimage: bool,
}

fn env<'a>(f: &'a ProcFacts, key: &str) -> Option<&'a str> {
    f.env.get(key).map(String::as_str).filter(|v| !v.is_empty())
}

fn steam_app(f: &ProcFacts) -> Option<String> {
    let valid = |v: &str| v.parse::<u64>().is_ok_and(|n| n > 0).then(|| v.to_owned());
    env(f, "SteamAppId")
        .or_else(|| env(f, "SteamGameId"))
        .and_then(valid)
        .or_else(|| {
            // Steam's reaper: `reaper SteamLaunch AppId=<id> -- ...`.
            f.cmdline.iter().any(|a| a == "SteamLaunch").then_some(())?;
            f.cmdline
                .iter()
                .find_map(|a| a.strip_prefix("AppId="))
                .and_then(valid)
        })
}

fn proton(f: &ProcFacts) -> bool {
    env(f, "STEAM_COMPAT_DATA_PATH").is_some() || f.cmdline.iter().any(|a| basename(a) == "proton")
}

fn wine(f: &ProcFacts) -> bool {
    let program = f.cmdline.first().map_or("", |a| basename(a));
    env(f, "WINEPREFIX").is_some()
        || program.starts_with("wine")
        || program.to_lowercase().ends_with(".exe")
}

fn appimage(f: &ProcFacts) -> bool {
    // The runtime mounts the image under /tmp/.mount_<name><random>.
    let mounted = |p: &str| p.starts_with("/tmp/.mount_");
    env(f, "APPIMAGE").is_some()
        || f.exe.as_deref().is_some_and(mounted)
        || f.cmdline.first().is_some_and(|a| mounted(a))
}

impl Context {
    pub fn of(f: &ProcFacts, parent: Option<&Context>) -> Self {
        let p = parent.cloned().unwrap_or_default();
        Context {
            steam_app: steam_app(f).or(p.steam_app),
            proton: proton(f) || p.proton,
            wine: wine(f) || p.wine,
            flatpak: env(f, "FLATPAK_ID").is_some()
                || f.cgroup.contains("/app-flatpak-")
                || p.flatpak,
            appimage: appimage(f) || p.appimage,
        }
    }
}

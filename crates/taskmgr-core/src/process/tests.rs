//! The grouping must not depend on what the kernel lets this process read:
//! the same /proc gives the same result with or without the environment of
//! other processes (another user namespace, as under appimage-run).

use std::collections::BTreeMap;
use std::path::Path;

use super::ProcessSampler;
use crate::util::Root;

const APP: &str = "/user.slice/user-1000.slice/user@1000.service/app.slice";

/// pid, parent, comm, command line, cgroup, environment.
type Proc<'a> = (u32, u32, &'a str, &'a [&'a str], String, &'a [&'a str]);

fn write_proc(base: &Path, p: &Proc) {
    let (pid, ppid, comm, cmdline, cgroup, env) = p;
    let dir = base.join(format!("proc/{pid}"));
    std::fs::create_dir_all(&dir).unwrap();
    // state ppid ... nice (17th field) threads start (20th).
    let mut fields = vec!["0"; 50];
    fields[0] = "S";
    let ppid = ppid.to_string();
    fields[1] = &ppid;
    fields[17] = "1";
    let stat = format!("{pid} ({comm}) {}\n", fields.join(" "));
    std::fs::write(dir.join("stat"), stat).unwrap();
    std::fs::write(dir.join("status"), "Uid:\t1000\t1000\t1000\t1000\n").unwrap();
    std::fs::write(dir.join("cmdline"), cmdline.join("\0") + "\0").unwrap();
    std::fs::write(dir.join("cgroup"), format!("0::{cgroup}\n")).unwrap();
    std::fs::write(dir.join("environ"), env.join("\0") + "\0").unwrap();
}

/// Group key and via chips of every process.
fn groups(base: &Path) -> BTreeMap<u32, (String, Vec<&'static str>)> {
    let root = Root::new(base);
    let mut sampler = ProcessSampler::new(&root);
    let mut out = BTreeMap::new();
    for g in sampler.sample(&root, 1.0, None, &[], None) {
        for p in &g.processes {
            let mut via = g.via.clone();
            via.sort_unstable();
            out.insert(p.pid, (g.key.clone(), via));
        }
    }
    out
}

#[test]
fn same_groups_with_or_without_the_environment() {
    let base = std::env::temp_dir().join(format!("taskmgr-ctx-{}", std::process::id()));
    let steam = format!("{APP}/app-steam@1.service");
    let game_env: &[&str] = &["SteamAppId=361420", "STEAM_COMPAT_DATA_PATH=/c"];
    let procs: Vec<Proc> = vec![
        (1, 0, "systemd", &["/sbin/init"], "/init.scope".into(), &[]),
        (100, 1, "steam", &["steam"], steam.clone(), &[]),
        (
            200,
            100,
            "reaper",
            &["reaper", "SteamLaunch", "AppId=361420", "--", "run"],
            steam.clone(),
            &["SteamAppId=361420"],
        ),
        (
            210,
            200,
            "srt-bwrap",
            &["srt-bwrap", "--args", "26"],
            steam.clone(),
            &["SteamAppId=361420"],
        ),
        (
            220,
            210,
            "python3",
            &["python3", "/s/Proton 9/proton", "waitforexitandrun"],
            steam.clone(),
            game_env,
        ),
        (
            230,
            220,
            "Game.exe",
            &[r"Z:\g\Game.exe"],
            steam.clone(),
            game_env,
        ),
        (
            300,
            1,
            "app",
            &["/app/bin/app"],
            format!("{APP}/app-flatpak-org.example.App-12.scope"),
            &["FLATPAK_ID=org.example.App"],
        ),
        (
            400,
            1,
            "AppRun",
            &["/tmp/.mount_ToolAbc/AppRun"],
            format!("{APP}/app-tool-3.scope"),
            &["APPIMAGE=/h/Tool.AppImage"],
        ),
    ];
    for p in &procs {
        write_proc(&base, p);
    }
    let readable = groups(&base);
    for (pid, ..) in &procs {
        std::fs::remove_file(base.join(format!("proc/{pid}/environ"))).unwrap();
    }
    let hidden = groups(&base);
    std::fs::remove_dir_all(&base).unwrap();

    assert_eq!(readable, hidden);
    let game = &readable[&230];
    assert_eq!(game.0, "steam:361420");
    assert_eq!(game.1, vec!["Proton", "Steam"]);
    assert_eq!(readable[&200].0, "steam:361420");
    assert_eq!(readable[&100].0, "app:steam");
    assert!(readable[&300].1.contains(&"Flatpak"));
    assert!(readable[&400].1.contains(&"AppImage"));
}

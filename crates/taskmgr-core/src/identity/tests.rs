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
        &["/home/u/.var/app/org.vinegarhq.Vinegar/data/vinegar/versions/v/RobloxStudioBeta.exe"],
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
    let cg = format!("{APP}app-\\x2fhome\\x2fu\\x2fBureau\\x2fTMOG.AppImage@3b0be05c4f.service");
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
    assert_eq!(
        n(
            "ld-linux-x86-64",
            &[
                "/tmp/.mount_x/usr/lib/ld-linux-x86-64.so.2",
                "--argv0",
                "/tmp/.mount_x/AppRun",
                "--library-path",
                "/tmp/.mount_x/usr/lib:/usr/lib",
                "/tmp/.mount_x/usr/bin/taskmgr"
            ]
        ),
        "taskmgr"
    );
    assert_eq!(appimage_name("/a/Foo-1.2-x86_64.AppImage"), "Foo-1.2");
}

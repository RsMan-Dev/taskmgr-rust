//! Display name and role of a process, seen through its wrappers.

use super::{ProcFacts, Role};

const WRAPPERS: [&str; 11] = [
    "bwrap",
    "srt-bwrap",
    "pv-adverb",
    "xdg-dbus-proxy",
    "appimage-run",
    "steam-run",
    "reaper",
    "srt-logger",
    "steam-launch-wrapper",
    "wine-preloader",
    "wine64-preloader",
];

const WINE_INFRA: [&str; 11] = [
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
    "xalia.exe",
];

const INTERPRETERS: [&str; 9] = [
    "python", "python3", "perl", "node", "ruby", "bash", "sh", "zsh", "java",
];

pub(super) fn basename(path: &str) -> &str {
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

/// A program started through the dynamic loader (`ld-linux-x86-64.so.2
/// --library-path ... /path/to/program`, as bundles do): its arguments
/// from the program on.
fn through_loader(cmdline: &[String]) -> Option<&[String]> {
    const VALUED: [&str; 6] = [
        "--library-path",
        "--argv0",
        "--preload",
        "--audit",
        "--inhibit-rpath",
        "--glibc-hwcaps-mask",
    ];
    let loader = basename(cmdline.first()?);
    if !(loader.starts_with("ld-linux") || loader.starts_with("ld-musl") || loader == "ld.so") {
        return None;
    }
    let mut i = 1;
    while let Some(a) = cmdline.get(i) {
        if !a.starts_with("--") {
            return Some(&cmdline[i..]);
        }
        i += if VALUED.contains(&a.as_str()) { 2 } else { 1 };
    }
    None
}

/// Display name of a process. `comm` is unreliable for Wine (thread names
/// such as `Main`) and truncated to 15 bytes.
pub fn process_name(f: &ProcFacts) -> String {
    if let Some(program) = through_loader(&f.cmdline) {
        let inner = ProcFacts {
            comm: basename(&program[0]).to_owned(),
            cmdline: program.to_vec(),
            ..ProcFacts::default()
        };
        return process_name(&inner);
    }
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

pub(super) fn role(f: &ProcFacts, name: &str) -> Role {
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

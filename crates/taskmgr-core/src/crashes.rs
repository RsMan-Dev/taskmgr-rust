//! Crashed processes, from the journal: core dumps recorded by
//! systemd-coredump (with the stack trace), units hit by the OOM killer and
//! services whose main process failed. Recorded by the system, so crashes
//! that happened while taskmgr was closed are listed too.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use crate::Root;
use crate::catalog::Catalog;
use crate::identity::{self, ProcFacts};
use crate::journal::{field, run, text};
use crate::process::Users;

const COREDUMP: &str = "fc2e22bc6ee647b6b90729ab34a250b1";
const UNIT_OOM: &str = "fe6faa94e7774663a0da52717891d8ef";
const UNIT_EXITED: &str = "98e322203f7a4ed290d09fe03c09fe15";

#[derive(Clone, Debug, Serialize)]
pub struct Crash {
    /// "crash" (fatal signal), "oom" (killed for lack of memory) or
    /// "failed" (exited with an error code).
    pub kind: &'static str,
    pub time_us: u64,
    pub app: String,
    pub app_key: String,
    pub icon: Option<String>,
    pub process: String,
    pub exe: Option<String>,
    pub pid: Option<u32>,
    pub user: String,
    /// SIGSEGV, SIGABRT, ...
    pub signal: Option<String>,
    pub exit_status: Option<i32>,
    pub unit: Option<String>,
    pub user_unit: bool,
    pub command: String,
    /// Stack of the thread that crashed, when systemd-coredump got one.
    pub trace: String,
}

const SIGNALS: [(i32, &str); 8] = [
    (4, "SIGILL"),
    (5, "SIGTRAP"),
    (6, "SIGABRT"),
    (7, "SIGBUS"),
    (8, "SIGFPE"),
    (11, "SIGSEGV"),
    (31, "SIGSYS"),
    (9, "SIGKILL"),
];

fn signal_name(n: i32) -> String {
    SIGNALS
        .iter()
        .find(|(k, _)| *k == n)
        .map_or_else(|| format!("signal {n}"), |(_, s)| (*s).to_owned())
}

/// First "Stack trace of thread" block of a systemd-coredump message.
fn first_trace(message: &str) -> String {
    let Some(start) = message.find("Stack trace of thread") else {
        return String::new();
    };
    message[start..]
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .take(40)
        .collect::<Vec<_>>()
        .join("\n")
}

type Fields = HashMap<String, Value>;

fn get(e: &Fields, k: &str) -> Option<String> {
    e.get(k).and_then(text).filter(|s| !s.is_empty())
}

struct Namer {
    catalog: Catalog,
    users: Users,
}

impl Namer {
    fn name(&mut self, root: &Root, facts: &ProcFacts) -> (String, String, Option<String>) {
        let id = identity::resolve(facts, &mut self.catalog, root);
        let name = id.group.name.clone().unwrap_or_else(|| id.name.clone());
        (name, id.group.key, id.group.icon)
    }
}

fn unit_cgroup(unit: &str, user: bool, uid: u32) -> String {
    if user {
        format!("/user.slice/user-{uid}.slice/user@{uid}.service/app.slice/{unit}")
    } else {
        format!("/system.slice/{unit}")
    }
}

/// Proton games run in steamapps/compatdata/<app id>: the id names the game
/// when the environment was not recorded.
fn steam_app_id(paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|p| {
        let rest = &p[p.find("/steamapps/compatdata/")? + "/steamapps/compatdata/".len()..];
        let id: String = rest.chars().take_while(char::is_ascii_digit).collect();
        (!id.is_empty()).then_some(id)
    })
}

fn from_coredump(root: &Root, namer: &mut Namer, e: &Fields) -> Crash {
    let uid: u32 = field(e, "COREDUMP_UID").unwrap_or(u32::MAX);
    let command = get(e, "COREDUMP_CMDLINE").unwrap_or_default();
    let facts = ProcFacts {
        comm: get(e, "COREDUMP_COMM").unwrap_or_default(),
        cmdline: command.split_whitespace().map(str::to_owned).collect(),
        exe: get(e, "COREDUMP_EXE"),
        cgroup: get(e, "COREDUMP_CGROUP").unwrap_or_default(),
        env: get(e, "COREDUMP_ENVIRON")
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
    };
    let mut facts = facts;
    if !facts.env.contains_key("SteamAppId") {
        let cwd = get(e, "COREDUMP_CWD").unwrap_or_default();
        if let Some(id) = steam_app_id(&[&cwd, &command]) {
            facts.env.insert("SteamAppId".to_owned(), id);
        }
    }
    let (app, app_key, icon) = namer.name(root, &facts);
    let user_unit = get(e, "COREDUMP_USER_UNIT");
    Crash {
        kind: "crash",
        time_us: field(e, "COREDUMP_TIMESTAMP")
            .or_else(|| field(e, "__REALTIME_TIMESTAMP"))
            .unwrap_or(0),
        app,
        app_key,
        icon,
        process: identity::process_name(&facts),
        exe: facts.exe.clone(),
        pid: field(e, "COREDUMP_PID"),
        user: namer.users.name(root, uid),
        signal: get(e, "COREDUMP_SIGNAL_NAME")
            .or_else(|| field(e, "COREDUMP_SIGNAL").map(signal_name)),
        exit_status: None,
        user_unit: user_unit.is_some(),
        unit: user_unit.or_else(|| get(e, "COREDUMP_UNIT")),
        command,
        trace: get(e, "MESSAGE")
            .map(|m| first_trace(&m))
            .unwrap_or_default(),
    }
}

/// OOM kills and failed main processes, reported by systemd per unit.
fn from_unit(root: &Root, namer: &mut Namer, e: &Fields) -> Option<Crash> {
    let (unit, user_unit) = match get(e, "USER_UNIT") {
        Some(u) => (u, true),
        None => (get(e, "UNIT")?, false),
    };
    // Slices aggregate many units: the unit itself is reported too.
    if unit.ends_with(".slice") {
        return None;
    }
    let status: Option<i32> = field(e, "EXIT_STATUS");
    let (kind, signal, exit_status) = match get(e, "MESSAGE_ID").as_deref() {
        Some(UNIT_OOM) => ("oom", None, None),
        _ => match (get(e, "EXIT_CODE").as_deref(), status) {
            (Some("exited"), Some(s)) if s != 0 => ("failed", None, Some(s)),
            // Stopped on purpose (TERM, KILL, INT, HUP, PIPE) is not a crash.
            (Some("killed" | "dumped"), Some(s)) if [4, 5, 6, 7, 8, 11, 31].contains(&s) => {
                ("crash", Some(signal_name(s)), None)
            }
            _ => return None,
        },
    };
    let uid: u32 = field(e, "_UID").unwrap_or(0);
    let facts = ProcFacts {
        comm: unit.clone(),
        cgroup: unit_cgroup(&unit, user_unit, uid),
        ..ProcFacts::default()
    };
    let (app, app_key, icon) = namer.name(root, &facts);
    let process = if app_key.starts_with("app:") {
        app.clone()
    } else {
        unit.clone()
    };
    Some(Crash {
        kind,
        time_us: field(e, "__REALTIME_TIMESTAMP").unwrap_or(0),
        app,
        app_key,
        icon,
        process,
        exe: None,
        pid: None,
        user: namer.users.name(root, uid),
        signal,
        exit_status,
        unit: Some(unit),
        user_unit,
        command: String::new(),
        trace: String::new(),
    })
}

/// Newest first. `since_us`: microseconds since the epoch.
pub fn list(root: &Root, since_us: Option<u64>) -> Result<Vec<Crash>, String> {
    let mut args = vec![
        "-r".to_owned(),
        "-n".to_owned(),
        "400".to_owned(),
        format!("MESSAGE_ID={COREDUMP}"),
        format!("MESSAGE_ID={UNIT_OOM}"),
        format!("MESSAGE_ID={UNIT_EXITED}"),
        "--output-fields=MESSAGE_ID,MESSAGE,UNIT,USER_UNIT,EXIT_CODE,EXIT_STATUS,_UID,\
         COREDUMP_UID,COREDUMP_PID,COREDUMP_CWD,COREDUMP_COMM,COREDUMP_EXE,COREDUMP_CMDLINE,COREDUMP_CGROUP,\
         COREDUMP_ENVIRON,COREDUMP_UNIT,COREDUMP_USER_UNIT,COREDUMP_SIGNAL,COREDUMP_SIGNAL_NAME,\
         COREDUMP_TIMESTAMP"
            .to_owned(),
    ];
    if let Some(s) = since_us {
        args.push(format!("--since=@{}", s / 1_000_000));
    }
    let entries = run(&args)?;
    let mut namer = Namer {
        catalog: Catalog::load(root),
        users: Users::default(),
    };
    let mut out: Vec<Crash> = Vec::new();
    for e in &entries {
        if get(e, "MESSAGE_ID").as_deref() == Some(COREDUMP) {
            out.push(from_coredump(root, &mut namer, e));
        }
    }
    let dumps = out.clone();
    for e in &entries {
        if get(e, "MESSAGE_ID").as_deref() == Some(COREDUMP) {
            continue;
        }
        let Some(c) = from_unit(root, &mut namer, e) else {
            continue;
        };
        // The same crash seen by systemd-coredump already has its trace.
        let seen = c.kind == "crash"
            && dumps
                .iter()
                .any(|d| d.unit == c.unit && d.time_us.abs_diff(c.time_us) < 30_000_000);
        if !seen {
            out.push(c);
        }
    }
    out.sort_by_key(|c| std::cmp::Reverse(c.time_us));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_proton_game() {
        assert_eq!(
            steam_app_id(&["/mnt/x/SteamLibrary/steamapps/compatdata/1203620/pfx/drive_c/windows"]),
            Some("1203620".to_owned())
        );
        assert_eq!(steam_app_id(&["/home/a"]), None);
    }

    #[test]
    fn keeps_the_crashing_thread_only() {
        let m = "Process 1 (x) of user 1000 dumped core.\n\nModule libc.so\nStack trace of thread 1:\n#0  0x1 abort (libc.so.6)\n#1  0x2 main (x)\n\nStack trace of thread 2:\n#0  0x3 poll (libc.so.6)\n";
        assert_eq!(
            first_trace(m),
            "Stack trace of thread 1:\n#0  0x1 abort (libc.so.6)\n#1  0x2 main (x)"
        );
    }

    #[test]
    fn deliberate_stops_are_not_crashes() {
        let root = Root::new("/nonexistent");
        let mut namer = Namer {
            catalog: Catalog::with_desktop(&[]),
            users: Users::default(),
        };
        let entry = |code: &str, status: &str| -> Fields {
            [
                ("MESSAGE_ID", UNIT_EXITED),
                ("USER_UNIT", "app-org.videolan.VLC@1.service"),
                ("EXIT_CODE", code),
                ("EXIT_STATUS", status),
                ("_UID", "1000"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), Value::String(v.to_owned())))
            .collect()
        };
        assert!(from_unit(&root, &mut namer, &entry("killed", "9")).is_none());
        assert!(from_unit(&root, &mut namer, &entry("killed", "15")).is_none());
        assert!(from_unit(&root, &mut namer, &entry("exited", "0")).is_none());
        let segv = from_unit(&root, &mut namer, &entry("dumped", "11")).unwrap();
        assert_eq!(
            (segv.kind, segv.signal.as_deref()),
            ("crash", Some("SIGSEGV"))
        );
        let failed = from_unit(&root, &mut namer, &entry("exited", "1")).unwrap();
        assert_eq!((failed.kind, failed.exit_status), ("failed", Some(1)));
    }
}

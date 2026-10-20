//! systemd journal, read through journalctl (present wherever the journal
//! is). Members of wheel, adm or systemd-journal see the whole system;
//! others see their own processes.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Query {
    /// systemd unit; `user` says whether it belongs to the session.
    pub unit: Option<String>,
    #[serde(default)]
    pub user: bool,
    #[serde(default)]
    pub pids: Vec<u32>,
    /// Adds the running descendants of `pids`.
    #[serde(default)]
    pub children: bool,
    /// Microseconds since the epoch.
    pub since_us: Option<u64>,
    pub until_us: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub time_us: u64,
    /// 0 (emergency) to 7 (debug).
    pub priority: u8,
    pub source: String,
    pub pid: Option<u32>,
    pub message: String,
}

pub(crate) fn journalctl() -> Option<String> {
    crate::util::find_program(
        "journalctl",
        &[
            "/run/current-system/sw/bin/journalctl",
            "/usr/bin/journalctl",
            "/bin/journalctl",
        ],
    )
}

/// A field as text: journalctl prints binary values as byte arrays.
pub(crate) fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(bytes) => {
            let b: Vec<u8> = bytes
                .iter()
                .filter_map(|x| x.as_u64().map(|n| n as u8))
                .collect();
            Some(String::from_utf8_lossy(&b).into_owned())
        }
        _ => None,
    }
}

/// Runs journalctl with JSON output and returns one map per entry.
pub(crate) fn run(args: &[String]) -> Result<Vec<HashMap<String, Value>>, String> {
    let program = journalctl().ok_or("journalctl not found: no systemd journal.")?;
    let out = std::process::Command::new(program)
        .args(["-o", "json", "--no-pager", "-q"])
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() && out.stdout.is_empty() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

pub(crate) fn field<T: std::str::FromStr>(e: &HashMap<String, Value>, k: &str) -> Option<T> {
    e.get(k).and_then(text).and_then(|s| s.parse().ok())
}

/// Running processes below `pids` in the process tree.
fn descendants(pids: &[u32]) -> Vec<u32> {
    let parents: Vec<(u32, u32)> = std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let pid: u32 = e.ok()?.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            let ppid = stat[stat.rfind(')')? + 1..]
                .split_whitespace()
                .nth(1)?
                .parse()
                .ok()?;
            Some((pid, ppid))
        })
        .collect();
    let mut found: Vec<u32> = Vec::new();
    let mut frontier = pids.to_vec();
    while !frontier.is_empty() {
        let next: Vec<u32> = parents
            .iter()
            .filter(|(pid, ppid)| {
                frontier.contains(ppid) && !pids.contains(pid) && !found.contains(pid)
            })
            .map(|(pid, _)| *pid)
            .collect();
        found.extend(&next);
        frontier = next;
    }
    found
}

/// When a running process started, in seconds since the epoch (rounded
/// down: the journal compares whole seconds).
fn started_s(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let ticks: u64 = stat[stat.rfind(')')? + 1..]
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()?;
    let boot: u64 = std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("btime "))?
        .trim()
        .parse()
        .ok()?;
    Some(boot + ticks / rustix::param::clock_ticks_per_second())
}

pub fn entries(q: &Query) -> Result<Vec<Entry>, String> {
    let mut args = vec![
        "-n".to_owned(),
        q.limit.unwrap_or(500).to_string(),
        "--output-fields=PRIORITY,SYSLOG_IDENTIFIER,_COMM,_PID,MESSAGE".to_owned(),
    ];
    if let Some(u) = &q.unit {
        args.push(if q.user { "--user-unit" } else { "--unit" }.to_owned());
        args.push(u.clone());
    }
    let mut pids = q.pids.clone();
    if q.children {
        pids.extend(descendants(&q.pids));
    }
    // Matches on one field are OR-ed by journalctl.
    args.extend(pids.iter().take(256).map(|p| format!("_PID={p}")));
    // A pid is reused, across boots and within one: without a time range,
    // entries start when the oldest of these processes did.
    let since = match q.since_us {
        None if !q.pids.is_empty() => {
            let started = pids.iter().filter_map(|p| started_s(*p)).min();
            if started.is_none() {
                args.push("--boot".to_owned());
            }
            started
        }
        s => s.map(|s| s / 1_000_000),
    };
    if let Some(s) = since {
        args.push(format!("--since=@{s}"));
    }
    if let Some(u) = q.until_us {
        args.push(format!("--until=@{}", u.div_ceil(1_000_000)));
    }
    Ok(run(&args)?
        .iter()
        .map(|e| Entry {
            time_us: field(e, "__REALTIME_TIMESTAMP").unwrap_or(0),
            priority: field(e, "PRIORITY").unwrap_or(6),
            source: e
                .get("SYSLOG_IDENTIFIER")
                .or_else(|| e.get("_COMM"))
                .and_then(text)
                .unwrap_or_default(),
            pid: field(e, "_PID"),
            message: e.get("MESSAGE").and_then(text).unwrap_or_default(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_running_children() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let found = descendants(&[std::process::id()]);
        let _ = child.kill();
        let _ = child.wait();
        assert!(found.contains(&child.id()));
        assert!(started_s(std::process::id()).is_some());
    }
}

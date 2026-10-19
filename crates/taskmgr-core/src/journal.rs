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
    crate::energy::find_program(
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
    // Matches on one field are OR-ed by journalctl.
    args.extend(q.pids.iter().take(64).map(|p| format!("_PID={p}")));
    if let Some(s) = q.since_us {
        args.push(format!("--since=@{}", s / 1_000_000));
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

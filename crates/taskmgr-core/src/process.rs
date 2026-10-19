use std::collections::HashMap;

use serde::Serialize;

use crate::catalog::Catalog;
use crate::identity::{self, ENV_KEYS, Identity, Kind, ProcFacts, Role};
use crate::util::Root;

#[derive(Clone, Debug, Serialize)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub role: Role,
    pub cpu: f64,
    pub memory: u64,
    pub disk_bps: Option<f64>,
    pub threads: u64,
    pub command: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcessGroup {
    pub key: String,
    pub name: String,
    pub icon: Option<String>,
    pub kind: Kind,
    pub via: Vec<&'static str>,
    pub cpu: f64,
    pub memory: u64,
    pub disk_bps: Option<f64>,
    pub processes: Vec<ProcessRow>,
}

struct Stat {
    comm: String,
    ppid: u32,
    ticks: u64,
    threads: u64,
    start: u64,
}

fn parse_stat(text: &str) -> Option<Stat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let f: Vec<&str> = text[close + 1..].split_whitespace().collect();
    let n = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok());
    Some(Stat {
        comm: text[open + 1..close].to_owned(),
        ppid: n(1)? as u32,
        ticks: n(11)? + n(12)?,
        threads: n(17)?,
        start: n(19)?,
    })
}

fn status_kb(status: &str, key: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|l| l.strip_prefix(key))
        .and_then(|v| {
            v.trim_start_matches(':')
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn io_bytes(io: &str) -> Option<u64> {
    let get = |k: &str| {
        io.lines()
            .find_map(|l| l.strip_prefix(k)?.trim().parse::<u64>().ok())
    };
    Some(get("read_bytes:")? + get("write_bytes:")?)
}

fn split_nul(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

struct Prev {
    start: u64,
    ticks: u64,
    io: Option<u64>,
}

pub struct ProcessSampler {
    catalog: Catalog,
    prev: HashMap<u32, Prev>,
    identities: HashMap<(u32, u64, String), Identity>,
    tick_hz: f64,
    cpus: f64,
}

impl ProcessSampler {
    pub fn new(root: &Root) -> Self {
        // SAFETY: sysconf has no preconditions.
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        Self {
            catalog: Catalog::load(root),
            prev: HashMap::new(),
            identities: HashMap::new(),
            tick_hz: if hz > 0 { hz as f64 } else { 100.0 },
            cpus: std::thread::available_parallelism().map_or(1, |n| n.get()) as f64,
        }
    }

    fn facts(root: &Root, pid: u32, comm: &str) -> ProcFacts {
        let dir = format!("/proc/{pid}");
        let env = std::fs::read(root.path(format!("{dir}/environ")))
            .map(|b| {
                split_nul(&b)
                    .into_iter()
                    .filter_map(|kv| {
                        let (k, v) = kv.split_once('=')?;
                        ENV_KEYS.contains(&k).then(|| (k.to_owned(), v.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        ProcFacts {
            comm: comm.to_owned(),
            cmdline: std::fs::read(root.path(format!("{dir}/cmdline")))
                .map(|b| split_nul(&b))
                .unwrap_or_default(),
            exe: std::fs::read_link(root.path(format!("{dir}/exe")))
                .ok()
                .map(|p| p.to_string_lossy().into_owned()),
            cgroup: root
                .read(format!("{dir}/cgroup"))
                .and_then(|c| {
                    c.lines()
                        .find_map(|l| l.strip_prefix("0::").map(str::to_owned))
                })
                .unwrap_or_default(),
            env,
        }
    }

    pub fn sample(&mut self, root: &Root, secs: f64) -> Vec<ProcessGroup> {
        let mut groups: HashMap<String, ProcessGroup> = HashMap::new();
        let mut seen = HashMap::new();
        for entry in root.list("/proc") {
            let Ok(pid) = entry.parse::<u32>() else {
                continue;
            };
            let Some(stat) = root
                .read(format!("/proc/{pid}/stat"))
                .and_then(|s| parse_stat(&s))
            else {
                continue;
            };
            if pid == 2 || stat.ppid == 2 {
                continue;
            }
            let status = root.read(format!("/proc/{pid}/status")).unwrap_or_default();
            let io = root
                .read(format!("/proc/{pid}/io"))
                .and_then(|s| io_bytes(&s));
            let (cpu, disk_bps) = match self
                .prev
                .get(&pid)
                .filter(|p| p.start == stat.start && secs > 0.0)
            {
                Some(p) => (
                    stat.ticks.saturating_sub(p.ticks) as f64 / self.tick_hz / secs / self.cpus
                        * 100.0,
                    io.zip(p.io).map(|(c, o)| c.saturating_sub(o) as f64 / secs),
                ),
                None => (0.0, io.map(|_| 0.0)),
            };
            seen.insert(
                pid,
                Prev {
                    start: stat.start,
                    ticks: stat.ticks,
                    io,
                },
            );

            let key = (pid, stat.start, stat.comm.clone());
            let id = match self.identities.get(&key) {
                Some(id) => id.clone(),
                None => {
                    let facts = Self::facts(root, pid, &stat.comm);
                    if facts.cmdline.is_empty() && facts.cgroup.is_empty() {
                        continue;
                    }
                    let id = identity::resolve(&facts, &mut self.catalog, root);
                    self.identities.insert(key, id.clone());
                    id
                }
            };
            let command = std::fs::read(root.path(format!("/proc/{pid}/cmdline")))
                .map(|b| split_nul(&b).join(" "))
                .unwrap_or_default();
            let row = ProcessRow {
                pid,
                name: id.name.clone(),
                role: id.role,
                cpu,
                memory: (status_kb(&status, "RssAnon").unwrap_or(0)
                    + status_kb(&status, "RssShmem").unwrap_or(0))
                    * 1024,
                disk_bps,
                threads: stat.threads,
                command: command.chars().take(400).collect(),
            };
            let g = groups
                .entry(id.group.key.clone())
                .or_insert_with(|| ProcessGroup {
                    key: id.group.key.clone(),
                    name: id.group.name.clone().unwrap_or_default(),
                    icon: id.group.icon.clone(),
                    kind: id.group.kind,
                    via: Vec::new(),
                    cpu: 0.0,
                    memory: 0,
                    disk_bps: None,
                    processes: Vec::new(),
                });
            for v in &id.group.via {
                if !g.via.contains(v) {
                    g.via.push(v);
                }
            }
            g.cpu += row.cpu;
            g.memory += row.memory;
            if let Some(d) = row.disk_bps {
                g.disk_bps = Some(g.disk_bps.unwrap_or(0.0) + d);
            }
            g.processes.push(row);
        }
        self.identities
            .retain(|(pid, start, _), _| seen.get(pid).is_some_and(|p| p.start == *start));
        self.prev = seen;

        let mut out: Vec<ProcessGroup> = groups.into_values().collect();
        for g in &mut out {
            if g.name.is_empty() {
                g.name = g
                    .processes
                    .iter()
                    .filter(|p| p.role == Role::Normal)
                    .max_by_key(|p| p.memory)
                    .map_or_else(|| "Application".to_owned(), |p| p.name.clone());
            }
            g.processes.sort_by_key(|p| p.pid);
        }
        out.sort_by_key(|g| g.name.to_lowercase());
        out
    }
}

/// Signals every pid. Returns false if any of them was denied; processes
/// that already exited are not an error.
pub fn terminate(pids: &[i32], force: bool) -> bool {
    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    let mut ok = true;
    for &pid in pids.iter().filter(|p| **p > 0) {
        // SAFETY: kill has no memory preconditions.
        if unsafe { libc::kill(pid, signal) } != 0 {
            ok &= std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminate_kills_own_process_and_reports_denial() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        assert!(terminate(&[child.id() as i32], false));
        assert!(!child.wait().unwrap().success());
        assert!(!terminate(&[1], false));
        assert!(terminate(&[child.id() as i32], true));
    }

    #[test]
    fn stat_with_spaces_in_comm() {
        let s = parse_stat(
            "42 (Web Content) S 1 42 42 0 -1 4194560 1 0 0 0 30 12 0 0 20 0 7 0 555 0 0",
        )
        .unwrap();
        assert_eq!(s.comm, "Web Content");
        assert_eq!((s.ppid, s.ticks, s.threads, s.start), (1, 42, 7, 555));
    }

    #[test]
    fn io_and_status() {
        assert_eq!(
            io_bytes("rchar: 9\nread_bytes: 4096\nwrite_bytes: 512\n"),
            Some(4608)
        );
        assert_eq!(status_kb("RssAnon:\t  2048 kB\n", "RssAnon"), Some(2048));
    }
}

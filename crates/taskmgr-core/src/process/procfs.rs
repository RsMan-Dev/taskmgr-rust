//! Parsing of /proc/<pid> files.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::os::fd::OwnedFd;

use rustix::fs::{Mode, OFlags};

use crate::util::Root;

/// A process directory opened once: its files then all come from that
/// process, even if it exits and its pid is reused in between.
pub(super) struct ProcDir(OwnedFd);

impl ProcDir {
    pub fn open(root: &Root, pid: u32) -> Option<Self> {
        let flags = OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC;
        rustix::fs::open(root.path(format!("/proc/{pid}")), flags, Mode::empty())
            .ok()
            .map(Self)
    }

    fn file(&self, name: &str) -> Option<std::fs::File> {
        rustix::fs::openat(
            &self.0,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()
        .map(std::fs::File::from)
    }

    pub fn read_into(&self, name: &str, buf: &mut String) -> bool {
        buf.clear();
        self.file(name)
            .is_some_and(|mut f| f.read_to_string(buf).is_ok())
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let mut bytes = Vec::new();
        self.file(name)?.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    pub fn link(&self, name: &str) -> Option<String> {
        let target = rustix::fs::readlinkat(&self.0, name, Vec::new()).ok()?;
        Some(target.to_string_lossy().into_owned())
    }
}

pub(super) struct Stat {
    pub comm: String,
    pub state: char,
    pub nice: i32,
    pub ppid: u32,
    pub ticks: u64,
    pub threads: u64,
    pub start: u64,
}

/// Processes ordered so that each comes after its parent.
pub(super) fn parents_first(procs: Vec<(u32, Stat)>) -> Vec<(u32, Stat)> {
    let pids: HashSet<u32> = procs.iter().map(|(p, _)| *p).collect();
    let mut children: HashMap<u32, Vec<(u32, Stat)>> = HashMap::new();
    let mut out = Vec::with_capacity(procs.len());
    for (pid, stat) in procs {
        if pids.contains(&stat.ppid) && stat.ppid != pid {
            children.entry(stat.ppid).or_default().push((pid, stat));
        } else {
            out.push((pid, stat));
        }
    }
    let mut i = 0;
    while i < out.len() {
        if let Some(kids) = children.remove(&out[i].0) {
            out.extend(kids);
        }
        i += 1;
    }
    // A parent loop (pid reuse between two reads) leaves stragglers.
    out.extend(children.into_values().flatten());
    out
}

pub(super) fn parse_stat(text: &str) -> Option<Stat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let f: Vec<&str> = text[close + 1..].split_whitespace().collect();
    let n = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok());
    Some(Stat {
        comm: text[open + 1..close].to_owned(),
        state: f.first()?.chars().next()?,
        nice: f.get(16)?.parse().ok()?,
        ppid: n(1)? as u32,
        ticks: n(11)? + n(12)?,
        threads: n(17)?,
        start: n(19)?,
    })
}

pub(super) fn status_kb(status: &str, key: &str) -> Option<u64> {
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

pub(super) fn io_bytes(io: &str) -> Option<u64> {
    let get = |k: &str| {
        io.lines()
            .find_map(|l| l.strip_prefix(k)?.trim().parse::<u64>().ok())
    };
    Some(get("read_bytes:")? + get("write_bytes:")?)
}

pub(super) fn split_nul(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

/// The systemd cgroup path: the unified hierarchy (v2), else the
/// name=systemd one of v1 and hybrid systems.
pub(super) fn cgroup_path(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("0::"))
        .or_else(|| {
            text.lines()
                .find_map(|l| l.split_once(":name=systemd:").map(|(_, p)| p))
        })
        .map(str::to_owned)
}

/// uid to login name, from /etc/passwd (cached; NSS users fall back to the uid).
#[derive(Default)]
pub(crate) struct Users(HashMap<u32, String>);

impl Users {
    pub(crate) fn name(&mut self, root: &Root, uid: u32) -> String {
        if self.0.is_empty() {
            for line in root.read("/etc/passwd").unwrap_or_default().lines() {
                let f: Vec<&str> = line.split(':').collect();
                if let (Some(name), Some(Ok(id))) = (f.first(), f.get(2).map(|s| s.parse::<u32>()))
                {
                    self.0.insert(id, (*name).to_owned());
                }
            }
        }
        self.0.get(&uid).cloned().unwrap_or_else(|| uid.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn cgroup_v1_and_v2() {
        assert_eq!(
            cgroup_path("0::/user.slice/app.slice/a.scope\n").as_deref(),
            Some("/user.slice/app.slice/a.scope")
        );
        assert_eq!(
            cgroup_path("12:cpu,cpuacct:/user.slice\n1:name=systemd:/user.slice/user-1000.slice/app.slice/b.service\n").as_deref(),
            Some("/user.slice/user-1000.slice/app.slice/b.service")
        );
        assert_eq!(cgroup_path("0::/\n").as_deref(), Some("/"));
    }
}

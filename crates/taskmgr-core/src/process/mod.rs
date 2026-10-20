//! Processes of the machine, resolved to their application and grouped.

mod control;
mod grouping;
mod io;
mod procfs;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

use serde::Serialize;

pub use control::{PriorityError, Signal, Target, send, set_priority, set_priority_as_admin};
use grouping::Entry;
pub use io::io_totals;
pub(crate) use procfs::Users;
use procfs::{ProcDir, cgroup_path, io_bytes, parents_first, parse_stat, split_nul, status_kb};

use crate::catalog::Catalog;
use crate::gpu::GpuSnapshot;
use crate::identity::{self, ENV_KEYS, Identity, Kind, ProcFacts, Role};
use crate::netflow::HelperData;
use crate::util::Root;
use crate::windows::Window;

#[derive(Clone, Debug, Serialize)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    /// Left out when normal, like the other defaults below: a thousand rows
    /// are parsed by the interface at each refresh.
    #[serde(skip_serializing_if = "Role::is_normal")]
    pub role: Role,
    pub cpu: f64,
    pub memory: u64,
    pub disk_bps: Option<f64>,
    pub threads: u64,
    /// Sent apart (`ProcessSampler::details`): long and nearly constant.
    #[serde(skip)]
    pub command: String,
    pub ppid: u32,
    /// Start time (clock ticks since boot): with the pid, the identity used
    /// for actions.
    pub start: u64,
    pub user: String,
    /// `R` running, `S` sleeping, `D` waiting on I/O, `T` stopped, `Z` zombie.
    pub state: char,
    #[serde(skip_serializing_if = "is_zero")]
    pub nice: i32,
    #[serde(skip)]
    pub exe: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<String>,
    /// Received and sent bytes per second; only while the root helper runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<(f64, f64)>,
    /// Keyed by GPU index, dedicated GPUs only.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub gpu: BTreeMap<usize, f64>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub gpu_memory: BTreeMap<usize, u64>,
    /// Video encoder and decoder use, in percent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<(f64, f64)>,
}

fn is_zero(n: &i32) -> bool {
    *n == 0
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
    pub windows: Vec<String>,
    pub net: Option<(f64, f64)>,
    pub gpu: BTreeMap<usize, f64>,
    pub gpu_memory: BTreeMap<usize, u64>,
    /// Video encoder and decoder use, in percent.
    pub video: Option<(f64, f64)>,
    pub processes: Vec<ProcessRow>,
}

struct Prev {
    start: u64,
    ticks: u64,
    io: Option<u64>,
}

const DETAILS_REFRESH: u64 = 10;

#[derive(Clone, Serialize)]
pub struct Details {
    pub command: String,
    pub exe: Option<String>,
}

impl Details {
    fn read(dir: &ProcDir) -> Self {
        let command = dir
            .read("cmdline")
            .map(|b| split_nul(&b).join(" "))
            .unwrap_or_default();
        Self {
            command: command.chars().take(400).collect(),
            exe: dir.link("exe"),
        }
    }
}

pub struct ProcessSampler {
    restricted: bool,
    users: Users,
    catalog: Catalog,
    prev: HashMap<u32, Prev>,
    /// With the cgroup, for the disk activity of unreadable processes.
    identities: HashMap<(u32, u64, String), (Identity, String)>,
    cgroup_io: io::CgroupIo,
    /// Command line and executable per (pid, start): re-read now and then
    /// (programs may retitle themselves), not every second.
    details: HashMap<(u32, u64), Details>,
    samples: u64,
    tick_hz: f64,
    cpus: f64,
}

/// The initial user namespace has a fixed inode number (PROC_USER_INIT_INO).
fn in_child_user_namespace(root: &Root) -> bool {
    std::fs::read_link(root.path("/proc/self/ns/user"))
        .is_ok_and(|l| l.to_string_lossy() != "user:[4026531837]")
}

impl ProcessSampler {
    pub fn new(root: &Root) -> Self {
        let hz = rustix::param::clock_ticks_per_second();
        Self {
            users: Users::default(),
            catalog: Catalog::load(root),
            prev: HashMap::new(),
            identities: HashMap::new(),
            cgroup_io: io::CgroupIo::default(),
            details: HashMap::new(),
            samples: 0,
            restricted: in_child_user_namespace(root),
            tick_hz: if hz > 0 { hz as f64 } else { 100.0 },
            cpus: std::thread::available_parallelism().map_or(1, |n| n.get()) as f64,
        }
    }

    fn facts(dir: &ProcDir, comm: &str) -> ProcFacts {
        let env = dir
            .read("environ")
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
            cmdline: dir
                .read("cmdline")
                .map(|b| split_nul(&b))
                .unwrap_or_default(),
            exe: dir.link("exe"),
            cgroup: dir
                .read("cgroup")
                .and_then(|c| cgroup_path(&String::from_utf8_lossy(&c)))
                .unwrap_or_default(),
            env,
        }
    }

    /// Command line and executable of every process seen in the last
    /// sample, by pid.
    /// Running in a user namespace of its own (appimage-run, bwrap): the
    /// kernel hides the environment, executable and I/O of the processes
    /// outside it.
    pub fn restricted(&self) -> bool {
        self.restricted
    }

    pub fn details(&self) -> HashMap<u32, &Details> {
        self.details.iter().map(|((pid, _), d)| (*pid, d)).collect()
    }

    pub fn sample(
        &mut self,
        root: &Root,
        secs: f64,
        windows: Option<&HashMap<u32, Vec<Window>>>,
        gpus: &[GpuSnapshot],
        helper: Option<&HelperData>,
    ) -> Vec<ProcessGroup> {
        let net = helper.map(|h| &h.net);
        let mut gpu_use: HashMap<u32, Vec<(usize, f64, Option<u64>)>> = HashMap::new();
        let mut video: HashMap<u32, (f64, f64)> = HashMap::new();
        // Without a dedicated card, the integrated one is what the
        // processes use.
        let only_integrated = gpus.iter().all(|g| !g.dedicated);
        for g in gpus.iter().filter(|g| g.dedicated || only_integrated) {
            for p in &g.processes {
                gpu_use
                    .entry(p.pid)
                    .or_default()
                    .push((g.index, p.usage, p.memory));
                if p.encoder.is_some() || p.decoder.is_some() {
                    let v = video.entry(p.pid).or_default();
                    v.0 += p.encoder.unwrap_or(0.0);
                    v.1 += p.decoder.unwrap_or(0.0);
                }
            }
        }
        let mut entries: Vec<Entry> = Vec::new();
        let mut seen = HashMap::new();
        let (mut path, mut text, mut status) = (String::new(), String::new(), String::new());
        let mut procs = Vec::new();
        // Whether no process of a cgroup has readable I/O counters.
        let mut cgroups: HashMap<String, bool> = HashMap::new();
        let mut entry_cgroups: Vec<String> = Vec::new();
        for entry in root.list("/proc") {
            let Ok(pid) = entry.parse::<u32>() else {
                continue;
            };
            path.clear();
            let _ = write!(path, "/proc/{pid}/stat");
            if let Some(stat) = root
                .read_into(&path, &mut text)
                .then(|| parse_stat(&text))
                .flatten()
                .filter(|s| pid != 2 && s.ppid != 2)
            {
                procs.push((pid, stat));
            }
        }
        // Parents first: a process inherits its launch context.
        let mut contexts: HashMap<u32, identity::Context> = HashMap::new();
        for (pid, listed) in parents_first(procs) {
            // Everything below comes from this one process: its stat is read
            // again through the open directory, the pid may have been reused.
            let Some(dir) = ProcDir::open(root, pid) else {
                continue;
            };
            let Some(stat) = dir
                .read_into("stat", &mut text)
                .then(|| parse_stat(&text))
                .flatten()
                .filter(|s| s.start == listed.start)
            else {
                continue;
            };
            dir.read_into("status", &mut status);
            let io = dir
                .read_into("io", &mut text)
                .then(|| io_bytes(&text))
                .flatten()
                .or_else(|| helper.and_then(|h| h.io.get(&pid).copied()));
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
            let (id, cgroup) = match self.identities.get(&key) {
                Some(known) => known.clone(),
                None => {
                    let facts = Self::facts(&dir, &stat.comm);
                    if facts.cmdline.is_empty() && facts.cgroup.is_empty() {
                        continue;
                    }
                    let parent = contexts.get(&stat.ppid);
                    let id = identity::resolve(&facts, parent, &mut self.catalog, root);
                    self.identities
                        .insert(key, (id.clone(), facts.cgroup.clone()));
                    (id, facts.cgroup)
                }
            };
            *cgroups.entry(cgroup.clone()).or_insert(true) &= io.is_none();
            contexts.insert(pid, id.context.clone());
            let refresh = self.samples.is_multiple_of(DETAILS_REFRESH);
            let details = self
                .details
                .entry((pid, stat.start))
                .and_modify(|d| {
                    if refresh {
                        *d = Details::read(&dir);
                    }
                })
                .or_insert_with(|| Details::read(&dir))
                .clone();
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
                command: details.command,
                ppid: stat.ppid,
                start: stat.start,
                user: status_kb(&status, "Uid")
                    .map(|uid| self.users.name(root, uid as u32))
                    .unwrap_or_default(),
                state: stat.state,
                nice: stat.nice,
                exe: details.exe,
                windows: Vec::new(),
                net: net.map(|n| n.get(&pid).copied().unwrap_or((0.0, 0.0))),
                gpu: gpu_use
                    .get(&pid)
                    .map(|v| v.iter().map(|(i, u, _)| (*i, *u)).collect())
                    .unwrap_or_default(),
                gpu_memory: gpu_use
                    .get(&pid)
                    .map(|v| v.iter().filter_map(|(i, _, m)| Some((*i, (*m)?))).collect())
                    .unwrap_or_default(),
                video: video.get(&pid).copied(),
            };
            entries.push(Entry {
                group: id.group,
                row,
                ppid: stat.ppid,
                launched: id.launched,
                cgroup_disk: None,
            });
            entry_cgroups.push(cgroup);
        }
        // Cgroups where no process could be read: their own counters, given
        // to the group of one of their processes.
        let blind: HashSet<&str> = cgroups
            .iter()
            .filter(|(_, b)| **b)
            .map(|(c, _)| c.as_str())
            .collect();
        let mut rates = self.cgroup_io.rates(root, &blind, secs);
        for (entry, cgroup) in entries.iter_mut().zip(&entry_cgroups) {
            entry.cgroup_disk = rates.remove(cgroup);
        }
        self.identities
            .retain(|(pid, start, _), _| seen.get(pid).is_some_and(|p| p.start == *start));
        self.details
            .retain(|(pid, start), _| seen.get(pid).is_some_and(|p| p.start == *start));
        self.samples += 1;
        self.prev = seen;
        grouping::join_launchers(&mut entries);
        grouping::group_by_tree(&mut entries);
        if let Some(windows) = windows {
            grouping::apply_windows(&mut entries, windows, &self.catalog);
        }
        grouping::collect(entries, windows.is_some())
    }
}

//! Processes of the machine, resolved to their application and grouped.

mod control;
mod grouping;
mod procfs;

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

pub use control::{PriorityError, Signal, Target, send, set_priority, set_priority_as_admin};
use grouping::Entry;
pub(crate) use procfs::Users;
use procfs::{cgroup_path, io_bytes, parse_stat, split_nul, status_kb};

use crate::catalog::Catalog;
use crate::gpu::GpuSnapshot;
use crate::identity::{self, ENV_KEYS, Identity, Kind, ProcFacts, Role};
use crate::util::Root;
use crate::windows::Window;

#[derive(Clone, Debug, Serialize)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
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
    fn read(root: &Root, pid: u32) -> Self {
        let command = std::fs::read(root.path(format!("/proc/{pid}/cmdline")))
            .map(|b| split_nul(&b).join(" "))
            .unwrap_or_default();
        Self {
            command: command.chars().take(400).collect(),
            exe: std::fs::read_link(root.path(format!("/proc/{pid}/exe")))
                .ok()
                .map(|p| p.to_string_lossy().into_owned()),
        }
    }
}

pub struct ProcessSampler {
    users: Users,
    catalog: Catalog,
    prev: HashMap<u32, Prev>,
    identities: HashMap<(u32, u64, String), Identity>,
    /// Command line and executable per (pid, start): re-read now and then
    /// (programs may retitle themselves), not every second.
    details: HashMap<(u32, u64), Details>,
    samples: u64,
    tick_hz: f64,
    cpus: f64,
}

impl ProcessSampler {
    pub fn new(root: &Root) -> Self {
        let hz = rustix::param::clock_ticks_per_second();
        Self {
            users: Users::default(),
            catalog: Catalog::load(root),
            prev: HashMap::new(),
            identities: HashMap::new(),
            details: HashMap::new(),
            samples: 0,
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
                .and_then(|c| cgroup_path(&c))
                .unwrap_or_default(),
            env,
        }
    }

    /// Command line and executable of every process seen in the last
    /// sample, by pid.
    pub fn details(&self) -> HashMap<u32, &Details> {
        self.details.iter().map(|((pid, _), d)| (*pid, d)).collect()
    }

    pub fn sample(
        &mut self,
        root: &Root,
        secs: f64,
        windows: Option<&HashMap<u32, Vec<Window>>>,
        gpus: &[GpuSnapshot],
        net: Option<&HashMap<u32, (f64, f64)>>,
    ) -> Vec<ProcessGroup> {
        let mut gpu_use: HashMap<u32, Vec<(usize, f64, Option<u64>)>> = HashMap::new();
        for g in gpus.iter().filter(|g| g.dedicated) {
            for p in &g.processes {
                gpu_use
                    .entry(p.pid)
                    .or_default()
                    .push((g.index, p.usage, p.memory));
            }
        }
        let mut entries: Vec<Entry> = Vec::new();
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
            let refresh = self.samples % DETAILS_REFRESH == 0;
            let details = self
                .details
                .entry((pid, stat.start))
                .and_modify(|d| {
                    if refresh {
                        *d = Details::read(root, pid);
                    }
                })
                .or_insert_with(|| Details::read(root, pid))
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
            };
            entries.push(Entry {
                group: id.group,
                row,
                ppid: stat.ppid,
            });
        }
        self.identities
            .retain(|(pid, start, _), _| seen.get(pid).is_some_and(|p| p.start == *start));
        self.details
            .retain(|(pid, start), _| seen.get(pid).is_some_and(|p| p.start == *start));
        self.samples += 1;
        self.prev = seen;
        grouping::group_by_tree(&mut entries);
        if let Some(windows) = windows {
            grouping::apply_windows(&mut entries, windows, &self.catalog);
        }
        grouping::collect(entries, windows.is_some())
    }
}

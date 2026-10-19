use std::collections::BTreeSet;

use serde::Serialize;

use crate::hwmon::{self, Chip};
use crate::util::{Charts, History, Root};

#[derive(Clone, Debug, Default, Serialize)]
pub struct CacheLevel {
    pub name: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CpuInfo {
    pub model: String,
    pub sockets: usize,
    pub cores: usize,
    pub logical: usize,
    pub max_mhz: Option<f64>,
    pub virtualization: Option<String>,
    pub caches: Vec<CacheLevel>,
    /// Per logical processor on hybrid Intel CPUs: "P" or "E"; empty otherwise.
    pub core_types: Vec<String>,
    /// Logical processors of each NUMA node; empty with a single node.
    pub numa: Vec<Vec<usize>>,
}

/// "0-3,8,10-11" to [0, 1, 2, 3, 8, 10, 11].
fn cpu_list(s: &str) -> Vec<usize> {
    s.trim()
        .split(',')
        .filter_map(|r| match r.split_once('-') {
            Some((a, b)) => Some((a.parse().ok()?..=b.parse().ok()?).collect::<Vec<usize>>()),
            None => r.parse().ok().map(|v| vec![v]),
        })
        .flatten()
        .collect()
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CpuSnapshot {
    #[serde(flatten)]
    pub info: CpuInfo,
    pub usage: f64,
    pub history: History,
    pub mhz: Option<f64>,
    pub processes: usize,
    pub threads: u64,
    pub uptime_secs: u64,
    pub temperature: Option<f64>,
    pub voltage: Option<f64>,
    /// "sensor" (hwmon driver) or "vid" (requested voltage read from MSRs).
    pub voltage_source: Option<&'static str>,
    /// Intel thermal throttling events since boot, when the kernel counts them.
    pub throttle_count: Option<u64>,
    pub core_history: Vec<History>,
    /// MHz per logical processor; empty without cpufreq.
    pub core_mhz_history: Vec<History>,
    pub charts: Charts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Times {
    busy: u64,
    total: u64,
}

fn parse_stat(stat: &str) -> Option<Times> {
    parse_line(stat.lines().find(|l| l.starts_with("cpu "))?)
}

/// Per logical processor, in `/proc/stat` order.
fn parse_cores(stat: &str) -> Vec<Times> {
    stat.lines()
        .filter(|l| l.starts_with("cpu") && !l.starts_with("cpu "))
        .filter_map(parse_line)
        .collect()
}

fn busy_percent(prev: Times, cur: Times) -> Option<f64> {
    (cur.total > prev.total).then(|| {
        cur.busy.saturating_sub(prev.busy) as f64 / (cur.total - prev.total) as f64 * 100.0
    })
}

/// Core voltage, only exposed by Super I/O and SMU drivers (nct6775, it87,
/// zenpower), never by k10temp.
fn core_voltage(root: &Root, chips: &[Chip]) -> Option<f64> {
    chips.iter().find_map(|c| {
        ["Vcore", "CPU Vcore", "SVI2_Core", "VDDCR_CPU"]
            .iter()
            .find_map(|l| c.input(root, "in", l))
            .map(|mv| mv as f64 / 1000.0)
    })
}

fn parse_line(line: &str) -> Option<Times> {
    let v: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse().ok())
        .collect();
    // user nice system idle iowait irq softirq steal (guest is already in user)
    let total: u64 = v.iter().take(8).sum();
    let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
    Some(Times {
        busy: total - idle,
        total,
    })
}

/// ARM cores have no "model name": decode implementer and part numbers.
fn arm_core_name(implementer: &str, part: &str) -> Option<String> {
    let part = u32::from_str_radix(part.trim_start_matches("0x"), 16).ok()?;
    let name = match (implementer, part) {
        ("0x41", 0xd03) => "Cortex-A53",
        ("0x41", 0xd04) => "Cortex-A35",
        ("0x41", 0xd05) => "Cortex-A55",
        ("0x41", 0xd07) => "Cortex-A57",
        ("0x41", 0xd08) => "Cortex-A72",
        ("0x41", 0xd09) => "Cortex-A73",
        ("0x41", 0xd0a) => "Cortex-A75",
        ("0x41", 0xd0b) => "Cortex-A76",
        ("0x41", 0xd0c) => "Neoverse-N1",
        ("0x41", 0xd0d) => "Cortex-A77",
        ("0x41", 0xd40) => "Neoverse-V1",
        ("0x41", 0xd41) => "Cortex-A78",
        ("0x41", 0xd44) => "Cortex-X1",
        ("0x41", 0xd46) => "Cortex-A510",
        ("0x41", 0xd47) => "Cortex-A710",
        ("0x41", 0xd48) => "Cortex-X2",
        ("0x41", 0xd49) => "Neoverse-N2",
        ("0x41", 0xd4f) => "Neoverse-V2",
        ("0x61", _) => return Some("Apple".to_owned()),
        ("0x51", _) => return Some("Qualcomm".to_owned()),
        ("0x41", _) => return Some(format!("ARM core 0x{part:03x}")),
        _ => return None,
    };
    Some(format!("ARM {name}"))
}

pub fn info(root: &Root) -> CpuInfo {
    let cpuinfo = root.read("/proc/cpuinfo").unwrap_or_default();
    let mut implementer: Option<String> = None;
    let mut part: Option<String> = None;
    let mut model = None;
    let mut logical = 0;
    let mut sockets = BTreeSet::new();
    let mut cores = BTreeSet::new();
    let mut physical = String::new();
    let mut flags = String::new();
    for line in cpuinfo.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "processor" => logical += 1,
            "model name" if model.is_none() => model = Some(v.to_owned()),
            "CPU implementer" => implementer = Some(v.to_owned()),
            "CPU part" if part.is_none() => part = Some(v.to_owned()),
            "physical id" => {
                physical = v.to_owned();
                sockets.insert(v.to_owned());
            }
            "core id" => {
                cores.insert((physical.clone(), v.to_owned()));
            }
            "flags" if flags.is_empty() => flags = v.to_owned(),
            _ => {}
        }
    }
    let virtualization = flags.split_whitespace().find_map(|f| match f {
        "svm" => Some("AMD-V".to_owned()),
        "vmx" => Some("VT-x".to_owned()),
        _ => None,
    });
    // amd_pstate reports the boost ceiling separately; cpuinfo_max_freq
    // drops to the base clock when boost is off for the policy.
    let max_mhz = ["amd_pstate_max_freq", "cpuinfo_max_freq"]
        .iter()
        .find_map(|f| root.read_u64(format!("/sys/devices/system/cpu/cpu0/cpufreq/{f}")))
        .map(|khz| khz as f64 / 1000.0);
    CpuInfo {
        model: model
            .or_else(|| arm_core_name(implementer.as_deref()?, part.as_deref()?))
            .or_else(|| {
                root.read_trimmed("/sys/firmware/devicetree/base/model")
                    .map(|m| m.trim_end_matches('\0').to_owned())
            })
            .unwrap_or_else(|| "CPU".to_owned()),
        sockets: sockets.len().max(1),
        cores: if cores.is_empty() {
            logical
        } else {
            cores.len()
        },
        logical,
        max_mhz,
        virtualization,
        caches: caches(root),
        core_types: {
            let p = root
                .read("/sys/devices/cpu_core/cpus")
                .map(|s| cpu_list(&s))
                .unwrap_or_default();
            let e = root
                .read("/sys/devices/cpu_atom/cpus")
                .map(|s| cpu_list(&s))
                .unwrap_or_default();
            if e.is_empty() {
                Vec::new()
            } else {
                (0..logical)
                    .map(|i| if p.contains(&i) { "P" } else { "E" }.to_owned())
                    .collect()
            }
        },
        numa: {
            let nodes: Vec<Vec<usize>> = root
                .list("/sys/devices/system/node")
                .into_iter()
                .filter(|n| {
                    n.strip_prefix("node")
                        .is_some_and(|i| i.parse::<u32>().is_ok())
                })
                .filter_map(|n| {
                    root.read(format!("/sys/devices/system/node/{n}/cpulist"))
                        .map(|s| cpu_list(&s))
                })
                .filter(|l| !l.is_empty())
                .collect();
            if nodes.len() > 1 { nodes } else { Vec::new() }
        },
    }
}

fn caches(root: &Root) -> Vec<CacheLevel> {
    let mut seen = BTreeSet::new();
    let mut totals: Vec<(String, u64)> = Vec::new();
    for cpu in root.list("/sys/devices/system/cpu") {
        if !cpu
            .strip_prefix("cpu")
            .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty())
        {
            continue;
        }
        let base = format!("/sys/devices/system/cpu/{cpu}/cache");
        for idx in root
            .list(&base)
            .into_iter()
            .filter(|i| i.starts_with("index"))
        {
            let d = format!("{base}/{idx}");
            let (Some(level), Some(kind), Some(size), Some(shared)) = (
                root.read_trimmed(format!("{d}/level")),
                root.read_trimmed(format!("{d}/type")),
                root.read_trimmed(format!("{d}/size")),
                root.read_trimmed(format!("{d}/shared_cpu_list")),
            ) else {
                continue;
            };
            if !seen.insert((level.clone(), kind.clone(), shared)) {
                continue;
            }
            let name = match kind.as_str() {
                "Data" => format!("L{level}d"),
                "Instruction" => format!("L{level}i"),
                _ => format!("L{level}"),
            };
            let bytes = parse_size(&size);
            match totals.iter_mut().find(|(n, _)| *n == name) {
                Some((_, b)) => *b += bytes,
                None => totals.push((name, bytes)),
            }
        }
    }
    let mut out: Vec<CacheLevel> = Vec::new();
    for (name, bytes) in totals {
        let name = name.trim_end_matches(['d', 'i']).to_owned();
        match out.iter_mut().find(|c| c.name == name) {
            Some(c) => c.bytes += bytes,
            None => out.push(CacheLevel { name, bytes }),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn parse_size(s: &str) -> u64 {
    let (num, mult) = match s.chars().last() {
        Some('K') => (&s[..s.len() - 1], 1024),
        Some('M') => (&s[..s.len() - 1], 1024 * 1024),
        _ => (s, 1),
    };
    num.parse::<u64>().unwrap_or(0) * mult
}

/// Current MHz of each logical processor (None when cpufreq is absent).
fn core_mhz(root: &Root, logical: usize) -> Vec<Option<f64>> {
    (0..logical)
        .map(|i| {
            root.read_u64(format!(
                "/sys/devices/system/cpu/cpu{i}/cpufreq/scaling_cur_freq"
            ))
            .map(|khz| khz as f64 / 1000.0)
        })
        .collect()
}

/// Processes from the /proc listing, threads from /proc/loadavg
/// ("running/total" scheduling entities), without opening a file per
/// process.
fn task_counts(root: &Root) -> (usize, u64) {
    let processes = root
        .list("/proc")
        .iter()
        .filter(|p| p.bytes().all(|b| b.is_ascii_digit()))
        .count();
    let threads = root
        .read("/proc/loadavg")
        .and_then(|s| s.split_whitespace().nth(3)?.split_once('/')?.1.parse().ok())
        .unwrap_or(0);
    (processes, threads)
}

pub struct CpuSampler {
    info: CpuInfo,
    prev: Option<Times>,
    history: History,
    prev_cores: Vec<Times>,
    cores: Vec<History>,
    core_mhz: Vec<History>,
    charts: Charts,
}

impl CpuSampler {
    pub fn new(root: &Root) -> Self {
        Self {
            info: info(root),
            prev: None,
            history: History::default(),
            prev_cores: Vec::new(),
            cores: Vec::new(),
            core_mhz: Vec::new(),
            charts: Charts::default(),
        }
    }

    /// `fallback_voltage`: from the root helper when no driver exposes it.
    pub fn sample(
        &mut self,
        root: &Root,
        chips: &[Chip],
        fallback_voltage: Option<f64>,
    ) -> CpuSnapshot {
        let stat = root.read("/proc/stat").unwrap_or_default();
        let cur = parse_stat(&stat);
        let usage = self
            .prev
            .zip(cur)
            .and_then(|(p, c)| busy_percent(p, c))
            .unwrap_or(0.0);
        if self.prev.is_some() {
            self.history.push(usage);
        }
        self.prev = cur;
        let cores = parse_cores(&stat);
        self.cores.resize_with(cores.len(), History::default);
        for (i, c) in cores.iter().enumerate() {
            if let Some(u) = self.prev_cores.get(i).and_then(|p| busy_percent(*p, *c)) {
                self.cores[i].push(u);
            }
        }
        self.prev_cores = cores;
        let per_core = core_mhz(root, self.info.logical);
        let known: Vec<f64> = per_core.iter().flatten().copied().collect();
        let mhz = (!known.is_empty()).then(|| known.iter().sum::<f64>() / known.len() as f64);
        if known.is_empty() {
            self.core_mhz.clear();
        } else {
            self.core_mhz.resize_with(per_core.len(), History::default);
            for (h, v) in self.core_mhz.iter_mut().zip(&per_core) {
                h.push(v.unwrap_or(0.0));
            }
        }
        let temperature = hwmon::cpu_temperature(root, chips);
        let (voltage, voltage_source) = match core_voltage(root, chips) {
            Some(v) => (Some(v), Some("sensor")),
            None => (fallback_voltage, fallback_voltage.map(|_| "vid")),
        };
        self.charts.push("temperature", temperature);
        self.charts.push("clock", mhz);
        self.charts.push("voltage", voltage);
        let (processes, threads) = task_counts(root);
        let uptime_secs = root
            .read("/proc/uptime")
            .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;
        CpuSnapshot {
            info: self.info.clone(),
            usage,
            history: self.history.clone(),
            mhz,
            processes,
            threads,
            uptime_secs,
            temperature,
            voltage,
            voltage_source,
            throttle_count: {
                let counts: Vec<u64> = (0..self.info.logical)
                    .filter_map(|i| {
                        root.read_u64(format!(
                            "/sys/devices/system/cpu/cpu{i}/thermal_throttle/core_throttle_count"
                        ))
                    })
                    .collect();
                (!counts.is_empty()).then(|| counts.iter().sum())
            },
            core_history: self.cores.clone(),
            core_mhz_history: self.core_mhz.clone(),
            charts: self.charts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_excludes_idle_and_iowait() {
        let t = parse_stat("cpu  10 0 5 80 5 0 0 0 0 0\ncpu0 1 2 3\n").unwrap();
        assert_eq!(
            t,
            Times {
                busy: 15,
                total: 100
            }
        );
    }

    #[test]
    fn arm_names() {
        assert_eq!(
            arm_core_name("0x41", "0xd09").as_deref(),
            Some("ARM Cortex-A73")
        );
        assert_eq!(arm_core_name("0x99", "0x1"), None);
    }

    #[test]
    fn cpu_lists() {
        assert_eq!(cpu_list("0-3,8,10-11\n"), vec![0, 1, 2, 3, 8, 10, 11]);
    }

    #[test]
    fn cache_sizes() {
        assert_eq!(parse_size("48K"), 48 * 1024);
        assert_eq!(parse_size("32M"), 32 * 1024 * 1024);
    }
}

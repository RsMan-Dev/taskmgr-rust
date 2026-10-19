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
    pub core_history: Vec<History>,
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

pub fn info(root: &Root) -> CpuInfo {
    let cpuinfo = root.read("/proc/cpuinfo").unwrap_or_default();
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
        model: model.unwrap_or_else(|| "Processeur".to_owned()),
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

fn average_mhz(root: &Root, logical: usize) -> Option<f64> {
    let khz: Vec<u64> = (0..logical)
        .filter_map(|i| {
            root.read_u64(format!(
                "/sys/devices/system/cpu/cpu{i}/cpufreq/scaling_cur_freq"
            ))
        })
        .collect();
    (!khz.is_empty()).then(|| khz.iter().sum::<u64>() as f64 / khz.len() as f64 / 1000.0)
}

fn task_counts(root: &Root) -> (usize, u64) {
    let mut processes = 0;
    let mut threads = 0;
    for pid in root.list("/proc") {
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Some(stat) = root.read(format!("/proc/{pid}/stat")) else {
            continue;
        };
        processes += 1;
        // Field 20 (num_threads), counted after the parenthesised comm.
        if let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) {
            threads += rest
                .split_whitespace()
                .nth(17)
                .and_then(|t| t.parse::<u64>().ok())
                .unwrap_or(0);
        }
    }
    (processes, threads)
}

pub struct CpuSampler {
    info: CpuInfo,
    prev: Option<Times>,
    history: History,
    prev_cores: Vec<Times>,
    cores: Vec<History>,
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
            charts: Charts::default(),
        }
    }

    pub fn sample(&mut self, root: &Root, chips: &[Chip]) -> CpuSnapshot {
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
        let mhz = average_mhz(root, self.info.logical);
        let temperature = hwmon::cpu_temperature(root, chips);
        let voltage = core_voltage(root, chips);
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
            core_history: self.cores.clone(),
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
    fn cache_sizes() {
        assert_eq!(parse_size("48K"), 48 * 1024);
        assert_eq!(parse_size("32M"), 32 * 1024 * 1024);
    }
}

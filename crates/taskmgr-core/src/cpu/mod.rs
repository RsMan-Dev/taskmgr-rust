use serde::Serialize;

use crate::hwmon::{self, Chip};
use crate::util::{Charts, History, Root};

mod info;
mod stat;

pub use info::{CacheLevel, CpuInfo, info};
use stat::{Times, busy_percent, parse_cores, parse_stat};

#[derive(Clone, Debug, Default, Serialize)]
pub struct CpuSnapshot {
    #[serde(flatten)]
    pub info: CpuInfo,
    pub usage: f64,
    pub history: History,
    /// Share of the time spent in the kernel, part of `usage`.
    pub kernel: f64,
    pub kernel_history: History,
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
    pub core_kernel_history: Vec<History>,
    /// MHz per logical processor; empty without cpufreq.
    pub core_mhz_history: Vec<History>,
    pub charts: Charts,
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
    kernel: History,
    prev_cores: Vec<Times>,
    cores: Vec<History>,
    core_kernel: Vec<History>,
    core_mhz: Vec<History>,
    charts: Charts,
}

impl CpuSampler {
    pub fn new(root: &Root) -> Self {
        Self {
            info: info(root),
            prev: None,
            history: History::default(),
            kernel: History::default(),
            prev_cores: Vec::new(),
            cores: Vec::new(),
            core_kernel: Vec::new(),
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
        let (usage, kernel) = self
            .prev
            .zip(cur)
            .and_then(|(p, c)| busy_percent(p, c))
            .unwrap_or((0.0, 0.0));
        if self.prev.is_some() {
            self.history.push(usage);
            self.kernel.push(kernel);
        }
        self.prev = cur;
        let cores = parse_cores(&stat);
        self.cores.resize_with(cores.len(), History::default);
        self.core_kernel.resize_with(cores.len(), History::default);
        for (i, c) in cores.iter().enumerate() {
            if let Some((u, k)) = self.prev_cores.get(i).and_then(|p| busy_percent(*p, *c)) {
                self.cores[i].push(u);
                self.core_kernel[i].push(k);
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
            kernel,
            kernel_history: self.kernel.clone(),
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
            core_kernel_history: self.core_kernel.clone(),
            core_mhz_history: self.core_mhz.clone(),
            charts: self.charts.clone(),
        }
    }
}

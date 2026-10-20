//! Benchmarks: CPU (one thread, all threads), memory bandwidth and
//! latency, disk throughput, and a stability test that catches
//! miscalculations. Each test also gives points against a reference
//! machine (see `score`). Results are kept in a history file.

pub mod cpu;
pub mod disk;
pub mod memory;
pub mod score;
pub mod workloads;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CpuScore {
    /// Points; before version 1, millions of operations per second.
    pub single: f64,
    pub multi: f64,
    pub threads: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StabilityResult {
    pub seconds: u64,
    pub blocks: u64,
    pub errors: u64,
}

/// One finished run, as kept in the history.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Run {
    /// Seconds since the epoch.
    pub time: u64,
    /// cpu, memory, disk or stability.
    pub kind: String,
    pub cpu_model: String,
    /// Test method (`score::VERSION`); 0 for runs made before scores.
    #[serde(default)]
    pub version: u32,
    /// Share of the processor other programs used during the test.
    #[serde(default)]
    pub background: f64,
    /// Points of a memory or disk test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuScore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<DiskRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability: Option<StabilityResult>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub read: f64,
    pub write: f64,
    pub copy: f64,
    pub latency_ns: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiskRecord {
    pub folder: String,
    pub write: f64,
    pub read: f64,
    pub random_iops: f64,
    #[serde(default)]
    pub random_iops_parallel: f64,
    /// Reads bypassed the page cache.
    #[serde(default)]
    pub direct: bool,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Median calls per second of each workload, after a second of warm-up
/// that lets the clock settle.
fn workload_rates(
    threads: usize,
    pin: Option<usize>,
    stop: &AtomicBool,
    progress: &dyn Fn(f64),
) -> Vec<f64> {
    cpu::rate(
        workloads::integer,
        threads,
        pin,
        Duration::from_secs(1),
        stop,
    );
    let total = (workloads::ALL.len() * 3) as f64;
    let mut done = 0.0;
    workloads::ALL
        .iter()
        .map(|(_, work)| {
            score::median(
                (0..3)
                    .map(|_| {
                        let r = cpu::rate(*work, threads, pin, Duration::from_millis(1500), stop);
                        done += 1.0;
                        progress(done / total);
                        r
                    })
                    .collect(),
            )
        })
        .collect()
}

/// Points on the fastest core, then on all of them (about 40 seconds).
pub fn run_cpu(root: &crate::Root, stop: &AtomicBool, progress: &dyn Fn(f64)) -> CpuScore {
    let threads = threads();
    let single = workload_rates(1, cpu::best_core(root), stop, &|f| progress(f * 0.5));
    let multi = workload_rates(threads, None, stop, &|f| progress(0.5 + f * 0.5));
    CpuScore {
        single: score::points(&single, &score::Reference::CPU_SINGLE, &[]),
        multi: score::points(&multi, &score::Reference::CPU_MULTI, &[]),
        threads,
    }
}

/// Eight times the last cache level, at least 512 MiB, at most half the
/// available memory. None when that memory cannot hold four times the
/// cache: the test would measure the cache.
pub fn memory_size(available: u64, cache: u64) -> Option<usize> {
    let size = (cache * 8).max(512 << 20).min(available / 2);
    (size >= cache * 4).then_some(size as usize)
}

/// MemAvailable, in bytes.
pub fn available_memory(root: &crate::Root) -> u64 {
    root.read("/proc/meminfo")
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("MemAvailable:"))?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })
        .map_or(0, |kb| kb * 1024)
}

pub fn run_memory(
    root: &crate::Root,
    stop: &AtomicBool,
    progress: &dyn Fn(f64),
) -> Result<(MemoryRecord, f64), String> {
    let size = memory_size(available_memory(root), memory::last_level_cache(root))
        .ok_or("Not enough free memory to test past the processor cache.")?;
    let s = memory::run(size, threads(), stop, progress);
    let points = score::points(
        &[s.read, s.write, s.copy, s.latency_ns],
        &score::Reference::MEMORY,
        &[false, false, false, true],
    );
    let record = MemoryRecord {
        read: s.read,
        write: s.write,
        copy: s.copy,
        latency_ns: s.latency_ns,
    };
    Ok((record, points))
}

pub fn run_disk(
    folder: &std::path::Path,
    stop: &AtomicBool,
    progress: &dyn Fn(f64),
) -> Result<(DiskRecord, f64), String> {
    let s = disk::run(folder, 1 << 30, stop, progress)?;
    let points = score::points(
        &[s.write, s.read, s.random_iops, s.random_iops_parallel],
        &score::Reference::DISK,
        &[],
    );
    let record = DiskRecord {
        folder: s.folder,
        write: s.write,
        read: s.read,
        random_iops: s.random_iops,
        random_iops_parallel: s.random_iops_parallel,
        direct: s.direct,
    };
    Ok((record, points))
}

fn history_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME").map_or_else(
        || PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share"),
        PathBuf::from,
    );
    base.join("taskmgr/benchmarks.json")
}

pub fn history() -> Vec<Run> {
    std::fs::read_to_string(history_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Appends a run; the newest 200 are kept.
pub fn record(run: Run) -> Result<(), String> {
    let path = history_path();
    let mut list = history();
    list.push(run);
    let excess = list.len().saturating_sub(200);
    list.drain(..excess);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

pub fn clear_history() -> Result<(), String> {
    match std::fs::remove_file(history_path()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

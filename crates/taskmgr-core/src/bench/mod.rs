//! Benchmarks for overclockers: CPU score (one thread, all threads),
//! memory bandwidth and latency, disk throughput, and a stability test that
//! catches miscalculations. Scores are kept in a history file.

pub mod cpu;
pub mod disk;
pub mod memory;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CpuScore {
    /// Millions of operations per second.
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
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Single then multi-threaded score, 5 seconds each.
pub fn run_cpu(stop: &AtomicBool, progress: &dyn Fn(f64)) -> CpuScore {
    let threads = threads();
    progress(0.0);
    let single = cpu::score(1, Duration::from_secs(5), stop);
    progress(0.5);
    let multi = cpu::score(threads, Duration::from_secs(5), stop);
    progress(1.0);
    CpuScore {
        single,
        multi,
        threads,
    }
}

/// A quarter of the available memory, at most 1 GiB, at least 64 MiB.
pub fn memory_size(available: u64) -> usize {
    (available / 4).clamp(64 << 20, 1 << 30) as usize
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

pub fn run_memory(available: u64, stop: &AtomicBool, progress: &dyn Fn(f64)) -> MemoryRecord {
    let s = memory::run(memory_size(available), threads(), stop, progress);
    MemoryRecord {
        read: s.read,
        write: s.write,
        copy: s.copy,
        latency_ns: s.latency_ns,
    }
}

pub fn run_disk(
    folder: &std::path::Path,
    stop: &AtomicBool,
    progress: &dyn Fn(f64),
) -> Result<DiskRecord, String> {
    let s = disk::run(folder, 1 << 30, stop, progress)?;
    Ok(DiskRecord {
        folder: s.folder,
        write: s.write,
        read: s.read,
        random_iops: s.random_iops,
    })
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

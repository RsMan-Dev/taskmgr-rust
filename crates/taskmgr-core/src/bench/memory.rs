//! Memory bandwidth (all threads) and latency (dependent random reads).

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use std::collections::HashMap;

use serde::Serialize;

use super::score::median;
use crate::util::Root;

#[derive(Clone, Debug, Default, Serialize)]
pub struct MemoryScore {
    /// Gigabytes per second.
    pub read: f64,
    pub write: f64,
    pub copy: f64,
    /// Nanoseconds per random access, past the CPU caches.
    pub latency_ns: f64,
}

/// Repeats `pass` over every chunk on its own thread until `duration`;
/// returns the gigabytes per second it went through.
fn bandwidth(
    chunks: &mut [&mut [u64]],
    duration: Duration,
    stop: &AtomicBool,
    pass: impl Fn(&mut [u64]) + Sync,
) -> f64 {
    let started = Instant::now();
    let moved: f64 = std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .iter_mut()
            .map(|chunk| {
                let pass = &pass;
                s.spawn(move || {
                    let mut passes = 0u64;
                    while started.elapsed() < duration && !stop.load(Ordering::Relaxed) {
                        pass(chunk);
                        passes += 1;
                    }
                    passes as f64 * chunk.len() as f64 * 8.0
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).sum()
    });
    moved / started.elapsed().as_secs_f64() / 1e9
}

/// Total size of the last cache level, each shared cache counted once
/// (96 + 32 MB on a 9950X3D, whose two dies have their own).
pub fn last_level_cache(root: &Root) -> u64 {
    let mut caches: HashMap<String, (u32, u64)> = HashMap::new();
    for cpu in root.list("/sys/devices/system/cpu") {
        let base = format!("/sys/devices/system/cpu/{cpu}/cache");
        for index in root.list(&base) {
            let dir = format!("{base}/{index}");
            let level: u32 = root
                .read_trimmed(format!("{dir}/level"))
                .and_then(|l| l.parse().ok())
                .unwrap_or(0);
            let size = root
                .read_trimmed(format!("{dir}/size"))
                .and_then(|s| s.strip_suffix('K').and_then(|k| k.parse::<u64>().ok()))
                .map_or(0, |k| k * 1024);
            let shared = root
                .read_trimmed(format!("{dir}/shared_cpu_list"))
                .unwrap_or_default();
            caches.insert(format!("{level}:{shared}"), (level, size));
        }
    }
    let top = caches.values().map(|(l, _)| *l).max().unwrap_or(0);
    caches
        .values()
        .filter(|(l, _)| *l == top)
        .map(|(_, s)| s)
        .sum()
}

const PASSES: usize = 3;

/// `bytes`: buffer size, several times the last cache level so that the
/// figures are the memory's, not the cache's. Each figure is the median
/// of three passes.
pub fn run(bytes: usize, threads: usize, stop: &AtomicBool, progress: &dyn Fn(f64)) -> MemoryScore {
    let words = (bytes / 8).max(1 << 16);
    let mut buffer = vec![1u64; words];
    let per = words.div_ceil(threads.max(1));
    let step = Duration::from_millis(1500);
    let mut chunks: Vec<&mut [u64]> = buffer.chunks_mut(per).collect();
    let mut figures = [Vec::new(), Vec::new(), Vec::new()];
    for pass in 0..PASSES {
        figures[0].push(bandwidth(&mut chunks, step, stop, |c| {
            black_box(c.iter().fold(0u64, |a, v| a.wrapping_add(*v)));
        }));
        figures[1].push(bandwidth(&mut chunks, step, stop, |c| c.fill(black_box(7))));
        figures[2].push(bandwidth(&mut chunks, step, stop, |c| {
            let half = c.len() / 2;
            let (a, b) = c.split_at_mut(half);
            b[..half].copy_from_slice(a);
        }));
        progress((pass + 1) as f64 / PASSES as f64 * 0.7);
    }
    drop(chunks);
    let latencies: Vec<f64> = (0..PASSES)
        .map(|pass| {
            let ns = latency(&mut buffer, pass as u64, stop);
            progress(0.7 + (pass + 1) as f64 / PASSES as f64 * 0.3);
            ns
        })
        .collect();
    let [read, write, copy] = figures.map(median);
    MemoryScore {
        read,
        write,
        copy,
        latency_ns: median(latencies),
    }
}

/// Follows a random cycle through the buffer: each read waits for the
/// previous one, so the time per step is the memory latency.
fn latency(buffer: &mut [u64], seed: u64, stop: &AtomicBool) -> f64 {
    let n = buffer.len();
    let mut order: Vec<usize> = (0..n).collect();
    let mut x = 0x2545_F491_4F6C_DD1Du64 ^ (seed << 32);
    for i in (1..n).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        order.swap(i, (x % (i as u64 + 1)) as usize);
    }
    for w in order.windows(2) {
        buffer[w[0]] = w[1] as u64;
    }
    if let (Some(last), Some(first)) = (order.last(), order.first()) {
        buffer[*last] = *first as u64;
    }
    let steps = 10_000_000.min(n * 4);
    let started = Instant::now();
    let mut at = order.first().copied().unwrap_or(0);
    for i in 0..steps {
        if i % 1_000_000 == 0 && stop.load(Ordering::Relaxed) {
            break;
        }
        at = buffer.get(at).copied().unwrap_or(0) as usize;
    }
    black_box(at);
    started.elapsed().as_secs_f64() * 1e9 / steps as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_run_gives_positive_figures() {
        let stop = AtomicBool::new(false);
        let mut buffer = vec![0u64; 1 << 16];
        assert!(latency(&mut buffer, 0, &stop) > 0.0);
        let mut chunks: Vec<&mut [u64]> = buffer.chunks_mut(1 << 15).collect();
        let gbs = bandwidth(&mut chunks, Duration::from_millis(50), &stop, |c| c.fill(1));
        assert!(gbs > 0.0);
    }

    #[test]
    fn counts_each_shared_cache_once() {
        let base = std::env::temp_dir().join(format!("taskmgr-cache-{}", std::process::id()));
        for (cpu, size, shared) in [
            (0, "98304K", "0-1"),
            (1, "98304K", "0-1"),
            (2, "32768K", "2-3"),
        ] {
            for (index, level, size) in [("index0", "1", "48K"), ("index3", "3", size)] {
                let dir = base.join(format!("sys/devices/system/cpu/cpu{cpu}/cache/{index}"));
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("level"), level).unwrap();
                std::fs::write(dir.join("size"), size).unwrap();
                std::fs::write(dir.join("shared_cpu_list"), format!("{shared}:{index}")).unwrap();
            }
        }
        assert_eq!(last_level_cache(&Root::new(&base)), (96 + 32) << 20);
        std::fs::remove_dir_all(&base).unwrap();
    }
}

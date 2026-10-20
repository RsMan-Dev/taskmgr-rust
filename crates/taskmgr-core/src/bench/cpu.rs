//! CPU throughput, and a stability test whose results are known in advance.

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustix::thread::{CpuSet, sched_setaffinity};

use crate::util::Root;

/// Iterations per block: about a millisecond on a current core.
pub const BLOCK: u64 = 200_000;

pub fn kernel(seed: u64, iterations: u64) -> u64 {
    let mut x = seed | 1;
    let mut acc = 0u64;
    let mut f = 1.0f64;
    for _ in 0..iterations {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        acc = acc.wrapping_add(x.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        f = f.mul_add(1.000_000_1, (x & 0xff) as f64 * 1e-9);
    }
    acc ^ f.to_bits()
}

/// The core the firmware rates fastest. On processors whose cores differ
/// (hybrid, or one die with more cache), a one-thread score would else
/// depend on where the scheduler puts it.
pub fn best_core(root: &Root) -> Option<usize> {
    let rating = |cpu: usize, file: &str| -> Option<u64> {
        root.read_trimmed(format!("/sys/devices/system/cpu/cpu{cpu}/{file}"))?
            .parse()
            .ok()
    };
    let cpus = super::threads();
    [
        "acpi_cppc/highest_perf",
        "cpufreq/amd_pstate_prefcore_ranking",
        "cpufreq/cpuinfo_max_freq",
    ]
    .iter()
    .find_map(|file| {
        let rated: Vec<(usize, u64)> = (0..cpus)
            .filter_map(|c| Some((c, rating(c, file)?)))
            .collect();
        let top = rated.iter().map(|(_, r)| *r).max()?;
        rated.iter().find(|(_, r)| *r == top).map(|(c, _)| *c)
    })
}

/// Calls of `work` per second, on `threads` threads at once; one thread
/// runs on `pin` when given.
pub fn rate(
    work: fn(u64) -> u64,
    threads: usize,
    pin: Option<usize>,
    duration: Duration,
    stop: &AtomicBool,
) -> f64 {
    let done = AtomicU64::new(0);
    let started = Instant::now();
    std::thread::scope(|s| {
        for t in 0..threads.max(1) {
            let done = &done;
            s.spawn(move || {
                if let Some(core) = pin.filter(|_| threads == 1) {
                    let mut set = CpuSet::new();
                    set.set(core);
                    let _ = sched_setaffinity(None, &set);
                }
                let (mut seed, mut calls) = (t as u64 * 1_000_003 + 1, 0);
                while started.elapsed() < duration && !stop.load(Ordering::Relaxed) {
                    black_box(work(black_box(seed)));
                    seed += 1;
                    calls += 1;
                }
                done.fetch_add(calls, Ordering::Relaxed);
            });
        }
    });
    done.load(Ordering::Relaxed) as f64 / started.elapsed().as_secs_f64()
}

/// Live state of a stability test, read by the interface.
#[derive(Default)]
pub struct Stability {
    pub blocks: AtomicU64,
    pub errors: AtomicU64,
}

const SEEDS: u64 = 64;

/// Every thread flat out until `stop`: an unstable overclock miscalculates
/// before it crashes.
pub fn stability(threads: usize, state: &Stability, stop: &AtomicBool) {
    let expected: Vec<u64> = (0..SEEDS).map(|s| kernel(s + 1, BLOCK)).collect();
    std::thread::scope(|s| {
        for t in 0..threads.max(1) {
            let expected = &expected;
            s.spawn(move || {
                let mut i = t as u64;
                while !stop.load(Ordering::Relaxed) {
                    let seed = i % SEEDS;
                    if black_box(kernel(black_box(seed + 1), BLOCK)) != expected[seed as usize] {
                        state.errors.fetch_add(1, Ordering::Relaxed);
                    }
                    state.blocks.fetch_add(1, Ordering::Relaxed);
                    i += 1;
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_is_deterministic() {
        assert_eq!(kernel(7, 1000), kernel(7, 1000));
        assert_ne!(kernel(7, 1000), kernel(8, 1000));
    }

    #[test]
    fn stability_finds_no_error_on_a_sound_cpu() {
        let state = Stability::default();
        let stop = AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| stability(2, &state, &stop));
            std::thread::sleep(Duration::from_millis(200));
            stop.store(true, Ordering::Relaxed);
        });
        assert!(state.blocks.load(Ordering::Relaxed) > 0);
        assert_eq!(state.errors.load(Ordering::Relaxed), 0);
    }
}

//! CPU score and stability test, on a deterministic integer and floating
//! point workload whose exact result is known in advance.

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Iterations per block: about a millisecond on a current core.
pub const BLOCK: u64 = 200_000;

/// One block of work; the same seed always gives the same result.
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

/// Millions of kernel iterations per second on `threads` threads.
pub fn score(threads: usize, duration: Duration, stop: &AtomicBool) -> f64 {
    let done = AtomicU64::new(0);
    let started = Instant::now();
    std::thread::scope(|s| {
        for t in 0..threads.max(1) {
            let done = &done;
            s.spawn(move || {
                let mut seed = t as u64 + 1;
                while started.elapsed() < duration && !stop.load(Ordering::Relaxed) {
                    black_box(kernel(black_box(seed), BLOCK));
                    seed += 1;
                    done.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    let secs = started.elapsed().as_secs_f64();
    (done.load(Ordering::Relaxed) * BLOCK) as f64 / secs / 1e6
}

/// Live state of a stability test, read by the interface.
#[derive(Default)]
pub struct Stability {
    pub blocks: AtomicU64,
    pub errors: AtomicU64,
}

const SEEDS: u64 = 64;

/// Runs every thread flat out until `stop`, checking each block against a
/// result computed once beforehand: an unstable overclock miscalculates
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

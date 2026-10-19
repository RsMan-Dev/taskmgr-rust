//! Memory bandwidth (all threads) and latency (dependent random reads).

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

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

/// `bytes`: buffer size, well above the last-level cache.
pub fn run(bytes: usize, threads: usize, stop: &AtomicBool, progress: &dyn Fn(f64)) -> MemoryScore {
    let words = (bytes / 8).max(1 << 16);
    let mut buffer = vec![1u64; words];
    let per = words.div_ceil(threads.max(1));
    let step = Duration::from_secs(2);

    let mut chunks: Vec<&mut [u64]> = buffer.chunks_mut(per).collect();
    let read = bandwidth(&mut chunks, step, stop, |c| {
        black_box(c.iter().fold(0u64, |a, v| a.wrapping_add(*v)));
    });
    progress(0.25);
    let write = bandwidth(&mut chunks, step, stop, |c| c.fill(black_box(7)));
    progress(0.5);
    let copy = bandwidth(&mut chunks, step, stop, |c| {
        let half = c.len() / 2;
        let (a, b) = c.split_at_mut(half);
        b[..half].copy_from_slice(a);
    });
    progress(0.75);
    drop(chunks);
    let latency_ns = latency(&mut buffer, stop);
    progress(1.0);
    MemoryScore {
        read,
        write,
        copy,
        latency_ns,
    }
}

/// Follows a random cycle through the buffer: each read waits for the
/// previous one, so the time per step is the memory latency.
fn latency(buffer: &mut [u64], stop: &AtomicBool) -> f64 {
    let n = buffer.len();
    let mut order: Vec<usize> = (0..n).collect();
    let mut x = 0x2545_F491_4F6C_DD1Du64;
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
    let steps = 20_000_000.min(n * 4);
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
        assert!(latency(&mut buffer, &stop) > 0.0);
        let mut chunks: Vec<&mut [u64]> = buffer.chunks_mut(1 << 15).collect();
        let gbs = bandwidth(&mut chunks, Duration::from_millis(50), &stop, |c| c.fill(1));
        assert!(gbs > 0.0);
    }
}

//! Points against a fixed reference machine (1000 = as fast). Never change
//! the reference: a new test method gets a new `VERSION` instead.

use std::time::Instant;

pub const VERSION: u32 = 1;

/// Ryzen 9 9950X3D, 64 GB DDR5-6400, Samsung 980 PRO (ext4), Linux 6.18.
pub struct Reference;

impl Reference {
    pub const CPU_SINGLE: [f64; 4] = [4351.0, 5997.0, 5486.0, 9355.0];
    pub const CPU_MULTI: [f64; 4] = [123_124.0, 89_448.0, 109_185.0, 199_548.0];
    /// Read, write, copy (GB/s), latency (ns).
    pub const MEMORY: [f64; 4] = [86.41, 41.26, 58.08, 87.07];
    /// Write, read (MB/s), random reads on one and on eight threads (IOPS).
    pub const DISK: [f64; 4] = [1113.0, 4010.0, 15_074.0, 114_163.0];
}

/// 1000 times the geometric mean of the ratios; `lower`: lower is better.
pub fn points(values: &[f64], reference: &[f64], lower: &[bool]) -> f64 {
    let logs: Vec<f64> = values
        .iter()
        .zip(reference)
        .zip(lower.iter().chain(std::iter::repeat(&false)))
        .filter(|((v, r), _)| **v > 0.0 && **r > 0.0)
        .map(|((v, r), low)| if *low { (r / v).ln() } else { (v / r).ln() })
        .collect();
    if logs.is_empty() {
        return 0.0;
    }
    1000.0 * (logs.iter().sum::<f64>() / logs.len() as f64).exp()
}

pub fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

fn busy_ticks() -> Option<(u64, u64)> {
    let total: u64 = std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .next()?
        .split_whitespace()
        .skip(1)
        .take(3)
        .filter_map(|v| v.parse::<u64>().ok())
        .sum();
    let own = std::fs::read_to_string("/proc/self/stat").ok()?;
    let own: u64 = own[own.rfind(')')? + 1..]
        .split_whitespace()
        .skip(11)
        .take(2)
        .filter_map(|v| v.parse::<u64>().ok())
        .sum();
    Some((total, own))
}

/// How busy other programs keep the processor during a test.
pub struct Background {
    started: Instant,
    ticks: Option<(u64, u64)>,
}

impl Background {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
            ticks: busy_ticks(),
        }
    }

    pub fn share(&self) -> f64 {
        let (Some((t0, o0)), Some((t1, o1))) = (self.ticks, busy_ticks()) else {
            return 0.0;
        };
        let hz = rustix::param::clock_ticks_per_second() as f64;
        let cpus = super::threads() as f64;
        let available = self.started.elapsed().as_secs_f64() * hz * cpus;
        let others = t1.saturating_sub(t0).saturating_sub(o1.saturating_sub(o0)) as f64;
        (others / available.max(1.0)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_are_a_geometric_mean_against_the_reference() {
        assert_eq!(points(&[2.0, 2.0], &[1.0, 1.0], &[]).round(), 2000.0);
        assert_eq!(points(&[4.0, 1.0], &[1.0, 1.0], &[]).round(), 2000.0);
        // Half the latency counts as twice as fast.
        assert_eq!(points(&[0.5], &[1.0], &[true]).round(), 2000.0);
        assert_eq!(points(&[], &[], &[]), 0.0);
    }

    #[test]
    fn median_of_three() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
    }
}

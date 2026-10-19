use serde::Serialize;

use crate::util::{History, Root};

#[derive(Clone, Debug, Default, Serialize)]
pub struct MemorySnapshot {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub cached: u64,
    pub committed: u64,
    pub commit_limit: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub usage: f64,
    pub history: History,
}

fn field(meminfo: &str, key: &str) -> u64 {
    meminfo
        .lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .unwrap_or(0)
        * 1024
}

#[derive(Default)]
pub struct MemorySampler {
    history: History,
}

impl MemorySampler {
    pub fn sample(&mut self, root: &Root) -> MemorySnapshot {
        let m = root.read("/proc/meminfo").unwrap_or_default();
        let total = field(&m, "MemTotal");
        let available = field(&m, "MemAvailable");
        let used = total.saturating_sub(available);
        let usage = if total > 0 {
            used as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        self.history.push(usage);
        let swap_total = field(&m, "SwapTotal");
        MemorySnapshot {
            total,
            used,
            available,
            cached: field(&m, "Cached") + field(&m, "Buffers") + field(&m, "SReclaimable"),
            committed: field(&m, "Committed_AS"),
            commit_limit: field(&m, "CommitLimit"),
            swap_total,
            swap_used: swap_total.saturating_sub(field(&m, "SwapFree")),
            usage,
            history: self.history.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_fields_in_bytes() {
        let m = "MemTotal:       1000 kB\nMemAvailable:    250 kB\n";
        assert_eq!(field(m, "MemTotal"), 1_024_000);
        assert_eq!(field(m, "MemAvailable"), 256_000);
        assert_eq!(field(m, "Missing"), 0);
    }
}

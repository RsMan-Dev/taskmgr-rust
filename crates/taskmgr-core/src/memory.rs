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
    pub dimms: Vec<Dimm>,
    pub slots: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Dimm {
    pub locator: String,
    pub bank: Option<String>,
    pub size: u64,
    pub kind: Option<String>,
    pub speed_mts: Option<u64>,
    pub manufacturer: Option<String>,
    pub part: Option<String>,
}

/// Firmware often leaves the vendor empty; part numbers carry it.
fn vendor_from_part(part: &str) -> Option<&'static str> {
    const PREFIXES: [(&str, &str); 8] = [
        ("CM", "Corsair"),
        ("F4-", "G.Skill"),
        ("F5-", "G.Skill"),
        ("KF", "Kingston Fury"),
        ("KVR", "Kingston"),
        ("CT", "Crucial"),
        ("BL", "Crucial Ballistix"),
        ("M3", "Samsung"),
    ];
    PREFIXES
        .iter()
        .find(|(p, _)| part.starts_with(p))
        .map(|(_, v)| *v)
}

/// Memory slots from udev's dmi_memory_id, readable without root.
fn dimms(root: &Root) -> (Vec<Dimm>, usize) {
    let known = |v: Option<String>| {
        v.filter(|s| !matches!(s.as_str(), "Unknown" | "Not Specified" | "None"))
    };
    let prop =
        |i: usize, k: &str| known(root.udev_property("+dmi:id", &format!("MEMORY_DEVICE_{i}_{k}")));
    let slots = (0..32)
        .take_while(|i| prop(*i, "LOCATOR").is_some())
        .count();
    let list = (0..slots)
        .filter_map(|i| {
            let size: u64 = prop(i, "SIZE")?.parse().ok()?;
            let part = prop(i, "PART_NUMBER");
            Some(Dimm {
                locator: prop(i, "LOCATOR").unwrap_or_default(),
                bank: prop(i, "BANK_LOCATOR"),
                size,
                kind: prop(i, "TYPE"),
                speed_mts: prop(i, "CONFIGURED_SPEED_MTS")
                    .or_else(|| prop(i, "SPEED_MTS"))
                    .and_then(|s| s.parse().ok()),
                manufacturer: prop(i, "MANUFACTURER").or_else(|| {
                    part.as_deref()
                        .and_then(vendor_from_part)
                        .map(str::to_owned)
                }),
                part,
            })
        })
        .collect();
    (list, slots)
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
    dimms: Option<(Vec<Dimm>, usize)>,
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
        let (dimms, slots) = self.dimms.get_or_insert_with(|| dimms(root)).clone();
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
            dimms,
            slots,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendors() {
        assert_eq!(vendor_from_part("CMH64GX5M2B6400C32"), Some("Corsair"));
        assert_eq!(vendor_from_part("XYZ"), None);
    }

    #[test]
    fn meminfo_fields_in_bytes() {
        let m = "MemTotal:       1000 kB\nMemAvailable:    250 kB\n";
        assert_eq!(field(m, "MemTotal"), 1_024_000);
        assert_eq!(field(m, "MemAvailable"), 256_000);
        assert_eq!(field(m, "Missing"), 0);
    }
}

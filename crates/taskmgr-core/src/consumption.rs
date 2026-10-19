//! Power drawn by everything that reports it: RAPL domains (CPU package,
//! cores, memory, platform), GPUs, batteries and other hwmon power sensors.
//! Disks and memory modules publish nothing; they cannot be listed.

use std::collections::HashMap;

use serde::Serialize;

use crate::Root;
use crate::battery::BatterySnapshot;
use crate::gpu::GpuSnapshot;
use crate::hwmon::Chip;
use crate::util::History;

#[derive(Clone, Debug, Default, Serialize)]
pub struct PowerSource {
    pub key: String,
    pub name: String,
    /// cpu, cpu-cores, cpu-uncore, memory, platform, gpu, battery, sensor.
    pub kind: &'static str,
    pub watts: Option<f64>,
    pub history: History,
    /// Energy since the window opened.
    pub energy_wh: f64,
    /// Key of the source this one is a part of (cores within the package),
    /// so totals do not count it twice.
    pub part_of: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ConsumptionSnapshot {
    pub sources: Vec<PowerSource>,
    pub total_watts: Option<f64>,
    /// platform (whole machine), battery (whole laptop) or sum (parts).
    pub total_kind: &'static str,
    pub total_history: History,
    pub total_wh: f64,
    /// RAPL exists but its counters are root-only.
    pub denied: bool,
}

#[derive(Default)]
pub struct ConsumptionSampler {
    counters: HashMap<String, u64>,
    history: HashMap<String, History>,
    energy: HashMap<String, f64>,
    total_history: History,
    total_wh: f64,
}

const POWERCAP: &str = "/sys/class/powercap";

fn rapl_kind(name: &str) -> &'static str {
    match name {
        n if n.starts_with("package") => "cpu",
        "core" => "cpu-cores",
        "uncore" => "cpu-uncore",
        "dram" => "memory",
        "psys" => "platform",
        _ => "sensor",
    }
}

impl ConsumptionSampler {
    pub fn sample(
        &mut self,
        root: &Root,
        secs: f64,
        chips: &[Chip],
        gpus: &[GpuSnapshot],
        batteries: &[BatterySnapshot],
    ) -> ConsumptionSnapshot {
        let mut sources = Vec::new();
        let mut denied = false;
        for zone in root.list(POWERCAP) {
            // intel-rapl:N (package) and intel-rapl:N:M (its parts); the
            // mmio interface repeats the same domains.
            if !zone.starts_with("intel-rapl:") {
                continue;
            }
            let dir = format!("{POWERCAP}/{zone}");
            let name = root.read_trimmed(format!("{dir}/name")).unwrap_or_default();
            let Some(cur) = root.read_u64(format!("{dir}/energy_uj")) else {
                denied = true;
                continue;
            };
            let range = root
                .read_u64(format!("{dir}/max_energy_range_uj"))
                .unwrap_or(u64::MAX);
            let watts = self
                .counters
                .insert(zone.clone(), cur)
                .filter(|_| secs > 0.0)
                .map(|prev| {
                    let delta = if cur >= prev {
                        cur - prev
                    } else {
                        range - prev + cur
                    };
                    delta as f64 / 1e6 / secs
                });
            let parent = zone.rsplit_once(':').map(|(p, _)| p.to_owned());
            sources.push(PowerSource {
                key: zone.clone(),
                kind: rapl_kind(&name),
                name,
                watts,
                part_of: parent.filter(|p| p.contains(':')),
                ..PowerSource::default()
            });
        }
        let package = sources
            .iter()
            .find(|s| s.kind == "cpu")
            .map(|s| s.key.clone());
        for g in gpus.iter().filter(|g| g.power_watts.is_some()) {
            sources.push(PowerSource {
                key: format!("gpu:{}", g.pci_slot),
                name: format!("GPU {} ({})", g.index, g.name),
                kind: "gpu",
                watts: g.power_watts,
                // An integrated GPU reports power within the CPU package.
                part_of: if g.dedicated { None } else { package.clone() },
                ..PowerSource::default()
            });
        }
        for b in batteries {
            if b.status == "Discharging" {
                sources.push(PowerSource {
                    key: format!("battery:{}", b.name),
                    name: b.name.clone(),
                    kind: "battery",
                    watts: b.watts,
                    ..PowerSource::default()
                });
            }
        }
        sources.extend(other_sensors(root, chips));

        for s in &mut sources {
            let h = self.history.entry(s.key.clone()).or_default();
            h.push(s.watts.unwrap_or(0.0));
            s.history = h.clone();
            let e = self.energy.entry(s.key.clone()).or_default();
            *e += s.watts.unwrap_or(0.0) * secs / 3600.0;
            s.energy_wh = *e;
        }
        let whole = |kind| sources.iter().find(|s| s.kind == kind && s.watts.is_some());
        let (total_kind, total_watts) = if let Some(p) = whole("platform") {
            ("platform", p.watts)
        } else if let Some(b) = whole("battery") {
            ("battery", b.watts)
        } else {
            let parts: Vec<f64> = sources
                .iter()
                .filter(|s| s.part_of.is_none() && s.kind != "battery")
                .filter_map(|s| s.watts)
                .collect();
            ("sum", (!parts.is_empty()).then(|| parts.iter().sum()))
        };
        if let Some(w) = total_watts {
            self.total_history.push(w);
            self.total_wh += w * secs / 3600.0;
        }
        ConsumptionSnapshot {
            sources,
            total_watts,
            total_kind,
            total_history: self.total_history.clone(),
            total_wh: self.total_wh,
            denied,
        }
    }
}

/// hwmon power sensors that are not a GPU (already listed): power supplies,
/// USB-C controllers, some motherboards.
fn other_sensors(root: &Root, chips: &[Chip]) -> Vec<PowerSource> {
    let gpu_chips = ["amdgpu", "nouveau", "i915", "xe", "radeon"];
    let mut out = Vec::new();
    for chip in chips
        .iter()
        .filter(|c| !gpu_chips.contains(&c.name.as_str()))
    {
        for n in 1..8 {
            let Some(uw) = chip
                .file(root, &format!("power{n}_average"))
                .or_else(|| chip.file(root, &format!("power{n}_input")))
            else {
                continue;
            };
            let label = chip
                .label(root, "power", n)
                .unwrap_or_else(|| format!("power{n}"));
            out.push(PowerSource {
                key: format!("hwmon:{}:{n}", chip.name),
                name: format!("{} ({label})", chip.name),
                kind: "sensor",
                watts: Some(uw as f64 / 1e6),
                ..PowerSource::default()
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(dir: &std::path::Path, zone: &str, name: &str, uj: u64) {
        let d = dir.join("sys/class/powercap").join(zone);
        std::fs::create_dir_all(&d).ok();
        std::fs::write(d.join("name"), format!("{name}\n")).ok();
        std::fs::write(d.join("energy_uj"), format!("{uj}\n")).ok();
    }

    #[test]
    fn package_and_parts_without_double_counting() {
        let dir = std::env::temp_dir().join(format!("taskmgr-power-{}", std::process::id()));
        let root = Root::new(&dir);
        zone(&dir, "intel-rapl:0", "package-0", 1_000_000);
        zone(&dir, "intel-rapl:0:0", "core", 500_000);
        let mut sampler = ConsumptionSampler::default();
        sampler.sample(&root, 1.0, &[], &[], &[]);
        zone(&dir, "intel-rapl:0", "package-0", 31_000_000);
        zone(&dir, "intel-rapl:0:0", "core", 20_500_000);
        let snap = sampler.sample(&root, 1.0, &[], &[], &[]);
        std::fs::remove_dir_all(&dir).ok();
        let watts: Vec<_> = snap
            .sources
            .iter()
            .map(|s| (s.kind, s.watts, s.part_of.as_deref()))
            .collect();
        assert_eq!(
            watts,
            [
                ("cpu", Some(30.0), None),
                ("cpu-cores", Some(20.0), Some("intel-rapl:0"))
            ]
        );
        assert_eq!((snap.total_kind, snap.total_watts), ("sum", Some(30.0)));
    }
}

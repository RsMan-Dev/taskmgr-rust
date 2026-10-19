//! Every temperature sensor separately, named after the device it measures.

use std::collections::HashMap;

use serde::Serialize;

use crate::disk::DiskSnapshot;
use crate::gpu::GpuSnapshot;
use crate::hwmon::Chip;
use crate::util::{History, Root};

#[derive(Clone, Debug, Serialize)]
pub struct Sensor {
    pub key: String,
    pub label: String,
    pub value: f64,
    pub high: Option<f64>,
    pub critical: Option<f64>,
    pub history: History,
}

#[derive(Clone, Debug, Serialize)]
pub struct SensorGroup {
    pub name: String,
    pub kind: &'static str,
    /// Disk name, GPU PCI slot or network interface the group belongs to.
    pub device: Option<String>,
    pub sensors: Vec<Sensor>,
}

const BOARD_CHIPS: [&str; 7] = [
    "gigabyte_wmi",
    "asus_wmi_sensors",
    "asus-ec-sensors",
    "dell_smm",
    "it87",
    "nct6775",
    "nct6683",
];

fn label(chip: &str, raw: Option<&str>, n: u32) -> String {
    let Some(raw) = raw else {
        return format!("Sensor {n}");
    };
    if let Some(ccd) = raw.strip_prefix("Tccd") {
        return format!("CCD {ccd}");
    }
    match (chip, raw) {
        (_, "Tctl") => "Tctl (control)".to_owned(),
        (_, "Tdie") => "Die".to_owned(),
        (_, "Package id 0") => "Package".to_owned(),
        ("amdgpu", "edge") => "Edge".to_owned(),
        ("amdgpu", "junction") => "Junction".to_owned(),
        ("amdgpu", "mem") => "Memory".to_owned(),
        (_, other) => {
            let other = other.strip_suffix(" Temperature").unwrap_or(other);
            match other.strip_prefix("Sensor ") {
                Some(i) => format!("Sensor {i}"),
                None => other.to_owned(),
            }
        }
    }
}

#[derive(Default)]
pub struct SensorSampler {
    history: HashMap<String, History>,
    /// Max and critical thresholds never change, and NVMe drives answer
    /// each read with a command: read once.
    limits: HashMap<String, (Option<f64>, Option<f64>)>,
}

impl SensorSampler {
    pub fn sample(
        &mut self,
        root: &Root,
        chips: &[Chip],
        disks: &[DiskSnapshot],
        gpus: &[GpuSnapshot],
    ) -> Vec<SensorGroup> {
        let mut groups: Vec<SensorGroup> = Vec::new();
        let mut seen = Vec::new();
        for chip in chips {
            let device_name = chip
                .device
                .as_ref()
                .and_then(|d| Some(d.file_name()?.to_string_lossy().into_owned()));
            let disk = disks.iter().find(|d| {
                root.canonical(format!("/sys/block/{}/device", d.name))
                    .is_some_and(|p| Some(&p) == chip.device.as_ref())
            });
            let gpu = gpus
                .iter()
                .find(|g| Some(&g.pci_slot) == device_name.as_ref());
            let (name, kind, device) = if let Some(d) = disk {
                (
                    format!("Disk {} ({})", d.index, d.model),
                    "disk",
                    Some(d.name.clone()),
                )
            } else if let Some(g) = gpu {
                (
                    format!("GPU {} ({})", g.index, g.name),
                    "gpu",
                    Some(g.pci_slot.clone()),
                )
            } else if matches!(chip.name.as_str(), "k10temp" | "zenpower" | "coretemp") {
                ("CPU".to_owned(), "cpu", None)
            } else if BOARD_CHIPS.iter().any(|b| chip.name.starts_with(b)) {
                ("Motherboard".to_owned(), "board", None)
            } else if chip.name == "acpitz" {
                ("ACPI (thermal zone)".to_owned(), "other", None)
            } else if root.exists(format!("/sys/class/net/{}", chip.name)) {
                (
                    format!("Network card {}", chip.name),
                    "network",
                    Some(chip.name.clone()),
                )
            } else {
                (chip.name.clone(), "other", None)
            };
            let base = format!("{}:{}", chip.name, device_name.unwrap_or_default());
            let sensors: Vec<Sensor> = chip
                .temps(root)
                .into_iter()
                .filter_map(|(n, raw, input)| {
                    let value = root.read_sensor(&input)? as f64 / 1000.0;
                    let key = format!("{base}:{n}");
                    let (max, crit) = *self.limits.entry(key.clone()).or_insert_with(|| {
                        (chip.limit(root, n, "max"), chip.limit(root, n, "crit"))
                    });
                    Some(self.sensor(
                        &mut seen,
                        key,
                        label(&chip.name, raw.as_deref(), n),
                        value,
                        max,
                        crit,
                    ))
                })
                .collect();
            if !sensors.is_empty() {
                groups.push(SensorGroup {
                    name,
                    kind,
                    device,
                    sensors,
                });
            }
        }
        for g in gpus.iter().filter(|g| g.driver == "nvidia") {
            if let Some(t) = g.temperature {
                let s = self.sensor(
                    &mut seen,
                    format!("nvml:{}", g.pci_slot),
                    "GPU".to_owned(),
                    t,
                    None,
                    None,
                );
                groups.push(SensorGroup {
                    name: format!("GPU {} ({})", g.index, g.name),
                    kind: "gpu",
                    device: Some(g.pci_slot.clone()),
                    sensors: vec![s],
                });
            }
        }
        self.history.retain(|k, _| seen.contains(k));
        let rank = |k: &str| {
            ["cpu", "gpu", "disk", "board", "network", "other"]
                .iter()
                .position(|x| *x == k)
                .unwrap_or(9)
        };
        groups.sort_by(|a, b| rank(a.kind).cmp(&rank(b.kind)).then(a.name.cmp(&b.name)));
        groups
    }

    fn sensor(
        &mut self,
        seen: &mut Vec<String>,
        key: String,
        label: String,
        value: f64,
        high: Option<f64>,
        critical: Option<f64>,
    ) -> Sensor {
        let h = self.history.entry(key.clone()).or_default();
        h.push(value);
        seen.push(key.clone());
        Sensor {
            history: h.clone(),
            key,
            label,
            value,
            high,
            critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_labels() {
        assert_eq!(label("k10temp", Some("Tccd2"), 4), "CCD 2");
        assert_eq!(label("nvme", Some("Sensor 8"), 9), "Sensor 8");
        assert_eq!(label("enp7s0", Some("PHY Temperature"), 1), "PHY");
        assert_eq!(label("gigabyte_wmi", None, 3), "Sensor 3");
        assert_eq!(label("amdgpu", Some("edge"), 1), "Edge");
    }
}

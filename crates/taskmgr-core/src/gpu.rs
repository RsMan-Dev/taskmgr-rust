//! The proprietary NVIDIA driver exposes nothing in sysfs: NVML is loaded at runtime.

use std::ffi::OsStr;

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use serde::Serialize;

use crate::hwmon::{self, Chip};
use crate::util::{History, Root};

#[derive(Clone, Debug, Default, Serialize)]
pub struct GpuSnapshot {
    pub index: usize,
    pub name: String,
    pub vendor: String,
    pub driver: String,
    pub driver_version: Option<String>,
    pub pci_slot: String,
    pub usage: Option<f64>,
    pub encoder: Option<f64>,
    pub decoder: Option<f64>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub shared_used: Option<u64>,
    pub shared_total: Option<u64>,
    pub temperature: Option<f64>,
    pub power_watts: Option<f64>,
    pub history: History,
    pub memory_history: History,
}

fn vendor_name(id: &str) -> &'static str {
    match id {
        "0x1002" => "AMD",
        "0x10de" => "NVIDIA",
        "0x8086" => "Intel",
        _ => "",
    }
}

fn marketing_name(vendor: &str, pci_model: &str) -> String {
    let inner = pci_model
        .rsplit_once('[')
        .and_then(|(_, r)| r.strip_suffix(']'))
        .unwrap_or(pci_model);
    if vendor.is_empty() || inner.starts_with(vendor) {
        inner.to_owned()
    } else {
        format!("{vendor} {inner}")
    }
}

fn load_nvml() -> Option<Nvml> {
    Nvml::init().ok().or_else(|| {
        // NixOS: the driver lives outside the default library search path.
        Nvml::builder()
            .lib_path(OsStr::new("/run/opengl-driver/lib/libnvidia-ml.so.1"))
            .init()
            .ok()
    })
}

#[derive(Default)]
struct GpuState {
    usage: History,
    memory: History,
}

pub struct GpuSampler {
    nvml: Option<Nvml>,
    state: Vec<(String, GpuState)>,
}

impl GpuSampler {
    pub fn new() -> Self {
        Self {
            nvml: load_nvml(),
            state: Vec::new(),
        }
    }

    fn cards(root: &Root) -> Vec<String> {
        root.list("/sys/class/drm")
            .into_iter()
            .filter(|c| {
                c.strip_prefix("card")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
            .collect()
    }

    pub fn sample(&mut self, root: &Root, chips: &[Chip]) -> Vec<GpuSnapshot> {
        let mut cards = Self::cards(root);
        cards.sort_by_key(|c| c[4..].parse::<u32>().unwrap_or(0));
        cards
            .into_iter()
            .enumerate()
            .map(|(index, card)| {
                let dev = format!("/sys/class/drm/{card}/device");
                let pci_slot = root
                    .canonical(&dev)
                    .and_then(|p| Some(p.file_name()?.to_string_lossy().into_owned()))
                    .unwrap_or_default();
                let vendor = vendor_name(
                    &root
                        .read_trimmed(format!("{dev}/vendor"))
                        .unwrap_or_default(),
                )
                .to_owned();
                let driver = root.link_name(format!("{dev}/driver")).unwrap_or_default();
                let pci_model =
                    root.udev_property(&format!("+pci:{pci_slot}"), "ID_MODEL_FROM_DATABASE");
                let mut g = GpuSnapshot {
                    index,
                    name: pci_model
                        .map(|m| marketing_name(&vendor, &m))
                        .unwrap_or_else(|| format!("{vendor} GPU")),
                    vendor,
                    driver,
                    pci_slot,
                    ..Default::default()
                };
                if g.driver == "nvidia" {
                    self.fill_nvml(&mut g);
                } else {
                    fill_sysfs(root, &dev, &mut g);
                    g.temperature = root
                        .canonical(&dev)
                        .and_then(|d| hwmon::device_temperature(root, chips, &d, &["edge"]));
                }
                let st = match self.state.iter().position(|(k, _)| *k == g.pci_slot) {
                    Some(i) => &mut self.state[i].1,
                    None => {
                        self.state.push((g.pci_slot.clone(), GpuState::default()));
                        &mut self.state.last_mut().unwrap().1
                    }
                };
                st.usage.push(g.usage.unwrap_or(0.0));
                if let (Some(u), Some(t)) = (g.memory_used, g.memory_total) {
                    st.memory.push(u as f64 / t.max(1) as f64 * 100.0);
                }
                g.history = st.usage.clone();
                g.memory_history = st.memory.clone();
                g
            })
            .collect()
    }

    fn fill_nvml(&self, g: &mut GpuSnapshot) {
        let Some(nvml) = &self.nvml else { return };
        let Ok(d) = nvml.device_by_pci_bus_id(g.pci_slot.as_str()) else {
            return;
        };
        if let Ok(name) = d.name() {
            g.name = name;
        }
        g.driver_version = nvml.sys_driver_version().ok();
        g.usage = d.utilization_rates().ok().map(|u| u.gpu as f64);
        g.encoder = d.encoder_utilization().ok().map(|u| u.utilization as f64);
        g.decoder = d.decoder_utilization().ok().map(|u| u.utilization as f64);
        if let Ok(m) = d.memory_info() {
            g.memory_used = Some(m.used);
            g.memory_total = Some(m.total);
        }
        g.temperature = d.temperature(TemperatureSensor::Gpu).ok().map(|t| t as f64);
        g.power_watts = d.power_usage().ok().map(|mw| mw as f64 / 1000.0);
    }
}

impl Default for GpuSampler {
    fn default() -> Self {
        Self::new()
    }
}

fn fill_sysfs(root: &Root, dev: &str, g: &mut GpuSnapshot) {
    g.usage = root
        .read_u64(format!("{dev}/gpu_busy_percent"))
        .map(|v| v as f64);
    g.memory_used = root.read_u64(format!("{dev}/mem_info_vram_used"));
    g.memory_total = root.read_u64(format!("{dev}/mem_info_vram_total"));
    g.shared_used = root.read_u64(format!("{dev}/mem_info_gtt_used"));
    g.shared_total = root.read_u64(format!("{dev}/mem_info_gtt_total"));
    g.driver_version = root.read_trimmed(format!("/sys/module/{}/version", g.driver));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marketing_names() {
        assert_eq!(
            marketing_name("NVIDIA", "AD103 [GeForce RTX 4080]"),
            "NVIDIA GeForce RTX 4080"
        );
        assert_eq!(
            marketing_name("AMD", "Granite Ridge [Radeon Graphics]"),
            "AMD Radeon Graphics"
        );
        assert_eq!(marketing_name("", "Mystery"), "Mystery");
    }
}

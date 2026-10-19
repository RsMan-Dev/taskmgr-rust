//! The proprietary NVIDIA driver exposes nothing in sysfs: NVML is loaded at runtime.

use std::ffi::OsStr;

use std::path::PathBuf;

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::enums::device::UsedGpuMemory;
use serde::Serialize;

use crate::hwmon::Chip;
use crate::util::{Charts, History, Root};

#[derive(Clone, Debug, Default, Serialize)]
pub struct GpuSnapshot {
    pub index: usize,
    pub name: String,
    pub vendor: String,
    pub driver: String,
    pub driver_version: Option<String>,
    pub pci_slot: String,
    /// Integrated GPUs share memory with the CPU and are left out of the
    /// per-process columns.
    pub dedicated: bool,
    pub usage: Option<f64>,
    pub encoder: Option<f64>,
    pub decoder: Option<f64>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub shared_used: Option<u64>,
    pub shared_total: Option<u64>,
    pub temperature: Option<f64>,
    pub power_watts: Option<f64>,
    pub power_limit_watts: Option<f64>,
    pub core_mhz: Option<f64>,
    pub core_max_mhz: Option<f64>,
    pub memory_mhz: Option<f64>,
    pub memory_max_mhz: Option<f64>,
    pub voltage_mv: Option<f64>,
    pub history: History,
    pub memory_history: History,
    pub charts: Charts,
    #[serde(skip)]
    pub processes: Vec<GpuProcess>,
}

#[derive(Clone, Debug, Default)]
pub struct GpuProcess {
    pub pid: u32,
    pub usage: f64,
    pub memory: Option<u64>,
}

/// AMD APU graphics sit behind the 00:08.1 bridge, Intel's at 00:02.0.
/// Server management chips (ASPEED, Matrox G200) only drive a console.
fn is_integrated(device_path: &str, pci_slot: &str, vendor_id: &str) -> bool {
    device_path.contains("/0000:00:08.1/")
        || pci_slot == "0000:00:02.0"
        || matches!(vendor_id, "0x1a03" | "0x102b")
}

/// Highest level of `pp_dpm_sclk` style tables (`2: 2200Mhz *`).
fn dpm_max_mhz(table: &str) -> Option<f64> {
    table
        .lines()
        .filter_map(|l| {
            l.split_whitespace()
                .nth(1)?
                .to_lowercase()
                .strip_suffix("mhz")?
                .parse::<f64>()
                .ok()
        })
        .reduce(f64::max)
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
    charts: Charts,
    nvml_seen: u64,
    /// What does not change while the card is present, read once.
    fixed: Option<GpuSnapshot>,
    device_path: Option<PathBuf>,
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

    /// Identity, limits and driver of a card: read on first sight only.
    fn identify(&self, root: &Root, index: usize, dev: &str) -> (GpuSnapshot, Option<PathBuf>) {
        let device_path = root.canonical(dev);
        let pci_slot = device_path
            .as_ref()
            .and_then(|p| Some(p.file_name()?.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let vendor_id = root
            .read_trimmed(format!("{dev}/vendor"))
            .unwrap_or_default();
        let vendor = vendor_name(&vendor_id).to_owned();
        let driver = root.link_name(format!("{dev}/driver")).unwrap_or_default();
        let pci_model = root.udev_property(&format!("+pci:{pci_slot}"), "ID_MODEL_FROM_DATABASE");
        let path_text = device_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut g = GpuSnapshot {
            index,
            dedicated: !is_integrated(&path_text, &pci_slot, &vendor_id),
            name: pci_model
                .map(|m| marketing_name(&vendor, &m))
                .unwrap_or_else(|| format!("{vendor} GPU")),
            vendor,
            driver,
            pci_slot,
            ..Default::default()
        };
        if g.driver == "nvidia" {
            if let Some(d) = self
                .nvml
                .as_ref()
                .and_then(|n| n.device_by_pci_bus_id(g.pci_slot.as_str()).ok())
            {
                if let Ok(name) = d.name() {
                    g.name = name;
                }
                g.core_max_mhz = d.max_clock_info(Clock::Graphics).ok().map(f64::from);
                g.memory_max_mhz = d.max_clock_info(Clock::Memory).ok().map(f64::from);
            }
            g.driver_version = self.nvml.as_ref().and_then(|n| n.sys_driver_version().ok());
        } else {
            g.driver_version = root.read_trimmed(format!("/sys/module/{}/version", g.driver));
            g.memory_total = root.read_u64(format!("{dev}/mem_info_vram_total"));
            g.shared_total = root.read_u64(format!("{dev}/mem_info_gtt_total"));
            g.core_max_mhz = root
                .read(format!("{dev}/pp_dpm_sclk"))
                .and_then(|t| dpm_max_mhz(&t));
            g.memory_max_mhz = root
                .read(format!("{dev}/pp_dpm_mclk"))
                .and_then(|t| dpm_max_mhz(&t))
                .filter(|_| g.dedicated);
        }
        (g, device_path)
    }

    /// `per_process`: also read which processes use each GPU (costly with
    /// NVML), only needed while a process list is shown.
    pub fn sample(&mut self, root: &Root, chips: &[Chip], per_process: bool) -> Vec<GpuSnapshot> {
        let mut cards = Self::cards(root);
        cards.sort_by_key(|c| c[4..].parse::<u32>().unwrap_or(0));
        let mut out = Vec::new();
        for (index, card) in cards.into_iter().enumerate() {
            let dev = format!("/sys/class/drm/{card}/device");
            let i = match self.state.iter().position(|(k, _)| *k == card) {
                Some(i) => i,
                None => {
                    self.state.push((card.clone(), GpuState::default()));
                    self.state.len() - 1
                }
            };
            if self.state[i].1.fixed.is_none() {
                let (fixed, path) = self.identify(root, index, &dev);
                self.state[i].1.fixed = Some(fixed);
                self.state[i].1.device_path = path;
            }
            let Some(mut g) = self.state[i].1.fixed.clone() else {
                continue;
            };
            g.index = index;
            if g.driver == "nvidia" {
                let seen = self.state[i].1.nvml_seen;
                self.state[i].1.nvml_seen = self.fill_nvml(&mut g, seen, per_process);
            } else {
                fill_sysfs(root, &dev, &mut g);
                let path = self.state[i].1.device_path.as_ref();
                if let Some(c) = chips.iter().find(|c| c.device.as_ref() == path) {
                    g.temperature = c.temperature(root, &["edge"]);
                    g.core_mhz = c.input(root, "freq", "sclk").map(|hz| hz as f64 / 1e6);
                    g.memory_mhz = c
                        .input(root, "freq", "mclk")
                        .filter(|_| g.dedicated)
                        .map(|hz| hz as f64 / 1e6);
                    g.voltage_mv = c.input(root, "in", "vddgfx").map(|mv| mv as f64);
                    g.power_watts = c
                        .file(root, "power1_average")
                        .or_else(|| c.file(root, "power1_input"))
                        .map(|uw| uw as f64 / 1e6);
                    g.power_limit_watts = c.file(root, "power1_cap").map(|uw| uw as f64 / 1e6);
                }
            }
            let st = &mut self.state[i].1;
            st.charts.push(
                "memory",
                g.memory_used
                    .zip(g.memory_total)
                    .map(|(u, t)| u as f64 / t.max(1) as f64 * 100.0),
            );
            st.charts.push("core_clock", g.core_mhz);
            st.charts.push("memory_clock", g.memory_mhz);
            st.charts.push("voltage", g.voltage_mv);
            st.charts.push("power", g.power_watts);
            st.charts.push("encoder", g.encoder);
            st.charts.push("decoder", g.decoder);
            g.charts = st.charts.clone();
            st.usage.push(g.usage.unwrap_or(0.0));
            if let (Some(u), Some(t)) = (g.memory_used, g.memory_total) {
                st.memory.push(u as f64 / t.max(1) as f64 * 100.0);
            }
            g.history = st.usage.clone();
            g.memory_history = st.memory.clone();
            out.push(g);
        }
        out
    }

    /// Returns the timestamp of the newest per-process sample read.
    fn fill_nvml(&self, g: &mut GpuSnapshot, seen: u64, per_process: bool) -> u64 {
        let Some(nvml) = &self.nvml else { return seen };
        let Ok(d) = nvml.device_by_pci_bus_id(g.pci_slot.as_str()) else {
            return seen;
        };
        g.usage = d.utilization_rates().ok().map(|u| u.gpu as f64);
        g.encoder = d.encoder_utilization().ok().map(|u| u.utilization as f64);
        g.decoder = d.decoder_utilization().ok().map(|u| u.utilization as f64);
        if let Ok(m) = d.memory_info() {
            g.memory_used = Some(m.used);
            g.memory_total = Some(m.total);
        }
        g.temperature = d.temperature(TemperatureSensor::Gpu).ok().map(|t| t as f64);
        g.power_watts = d.power_usage().ok().map(|mw| mw as f64 / 1000.0);
        g.power_limit_watts = d.enforced_power_limit().ok().map(|mw| mw as f64 / 1000.0);
        g.core_mhz = d.clock_info(Clock::Graphics).ok().map(f64::from);
        g.memory_mhz = d.clock_info(Clock::Memory).ok().map(f64::from);
        if !per_process {
            return seen;
        }

        let mut newest = seen;
        let mut usage: std::collections::HashMap<u32, f64> = std::collections::HashMap::new();
        for s in d.process_utilization_stats(seen).unwrap_or_default() {
            newest = newest.max(s.timestamp);
            let u = usage.entry(s.pid).or_default();
            *u = u.max(s.sm_util as f64);
        }
        let mut memory: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
        for p in d
            .running_graphics_processes()
            .unwrap_or_default()
            .into_iter()
            .chain(d.running_compute_processes().unwrap_or_default())
        {
            // A process using both graphics and compute is listed twice.
            if let UsedGpuMemory::Used(b) = p.used_gpu_memory {
                let m = memory.entry(p.pid).or_default();
                *m = (*m).max(b);
            }
        }
        let pids: std::collections::BTreeSet<u32> =
            usage.keys().chain(memory.keys()).copied().collect();
        g.processes = pids
            .into_iter()
            .map(|pid| GpuProcess {
                pid,
                usage: usage.get(&pid).copied().unwrap_or(0.0),
                memory: memory.get(&pid).copied(),
            })
            .collect();
        newest
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
    g.shared_used = root.read_u64(format!("{dev}/mem_info_gtt_used"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpm_tables_and_integration() {
        assert_eq!(
            dpm_max_mhz("0: 600Mhz\n1: 1100Mhz *\n2: 2200Mhz\n"),
            Some(2200.0)
        );
        let amd_apu = "/sys/devices/pci0000:00/0000:00:08.1/0000:76:00.0";
        assert!(is_integrated(amd_apu, "0000:76:00.0", "0x1002"));
        let nvidia = "/sys/devices/pci0000:00/0000:00:01.1/0000:01:00.0";
        assert!(!is_integrated(nvidia, "0000:01:00.0", "0x10de"));
        let aspeed = "/sys/devices/pci0000:00/0000:00:1c.0/0000:06:00.0";
        assert!(is_integrated(aspeed, "0000:06:00.0", "0x1a03"));
    }

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

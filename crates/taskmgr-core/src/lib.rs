//! System sampling for taskmgr: CPU, memory, disks, network and GPUs, read
//! from `/proc` and `/sys` without root.

pub mod autostart;
pub mod catalog;
pub mod cpu;
pub mod disk;
pub mod energy;
pub mod gpu;
pub mod hwmon;
pub mod identity;
pub mod memory;
pub mod msr;
pub mod net;
pub mod netflow;
pub mod process;
pub mod sensors;
pub mod services;
pub mod sysinfo;
pub mod util;
pub mod windows;

use std::time::Instant;

use serde::Serialize;

pub use process::{ProcessGroup, ProcessSampler};
pub use sysinfo::{SystemInfo, system_info};
pub use util::Root;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub cpu: cpu::CpuSnapshot,
    pub memory: memory::MemorySnapshot,
    pub disks: Vec<disk::DiskSnapshot>,
    pub networks: Vec<net::NetSnapshot>,
    pub gpus: Vec<gpu::GpuSnapshot>,
    pub sensors: Vec<sensors::SensorGroup>,
    pub energy: energy::EnergySnapshot,
}

pub struct Sampler {
    root: Root,
    last: Option<Instant>,
    cpu: cpu::CpuSampler,
    memory: memory::MemorySampler,
    disks: disk::DiskSampler,
    networks: net::NetSampler,
    gpus: gpu::GpuSampler,
    sensors: sensors::SensorSampler,
    energy: energy::EnergySampler,
    /// Core voltage from the root helper, used when no driver exposes one.
    pub fallback_voltage: Option<f64>,
}

impl Sampler {
    pub fn new(root: Root) -> Self {
        Self {
            cpu: cpu::CpuSampler::new(&root),
            memory: Default::default(),
            disks: Default::default(),
            networks: Default::default(),
            gpus: gpu::GpuSampler::new(),
            sensors: Default::default(),
            energy: Default::default(),
            fallback_voltage: None,
            last: None,
            root,
        }
    }

    pub fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let secs = self
            .last
            .map_or(0.0, |l| now.duration_since(l).as_secs_f64());
        self.last = Some(now);
        let root = &self.root;
        let chips = hwmon::chips(root);
        let disks = self.disks.sample(root, &chips, secs);
        let gpus = self.gpus.sample(root, &chips);
        Snapshot {
            cpu: self.cpu.sample(root, &chips, self.fallback_voltage),
            memory: self.memory.sample(root),
            networks: self.networks.sample(root, secs),
            sensors: self.sensors.sample(root, &chips, &disks, &gpus),
            energy: self.energy.sample(root, secs),
            disks,
            gpus,
        }
    }
}

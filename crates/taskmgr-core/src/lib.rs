//! System sampling for taskmgr: CPU, memory, disks, network and GPUs, read
//! from `/proc` and `/sys` without root.

pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod hwmon;
pub mod memory;
pub mod net;
pub mod sysinfo;
pub mod util;

use std::time::Instant;

use serde::Serialize;

pub use sysinfo::{SystemInfo, system_info};
pub use util::Root;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub cpu: cpu::CpuSnapshot,
    pub memory: memory::MemorySnapshot,
    pub disks: Vec<disk::DiskSnapshot>,
    pub networks: Vec<net::NetSnapshot>,
    pub gpus: Vec<gpu::GpuSnapshot>,
}

pub struct Sampler {
    root: Root,
    last: Option<Instant>,
    cpu: cpu::CpuSampler,
    memory: memory::MemorySampler,
    disks: disk::DiskSampler,
    networks: net::NetSampler,
    gpus: gpu::GpuSampler,
}

impl Sampler {
    pub fn new(root: Root) -> Self {
        Self {
            cpu: cpu::CpuSampler::new(&root),
            memory: Default::default(),
            disks: Default::default(),
            networks: Default::default(),
            gpus: gpu::GpuSampler::new(),
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
        Snapshot {
            cpu: self.cpu.sample(root, &chips),
            memory: self.memory.sample(root),
            disks: self.disks.sample(root, &chips, secs),
            networks: self.networks.sample(root, secs),
            gpus: self.gpus.sample(root, &chips),
        }
    }
}

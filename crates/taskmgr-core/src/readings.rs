//! Every live reading of the machine as a tree (component, then reading),
//! with the lowest and highest value seen since the window opened: the
//! HWMonitor view of System > Details.

use std::collections::HashMap;

use serde::Serialize;

use crate::Snapshot;

#[derive(Clone, Debug, Serialize)]
pub struct Reading {
    pub key: String,
    /// English, translated by the interface (`Core 3`, `Temperature`...).
    pub label: String,
    /// percent, mhz, celsius, watts, volts, rpm, bytes, bytes/s.
    pub unit: &'static str,
    pub value: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReadingGroup {
    pub key: String,
    pub name: String,
    /// cpu, memory, gpu, disk, network, board, battery, fans, other.
    pub kind: &'static str,
    pub readings: Vec<Reading>,
}

#[derive(Default)]
pub struct ReadingTracker {
    extremes: HashMap<String, (f64, f64)>,
}

struct Builder<'a> {
    tracker: &'a mut ReadingTracker,
    group: ReadingGroup,
}

impl Builder<'_> {
    fn add(&mut self, label: impl Into<String>, unit: &'static str, value: Option<f64>) {
        let Some(value) = value.filter(|v| v.is_finite()) else {
            return;
        };
        let label = label.into();
        let key = format!("{}/{label}", self.group.key);
        let (min, max) = self
            .tracker
            .extremes
            .entry(key.clone())
            .and_modify(|(lo, hi)| {
                *lo = lo.min(value);
                *hi = hi.max(value);
            })
            .or_insert((value, value));
        let (min, max) = (*min, *max);
        self.group.readings.push(Reading {
            key,
            label,
            unit,
            value,
            min,
            max,
        });
    }
}

impl ReadingTracker {
    fn group<'a>(&'a mut self, key: String, name: String, kind: &'static str) -> Builder<'a> {
        Builder {
            tracker: self,
            group: ReadingGroup {
                key,
                name,
                kind,
                readings: Vec::new(),
            },
        }
    }

    pub fn update(&mut self, s: &Snapshot) -> Vec<ReadingGroup> {
        let mut out = Vec::new();
        let sensors_of = |kind: &str, device: Option<&str>| {
            s.sensors
                .iter()
                .filter(|g| g.kind == kind && g.device.as_deref() == device)
                .flat_map(|g| g.sensors.iter())
                .map(|t| (t.label.clone(), t.value))
                .collect::<Vec<_>>()
        };

        let c = &s.cpu;
        let mut g = self.group("cpu".into(), c.info.model.clone(), "cpu");
        g.add("Utilization", "percent", Some(c.usage));
        g.add("Average clock", "mhz", c.mhz);
        for (i, h) in c.core_mhz_history.iter().enumerate() {
            g.add(format!("Processor {i}"), "mhz", Some(h.last()));
        }
        for (label, v) in sensors_of("cpu", None) {
            g.add(label, "celsius", Some(v));
        }
        for src in &s.consumption.sources {
            match src.kind {
                "cpu" => g.add("Package power", "watts", src.watts),
                "cpu-cores" => g.add("Cores power", "watts", src.watts),
                _ => {}
            }
        }
        g.add("Voltage", "volts", c.voltage);
        out.push(g.group);

        let m = &s.memory;
        let mut g = self.group("memory".into(), "Memory".into(), "memory");
        g.add("Utilization", "percent", Some(m.usage));
        g.add("In use", "bytes", Some(m.used as f64));
        g.add("Swap used", "bytes", Some(m.swap_used as f64));
        out.push(g.group);

        for gpu in &s.gpus {
            let mut g = self.group(format!("gpu:{}", gpu.pci_slot), gpu.name.clone(), "gpu");
            g.add("Utilization", "percent", gpu.usage);
            g.add("Clock", "mhz", gpu.core_mhz);
            g.add("Memory clock", "mhz", gpu.memory_mhz);
            for (label, v) in sensors_of("gpu", Some(&gpu.pci_slot)) {
                g.add(label, "celsius", Some(v));
            }
            g.add("Power", "watts", gpu.power_watts);
            g.add("Voltage", "volts", gpu.voltage_mv.map(|mv| mv / 1000.0));
            g.add("Fan", "percent", gpu.fan_percent);
            g.add("Memory used", "bytes", gpu.memory_used.map(|b| b as f64));
            out.push(g.group);
        }

        for d in &s.disks {
            let mut g = self.group(format!("disk:{}", d.name), d.model.clone(), "disk");
            for (label, v) in sensors_of("disk", Some(&d.name)) {
                g.add(label, "celsius", Some(v));
            }
            g.add("Active time", "percent", Some(d.active));
            g.add("Read", "bytes/s", Some(d.read_bps));
            g.add("Write", "bytes/s", Some(d.write_bps));
            out.push(g.group);
        }

        for n in &s.networks {
            let mut g = self.group(format!("net:{}", n.name), n.adapter.clone(), "network");
            for (label, v) in sensors_of("network", Some(&n.name)) {
                g.add(label, "celsius", Some(v));
            }
            g.add("Receive", "bytes/s", Some(n.rx_bps));
            g.add("Send", "bytes/s", Some(n.tx_bps));
            out.push(g.group);
        }

        for b in &s.batteries {
            let mut g = self.group(format!("battery:{}", b.name), b.model.clone(), "battery");
            g.add("Charge", "percent", b.percent);
            g.add("Power", "watts", b.watts);
            out.push(g.group);
        }

        let mut sources: Vec<&str> = Vec::new();
        for f in &s.fans {
            if !sources.contains(&f.source.as_str()) {
                sources.push(&f.source);
            }
        }
        for src in sources {
            let mut g = self.group(format!("fans:{src}"), format!("{src} fans"), "fans");
            for f in s.fans.iter().filter(|f| f.source == src) {
                g.add(f.name.clone(), "rpm", f.rpm);
                g.add(format!("{} duty", f.name), "percent", f.percent);
            }
            out.push(g.group);
        }

        // Board and other chips: temperatures only reach the tree through
        // their sensor group.
        for sg in s
            .sensors
            .iter()
            .filter(|g| matches!(g.kind, "board" | "other"))
        {
            let mut g = self.group(format!("sensors:{}", sg.name), sg.name.clone(), sg.kind);
            for t in &sg.sensors {
                g.add(t.label.clone(), "celsius", Some(t.value));
            }
            out.push(g.group);
        }
        out.retain(|g| !g.readings.is_empty());
        out
    }
}

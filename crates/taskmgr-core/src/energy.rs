//! CPU package power from RAPL. The counter is root-only by default since
//! CVE-2020-8694 (PLATYPUS); docs/energy.md explains how to open it.

use serde::Serialize;

use crate::util::{History, Root};

#[derive(Clone, Debug, Default, Serialize)]
pub struct EnergySnapshot {
    /// "ok", "denied" (counter not readable) or "absent".
    pub status: &'static str,
    pub package_watts: Option<f64>,
    pub history: History,
}

const DOMAIN: &str = "/sys/class/powercap/intel-rapl:0";

#[derive(Default)]
pub struct EnergySampler {
    prev: Option<u64>,
    history: History,
}

impl EnergySampler {
    pub fn sample(&mut self, root: &Root, secs: f64) -> EnergySnapshot {
        if !root.exists(DOMAIN) {
            return EnergySnapshot {
                status: "absent",
                ..Default::default()
            };
        }
        let Some(cur) = root.read_u64(format!("{DOMAIN}/energy_uj")) else {
            return EnergySnapshot {
                status: "denied",
                ..Default::default()
            };
        };
        let range = root
            .read_u64(format!("{DOMAIN}/max_energy_range_uj"))
            .unwrap_or(u64::MAX);
        let watts = self.prev.filter(|_| secs > 0.0).map(|p| {
            let delta = if cur >= p { cur - p } else { range - p + cur };
            delta as f64 / 1e6 / secs
        });
        self.prev = Some(cur);
        if let Some(w) = watts {
            self.history.push(w);
        }
        EnergySnapshot {
            status: "ok",
            package_watts: watts,
            history: self.history.clone(),
        }
    }
}

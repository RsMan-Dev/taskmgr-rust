//! Laptop batteries, from /sys/class/power_supply (peripheral batteries,
//! scope "Device", are left out).

use std::collections::HashMap;

use serde::Serialize;

use crate::Root;
use crate::util::History;

#[derive(Clone, Debug, Default, Serialize)]
pub struct BatterySnapshot {
    pub name: String,
    pub model: String,
    pub technology: Option<String>,
    /// Charging, Discharging, Full, Not charging, Unknown.
    pub status: String,
    pub on_ac: bool,
    pub percent: Option<f64>,
    /// Positive both ways; the status tells the direction.
    pub watts: Option<f64>,
    pub energy_wh: Option<f64>,
    pub full_wh: Option<f64>,
    pub design_wh: Option<f64>,
    /// Until empty when discharging, until full when charging.
    pub seconds_left: Option<u64>,
    pub cycles: Option<u64>,
    pub percent_history: History,
    pub watts_history: History,
}

#[derive(Default)]
pub struct BatterySampler {
    history: HashMap<String, (History, History)>,
}

/// Dev aid: TASKMGR_POWER_SUPPLY points at a copy of the sysfs directory.
fn base() -> String {
    std::env::var("TASKMGR_POWER_SUPPLY").unwrap_or_else(|_| "/sys/class/power_supply".to_owned())
}

impl BatterySampler {
    pub fn sample(&mut self, root: &Root) -> Vec<BatterySnapshot> {
        let base = base();
        let read = |name: &str, f: &str| root.read_trimmed(format!("{base}/{name}/{f}"));
        let num = |name: &str, f: &str| read(name, f).and_then(|v| v.parse::<f64>().ok());
        let mut names = root.list(&base);
        names.sort();
        let on_ac = names.iter().any(|n| {
            read(n, "type").as_deref() == Some("Mains") && read(n, "online").as_deref() == Some("1")
        });
        let mut out = Vec::new();
        for name in names {
            if read(&name, "type").as_deref() != Some("Battery")
                || read(&name, "scope").as_deref() == Some("Device")
                || read(&name, "present").as_deref() == Some("0")
            {
                continue;
            }
            // Energy in uWh, or charge in uAh times the design voltage.
            let volts = num(&name, "voltage_min_design")
                .or_else(|| num(&name, "voltage_now"))
                .map(|uv| uv / 1e6);
            let wh = |energy: &str, charge: &str| {
                num(&name, energy)
                    .map(|uwh| uwh / 1e6)
                    .or_else(|| Some(num(&name, charge)? / 1e6 * volts?))
            };
            let energy_wh = wh("energy_now", "charge_now");
            let full_wh = wh("energy_full", "charge_full");
            let design_wh = wh("energy_full_design", "charge_full_design");
            let watts = num(&name, "power_now")
                .map(|uw| uw / 1e6)
                .or_else(|| {
                    Some(num(&name, "current_now")? / 1e6 * num(&name, "voltage_now")? / 1e6)
                })
                .map(f64::abs);
            let percent = num(&name, "capacity").or_else(|| Some(energy_wh? / full_wh? * 100.0));
            let status = read(&name, "status").unwrap_or_else(|| "Unknown".to_owned());
            let seconds_left = match (status.as_str(), watts.filter(|w| *w > 0.1)) {
                ("Discharging", Some(w)) => energy_wh.map(|e| e / w * 3600.0),
                ("Charging", Some(w)) => full_wh
                    .zip(energy_wh)
                    .map(|(f, e)| (f - e).max(0.0) / w * 3600.0),
                _ => None,
            }
            .map(|s| s as u64);
            let (ph, wh_hist) = self.history.entry(name.clone()).or_default();
            ph.push(percent.unwrap_or(0.0));
            wh_hist.push(watts.unwrap_or(0.0));
            let model = [read(&name, "manufacturer"), read(&name, "model_name")]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            out.push(BatterySnapshot {
                model,
                technology: read(&name, "technology").filter(|t| t != "Unknown"),
                status,
                on_ac,
                percent,
                watts,
                energy_wh,
                full_wh,
                design_wh,
                seconds_left,
                cycles: read(&name, "cycle_count")
                    .and_then(|c| c.parse().ok())
                    .filter(|c| *c > 0),
                percent_history: ph.clone(),
                watts_history: wh_hist.clone(),
                name,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &std::path::Path, name: &str, files: &[(&str, &str)]) {
        let d = dir.join("sys/class/power_supply").join(name);
        std::fs::create_dir_all(&d).unwrap();
        for (f, v) in files {
            std::fs::write(d.join(f), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn reads_charge_based_battery() {
        let dir = std::env::temp_dir().join(format!("taskmgr-bat-{}", std::process::id()));
        write(&dir, "AC", &[("type", "Mains"), ("online", "0")]);
        write(
            &dir,
            "hidpp_battery_0",
            &[("type", "Battery"), ("scope", "Device"), ("capacity", "40")],
        );
        write(
            &dir,
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "50"),
                ("voltage_min_design", "11000000"),
                ("voltage_now", "11000000"),
                ("charge_now", "2000000"),
                ("charge_full", "4000000"),
                ("charge_full_design", "5000000"),
                ("current_now", "1000000"),
                ("manufacturer", "ACME"),
                ("model_name", "B42"),
            ],
        );
        let list = BatterySampler::default().sample(&Root::new(&dir));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(list.len(), 1);
        let b = &list[0];
        assert_eq!(
            (b.name.as_str(), b.model.as_str(), b.on_ac),
            ("BAT0", "ACME B42", false)
        );
        assert_eq!(
            (b.energy_wh, b.full_wh, b.design_wh),
            (Some(22.0), Some(44.0), Some(55.0))
        );
        assert_eq!(b.watts, Some(11.0));
        assert_eq!(b.seconds_left, Some(7200));
    }
}

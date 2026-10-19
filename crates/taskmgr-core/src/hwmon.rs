//! hwmonN numbers change between boots: chips are matched by name and device.

use std::path::PathBuf;

use crate::util::Root;

#[derive(Clone, Debug)]
pub struct Chip {
    dir: String,
    pub name: String,
    pub device: Option<PathBuf>,
}

impl Chip {
    fn temps(&self, root: &Root) -> Vec<(Option<String>, String)> {
        let mut inputs: Vec<(u32, String)> = root
            .list(&self.dir)
            .into_iter()
            .filter_map(|f| {
                let n = f
                    .strip_prefix("temp")?
                    .strip_suffix("_input")?
                    .parse()
                    .ok()?;
                Some((n, f))
            })
            .collect();
        inputs.sort();
        inputs
            .into_iter()
            .map(|(n, f)| {
                let label = root.read_trimmed(format!("{}/temp{n}_label", self.dir));
                (label, format!("{}/{f}", self.dir))
            })
            .collect()
    }

    pub fn temperature(&self, root: &Root, preferred: &[&str]) -> Option<f64> {
        let temps = self.temps(root);
        let pick = preferred
            .iter()
            .find_map(|p| temps.iter().find(|(l, _)| l.as_deref() == Some(*p)))
            .or(temps.first())?;
        root.read_i64(&pick.1).map(|m| m as f64 / 1000.0)
    }
}

pub fn chips(root: &Root) -> Vec<Chip> {
    root.list("/sys/class/hwmon")
        .into_iter()
        .filter_map(|h| {
            let dir = format!("/sys/class/hwmon/{h}");
            let name = root.read_trimmed(format!("{dir}/name"))?;
            let device = root.canonical(format!("{dir}/device"));
            Some(Chip { dir, name, device })
        })
        .collect()
}

pub fn cpu_temperature(root: &Root, chips: &[Chip]) -> Option<f64> {
    chips
        .iter()
        .find(|c| matches!(c.name.as_str(), "k10temp" | "zenpower" | "coretemp"))?
        .temperature(root, &["Tctl", "Tdie", "Package id 0"])
}

pub fn device_temperature(
    root: &Root,
    chips: &[Chip],
    device: &PathBuf,
    preferred: &[&str],
) -> Option<f64> {
    chips
        .iter()
        .find(|c| c.device.as_ref() == Some(device))?
        .temperature(root, preferred)
}

//! Fans the hwmon drivers expose: speed, and the duty cycle their
//! controller asks for when it is readable.

use serde::Serialize;

use crate::hwmon::Chip;
use crate::util::{History, Root};

#[derive(Clone, Debug, Default, Serialize)]
pub struct FanSnapshot {
    /// `<chip>/<n>`, stable across boots.
    pub key: String,
    /// Driver label (`CPU fan`), else `Fan <n>`.
    pub name: String,
    /// Motherboard, Graphics card, Laptop or the driver name.
    pub source: String,
    pub rpm: Option<f64>,
    pub max_rpm: Option<f64>,
    /// PWM duty cycle in percent.
    pub percent: Option<f64>,
    pub history: History,
    pub percent_history: History,
}

fn source(chip: &str) -> String {
    let board = [
        "nct", "it8", "f71", "w83", "asus", "gigabyte", "asrock", "nzxt",
    ];
    let laptop = [
        "thinkpad", "dell", "hp", "applesmc", "asus-nb", "acer", "msi",
    ];
    if laptop.iter().any(|p| chip.starts_with(p)) {
        "Laptop"
    } else if board.iter().any(|p| chip.starts_with(p)) {
        "Motherboard"
    } else if matches!(chip, "amdgpu" | "nouveau" | "radeon") {
        "Graphics card"
    } else {
        chip
    }
    .to_owned()
}

#[derive(Default)]
pub struct FanSampler {
    history: Vec<(String, History, History)>,
}

impl FanSampler {
    pub fn sample(&mut self, root: &Root, chips: &[Chip]) -> Vec<FanSnapshot> {
        let mut out = Vec::new();
        for chip in chips {
            for n in chip.indices(root, "fan", "_input") {
                let read = |f: String| chip.file(root, &f).map(|v| v as f64);
                let rpm = read(format!("fan{n}_input"));
                let percent =
                    read(format!("pwm{n}")).map(|p| (p / 255.0 * 100.0).clamp(0.0, 100.0));
                // A header with nothing plugged in reads 0 and has no duty.
                if rpm.unwrap_or(0.0) == 0.0 && percent.is_none() {
                    continue;
                }
                let key = format!("{}/{n}", chip.name);
                let i = match self.history.iter().position(|(k, ..)| *k == key) {
                    Some(i) => i,
                    None => {
                        self.history
                            .push((key.clone(), History::default(), History::default()));
                        self.history.len() - 1
                    }
                };
                let (_, h, ph) = &mut self.history[i];
                h.push(rpm.unwrap_or(0.0));
                ph.push(percent.unwrap_or(0.0));
                out.push(FanSnapshot {
                    name: chip
                        .label(root, "fan", n)
                        .unwrap_or_else(|| format!("Fan {n}")),
                    source: source(&chip.name),
                    rpm,
                    max_rpm: read(format!("fan{n}_max")).filter(|m| *m > 0.0),
                    percent,
                    history: h.clone(),
                    percent_history: ph.clone(),
                    key,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources() {
        assert_eq!(source("nct6798"), "Motherboard");
        assert_eq!(source("thinkpad"), "Laptop");
        assert_eq!(source("amdgpu"), "Graphics card");
        assert_eq!(source("corsaircpro"), "corsaircpro");
    }

    #[test]
    fn reads_speed_and_duty_and_skips_empty_headers() {
        let base = std::env::temp_dir().join(format!("taskmgr-fans-{}", std::process::id()));
        let dir = base.join("sys/class/hwmon/hwmon3");
        std::fs::create_dir_all(&dir).unwrap();
        for (f, v) in [
            ("name", "nct6798"),
            ("fan1_input", "1180"),
            ("fan1_label", "CPU fan"),
            ("pwm1", "128"),
            ("fan2_input", "0"),
        ] {
            std::fs::write(dir.join(f), v).unwrap();
        }
        let root = Root::new(&base);
        let fans = FanSampler::default().sample(&root, &crate::hwmon::chips(&root));
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(fans.len(), 1);
        let f = &fans[0];
        assert_eq!(
            (f.key.as_str(), f.name.as_str(), f.source.as_str()),
            ("nct6798/1", "CPU fan", "Motherboard")
        );
        assert_eq!(f.rpm, Some(1180.0));
        assert!((f.percent.unwrap() - 50.2).abs() < 0.1);
    }
}

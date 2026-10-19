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

fn find_program(name: &str, extra: &[&str]) -> Option<String> {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|d| format!("{d}/{name}"))
        .chain(extra.iter().map(|s| (*s).to_owned()))
        .find(|p| std::path::Path::new(p).is_file())
}

/// Hands the RAPL counters to the current user until the next reboot
/// (sysfs permissions are not persistent). pkexec asks for the password
/// through the desktop's polkit agent. Blocks until the dialog is answered.
pub fn grant_access(root: &Root) -> Result<(), String> {
    let pkexec = find_program("pkexec", &["/run/wrappers/bin/pkexec", "/usr/bin/pkexec"])
        .ok_or("pkexec est introuvable (paquet polkit).")?;
    let chown = find_program(
        "chown",
        &[
            "/run/current-system/sw/bin/chown",
            "/usr/bin/chown",
            "/bin/chown",
        ],
    )
    .ok_or("chown est introuvable.")?;
    let files: Vec<String> = root
        .list("/sys/class/powercap")
        .into_iter()
        .filter(|d| d.starts_with("intel-rapl:"))
        .map(|d| format!("/sys/class/powercap/{d}/energy_uj"))
        .filter(|f| root.exists(f))
        .collect();
    if files.is_empty() {
        return Err("Aucun compteur RAPL sur cette machine.".to_owned());
    }
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    let status = std::process::Command::new(pkexec)
        .arg(chown)
        .arg(uid.to_string())
        .args(&files)
        .status()
        .map_err(|e| e.to_string())?;
    match status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) => Err("Autorisation refusée ou annulée.".to_owned()),
        _ => Err(format!("La commande a échoué ({status}).")),
    }
}

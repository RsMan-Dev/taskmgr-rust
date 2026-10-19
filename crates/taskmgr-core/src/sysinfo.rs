use serde::Serialize;

use crate::cpu::{self, CpuInfo};
use crate::util::Root;

#[derive(Clone, Debug, Default, Serialize)]
pub struct SystemInfo {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub architecture: String,
    pub desktop: Option<String>,
    pub session: Option<String>,
    pub vendor: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub board: Option<String>,
    pub bios: Option<String>,
    pub firmware: String,
    pub cpu: CpuInfo,
    pub memory_total: u64,
}

fn os_release(root: &Root) -> Option<String> {
    let text = root
        .read("/etc/os-release")
        .or_else(|| root.read("/usr/lib/os-release"))?;
    text.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_owned())
}

/// Skips the placeholders firmware vendors leave.
fn dmi(root: &Root, field: &str) -> Option<String> {
    root.read_trimmed(format!("/sys/class/dmi/id/{field}"))
        .filter(|v| {
            let l = v.to_lowercase();
            !(l.contains("to be filled")
                || l.contains("default string")
                || l == "system product name")
        })
}

fn dmi_pair(root: &Root, vendor: &str, name: &str) -> Option<String> {
    match (dmi(root, vendor), dmi(root, name)) {
        (Some(v), Some(n)) => Some(format!("{v} {n}")),
        (v, n) => v.or(n),
    }
}

pub fn system_info(root: &Root) -> SystemInfo {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let bios = match (
        dmi_pair(root, "bios_vendor", "bios_version"),
        root.read_trimmed("/sys/class/dmi/id/bios_date"),
    ) {
        (Some(b), Some(d)) => Some(format!("{b} ({d})")),
        (b, _) => b,
    };
    let memory_total = root
        .read("/proc/meminfo")
        .and_then(|m| {
            m.lines()
                .next()?
                .split_whitespace()
                .nth(1)?
                .parse::<u64>()
                .ok()
        })
        .unwrap_or(0)
        * 1024;
    SystemInfo {
        hostname: root.read_trimmed("/proc/sys/kernel/hostname"),
        os: os_release(root),
        kernel: root.read_trimmed("/proc/sys/kernel/osrelease"),
        architecture: std::env::consts::ARCH.to_owned(),
        desktop: env("XDG_CURRENT_DESKTOP"),
        session: env("XDG_SESSION_TYPE"),
        vendor: dmi(root, "sys_vendor"),
        manufacturer: dmi_pair(root, "sys_vendor", "product_name"),
        product: root.read_trimmed("/sys/class/dmi/id/product_family"),
        board: dmi_pair(root, "board_vendor", "board_name"),
        bios,
        firmware: if root.exists("/sys/firmware/efi") {
            "UEFI"
        } else {
            "BIOS"
        }
        .to_owned(),
        cpu: cpu::info(root),
        memory_total,
    }
}

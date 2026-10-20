//! Core voltage from model-specific registers, for machines without a
//! sensor driver. Root only (the root helper reads them). On AMD this is
//! the voltage the core requests (VID), not a measurement.

use crate::util::Root;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Intel,
    /// Zen 1 to 3: SVI2 encoding.
    AmdSvi2,
    /// Zen 4 and 5: SVI3 encoding (community-documented, not by AMD).
    AmdSvi3,
}

fn kind(root: &Root) -> Option<Kind> {
    let info = root.read("/proc/cpuinfo")?;
    let field = |k: &str| {
        info.lines().find_map(|l| {
            l.split_once(':')
                .filter(|(a, _)| a.trim() == k)
                .map(|(_, v)| v.trim().to_owned())
        })
    };
    let family: u32 = field("cpu family")?.parse().ok()?;
    let model: u32 = field("model")?.parse().ok()?;
    match field("vendor_id")?.as_str() {
        "GenuineIntel" => Some(Kind::Intel),
        "AuthenticAMD" => match (family, model) {
            (0x17, _) => Some(Kind::AmdSvi2),
            (0x19, 0x10..=0x1f | 0x60..=0x7f | 0xa0..=0xaf) => Some(Kind::AmdSvi3),
            (0x19, _) => Some(Kind::AmdSvi2),
            (0x1a, _) => Some(Kind::AmdSvi3),
            _ => None,
        },
        _ => None,
    }
}

fn register(kind: Kind) -> u64 {
    match kind {
        Kind::Intel => 0x198,
        Kind::AmdSvi2 | Kind::AmdSvi3 => 0xc001_0293,
    }
}

fn decode(kind: Kind, raw: u64) -> Option<f64> {
    let volts = match kind {
        Kind::Intel => ((raw >> 32) & 0xffff) as f64 / 8192.0,
        Kind::AmdSvi2 => 1.55 - ((raw >> 14) & 0xff) as f64 * 0.006_25,
        Kind::AmdSvi3 => 0.245 + ((raw >> 14) & 0xff) as f64 * 0.005,
    };
    (0.5..=1.7).contains(&volts).then_some(volts)
}

/// Loads the msr module when its device files are missing (root).
pub fn ensure_module(root: &Root) {
    if !root.exists("/dev/cpu/0/msr")
        && let Some(modprobe) = crate::util::find_program(
            "modprobe",
            &[
                "/run/current-system/sw/bin/modprobe",
                "/usr/sbin/modprobe",
                "/sbin/modprobe",
            ],
        )
    {
        let _ = std::process::Command::new(modprobe).arg("msr").status();
    }
}

/// Voltage of each logical processor, or None when unsupported or denied.
pub fn core_voltages(root: &Root) -> Option<Vec<f64>> {
    use std::os::unix::fs::FileExt;
    let kind = kind(root)?;
    let cpus = root
        .list("/dev/cpu")
        .into_iter()
        .filter(|d| d.bytes().all(|b| b.is_ascii_digit()))
        .count();
    let mut out = Vec::new();
    for cpu in 0..cpus {
        let file = std::fs::File::open(root.path(format!("/dev/cpu/{cpu}/msr"))).ok()?;
        let mut buf = [0u8; 8];
        file.read_exact_at(&mut buf, register(kind)).ok()?;
        if let Some(v) = decode(kind, u64::from_le_bytes(buf)) {
            out.push(v);
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding() {
        assert_eq!(decode(Kind::Intel, 0x2000_0000_0000), Some(1.0));
        assert_eq!(decode(Kind::AmdSvi2, 0x40 << 14), Some(1.55 - 0.4));
        assert!((decode(Kind::AmdSvi3, 180 << 14).unwrap() - 1.145).abs() < 1e-9);
        assert_eq!(decode(Kind::Intel, 0), None);
    }
}

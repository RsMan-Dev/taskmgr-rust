//! What the processor is: model, topology, caches, virtualization.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::util::Root;

#[derive(Clone, Debug, Default, Serialize)]
pub struct CacheLevel {
    pub name: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CpuInfo {
    pub model: String,
    pub sockets: usize,
    pub cores: usize,
    pub logical: usize,
    pub max_mhz: Option<f64>,
    pub virtualization: Option<String>,
    /// Hypervisor this system runs in, when it is a virtual machine.
    pub virtual_machine: Option<String>,
    pub caches: Vec<CacheLevel>,
    /// Per logical processor on hybrid Intel CPUs: "P" or "E"; empty otherwise.
    pub core_types: Vec<String>,
    /// Logical processors of each NUMA node; empty with a single node.
    pub numa: Vec<Vec<usize>>,
}

/// "0-3,8,10-11" to [0, 1, 2, 3, 8, 10, 11].
fn cpu_list(s: &str) -> Vec<usize> {
    s.trim()
        .split(',')
        .filter_map(|r| match r.split_once('-') {
            Some((a, b)) => Some((a.parse().ok()?..=b.parse().ok()?).collect::<Vec<usize>>()),
            None => r.parse().ok().map(|v| vec![v]),
        })
        .flatten()
        .collect()
}

/// ARM cores have no "model name": decode implementer and part numbers.
fn arm_core_name(implementer: &str, part: &str) -> Option<String> {
    let part = u32::from_str_radix(part.trim_start_matches("0x"), 16).ok()?;
    let name = match (implementer, part) {
        ("0x41", 0xd03) => "Cortex-A53",
        ("0x41", 0xd04) => "Cortex-A35",
        ("0x41", 0xd05) => "Cortex-A55",
        ("0x41", 0xd07) => "Cortex-A57",
        ("0x41", 0xd08) => "Cortex-A72",
        ("0x41", 0xd09) => "Cortex-A73",
        ("0x41", 0xd0a) => "Cortex-A75",
        ("0x41", 0xd0b) => "Cortex-A76",
        ("0x41", 0xd0c) => "Neoverse-N1",
        ("0x41", 0xd0d) => "Cortex-A77",
        ("0x41", 0xd40) => "Neoverse-V1",
        ("0x41", 0xd41) => "Cortex-A78",
        ("0x41", 0xd44) => "Cortex-X1",
        ("0x41", 0xd46) => "Cortex-A510",
        ("0x41", 0xd47) => "Cortex-A710",
        ("0x41", 0xd48) => "Cortex-X2",
        ("0x41", 0xd49) => "Neoverse-N2",
        ("0x41", 0xd4f) => "Neoverse-V2",
        ("0x61", _) => return Some("Apple".to_owned()),
        ("0x51", _) => return Some("Qualcomm".to_owned()),
        ("0x41", _) => return Some(format!("ARM core 0x{part:03x}")),
        _ => return None,
    };
    Some(format!("ARM {name}"))
}

/// Named from the firmware the hypervisor presents.
fn hypervisor(root: &Root) -> String {
    let dmi = |f: &str| {
        root.read_trimmed(format!("/sys/class/dmi/id/{f}"))
            .unwrap_or_default()
    };
    let text = format!("{} {}", dmi("sys_vendor"), dmi("product_name")).to_lowercase();
    let known = [
        ("qemu", "QEMU/KVM"),
        ("kvm", "KVM"),
        ("vmware", "VMware"),
        ("virtualbox", "VirtualBox"),
        ("innotek", "VirtualBox"),
        ("microsoft", "Hyper-V"),
        ("xen", "Xen"),
        ("parallels", "Parallels"),
        ("amazon", "Amazon EC2"),
        ("google", "Google Compute Engine"),
    ];
    known
        .iter()
        .find(|(k, _)| text.contains(k))
        .map(|(_, n)| (*n).to_owned())
        .or_else(|| root.read_trimmed("/sys/hypervisor/type"))
        .unwrap_or_else(|| "Unknown hypervisor".to_owned())
}

pub fn info(root: &Root) -> CpuInfo {
    let cpuinfo = root.read("/proc/cpuinfo").unwrap_or_default();
    let mut implementer: Option<String> = None;
    let mut part: Option<String> = None;
    let mut model = None;
    let mut logical = 0;
    let mut sockets = BTreeSet::new();
    let mut cores = BTreeSet::new();
    let mut physical = String::new();
    let mut flags = String::new();
    for line in cpuinfo.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "processor" => logical += 1,
            "model name" if model.is_none() => model = Some(v.to_owned()),
            "CPU implementer" => implementer = Some(v.to_owned()),
            "CPU part" if part.is_none() => part = Some(v.to_owned()),
            "physical id" => {
                physical = v.to_owned();
                sockets.insert(v.to_owned());
            }
            "core id" => {
                cores.insert((physical.clone(), v.to_owned()));
            }
            "flags" if flags.is_empty() => flags = v.to_owned(),
            _ => {}
        }
    }
    let virtualization = flags.split_whitespace().find_map(|f| match f {
        "svm" => Some("AMD-V".to_owned()),
        "vmx" => Some("VT-x".to_owned()),
        _ => None,
    });
    // amd_pstate reports the boost ceiling separately; cpuinfo_max_freq
    // drops to the base clock when boost is off for the policy.
    let max_mhz = ["amd_pstate_max_freq", "cpuinfo_max_freq"]
        .iter()
        .find_map(|f| root.read_u64(format!("/sys/devices/system/cpu/cpu0/cpufreq/{f}")))
        .map(|khz| khz as f64 / 1000.0);
    CpuInfo {
        model: model
            .or_else(|| arm_core_name(implementer.as_deref()?, part.as_deref()?))
            .or_else(|| {
                root.read_trimmed("/sys/firmware/devicetree/base/model")
                    .map(|m| m.trim_end_matches('\0').to_owned())
            })
            .unwrap_or_else(|| "CPU".to_owned()),
        sockets: sockets.len().max(1),
        cores: if cores.is_empty() {
            logical
        } else {
            cores.len()
        },
        logical,
        max_mhz,
        virtualization,
        virtual_machine: flags
            .split_whitespace()
            .any(|f| f == "hypervisor")
            .then(|| hypervisor(root)),
        caches: caches(root),
        core_types: {
            let p = root
                .read("/sys/devices/cpu_core/cpus")
                .map(|s| cpu_list(&s))
                .unwrap_or_default();
            let e = root
                .read("/sys/devices/cpu_atom/cpus")
                .map(|s| cpu_list(&s))
                .unwrap_or_default();
            if e.is_empty() {
                Vec::new()
            } else {
                (0..logical)
                    .map(|i| if p.contains(&i) { "P" } else { "E" }.to_owned())
                    .collect()
            }
        },
        numa: {
            let nodes: Vec<Vec<usize>> = root
                .list("/sys/devices/system/node")
                .into_iter()
                .filter(|n| {
                    n.strip_prefix("node")
                        .is_some_and(|i| i.parse::<u32>().is_ok())
                })
                .filter_map(|n| {
                    root.read(format!("/sys/devices/system/node/{n}/cpulist"))
                        .map(|s| cpu_list(&s))
                })
                .filter(|l| !l.is_empty())
                .collect();
            if nodes.len() > 1 { nodes } else { Vec::new() }
        },
    }
}

fn caches(root: &Root) -> Vec<CacheLevel> {
    let mut seen = BTreeSet::new();
    let mut totals: Vec<(String, u64)> = Vec::new();
    for cpu in root.list("/sys/devices/system/cpu") {
        if !cpu
            .strip_prefix("cpu")
            .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty())
        {
            continue;
        }
        let base = format!("/sys/devices/system/cpu/{cpu}/cache");
        for idx in root
            .list(&base)
            .into_iter()
            .filter(|i| i.starts_with("index"))
        {
            let d = format!("{base}/{idx}");
            let (Some(level), Some(kind), Some(size), Some(shared)) = (
                root.read_trimmed(format!("{d}/level")),
                root.read_trimmed(format!("{d}/type")),
                root.read_trimmed(format!("{d}/size")),
                root.read_trimmed(format!("{d}/shared_cpu_list")),
            ) else {
                continue;
            };
            if !seen.insert((level.clone(), kind.clone(), shared)) {
                continue;
            }
            let name = match kind.as_str() {
                "Data" => format!("L{level}d"),
                "Instruction" => format!("L{level}i"),
                _ => format!("L{level}"),
            };
            let bytes = parse_size(&size);
            match totals.iter_mut().find(|(n, _)| *n == name) {
                Some((_, b)) => *b += bytes,
                None => totals.push((name, bytes)),
            }
        }
    }
    let mut out: Vec<CacheLevel> = Vec::new();
    for (name, bytes) in totals {
        let name = name.trim_end_matches(['d', 'i']).to_owned();
        match out.iter_mut().find(|c| c.name == name) {
            Some(c) => c.bytes += bytes,
            None => out.push(CacheLevel { name, bytes }),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn parse_size(s: &str) -> u64 {
    let (num, mult) = match s.chars().last() {
        Some('K') => (&s[..s.len() - 1], 1024),
        Some('M') => (&s[..s.len() - 1], 1024 * 1024),
        _ => (s, 1),
    };
    num.parse::<u64>().unwrap_or(0) * mult
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_names() {
        assert_eq!(
            arm_core_name("0x41", "0xd09").as_deref(),
            Some("ARM Cortex-A73")
        );
        assert_eq!(arm_core_name("0x99", "0x1"), None);
    }

    #[test]
    fn cpu_lists() {
        assert_eq!(cpu_list("0-3,8,10-11\n"), vec![0, 1, 2, 3, 8, 10, 11]);
    }

    #[test]
    fn cache_sizes() {
        assert_eq!(parse_size("48K"), 48 * 1024);
        assert_eq!(parse_size("32M"), 32 * 1024 * 1024);
    }
}

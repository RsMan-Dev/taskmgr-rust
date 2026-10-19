//! Processor identity, topology, caches, instruction sets and the state of
//! the hardware vulnerabilities, from /proc/cpuinfo and sysfs.

use super::Node;
use crate::Root;
use crate::cpu::CpuInfo;

const CPU0: &str = "/sys/devices/system/cpu/cpu0";

/// Instruction set extensions worth knowing, by their cpuinfo flag.
const EXTENSIONS: [(&str, &str); 14] = [
    ("sse4_2", "SSE4.2"),
    ("avx", "AVX"),
    ("avx2", "AVX2"),
    ("fma", "FMA3"),
    ("bmi2", "BMI2"),
    ("avx512f", "AVX-512"),
    ("avx512_vnni", "AVX-512 VNNI"),
    ("avx512_bf16", "AVX-512 BF16"),
    ("avx_vnni", "AVX-VNNI"),
    ("amx_tile", "AMX"),
    ("aes", "AES-NI"),
    ("vaes", "VAES"),
    ("sha_ni", "SHA"),
    ("rdrand", "RDRAND"),
];

fn cpuinfo_field<'a>(cpuinfo: &'a str, key: &str) -> Option<&'a str> {
    cpuinfo
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == key).then(|| v.trim())
        })
}

fn khz(root: &Root, file: &str) -> Option<f64> {
    root.read_u64(format!("{CPU0}/cpufreq/{file}"))
        .map(|k| k as f64 / 1000.0)
}

/// One node per cache level and type, with how many instances there are.
fn caches(root: &Root, logical: usize) -> Node {
    let mut n = Node::new("Caches");
    for index in root.list(format!("{CPU0}/cache")) {
        if !index.starts_with("index") {
            continue;
        }
        let dir = format!("{CPU0}/cache/{index}");
        let read = |f: &str| root.read_trimmed(format!("{dir}/{f}"));
        let (Some(level), Some(kind), Some(size)) = (read("level"), read("type"), read("size"))
        else {
            continue;
        };
        let name = match kind.as_str() {
            "Data" => format!("L{level} data"),
            "Instruction" => format!("L{level} instruction"),
            _ => format!("L{level}"),
        };
        let shared = read("shared_cpu_list")
            .map(|l| {
                l.split(',')
                    .map(|r| match r.split_once('-') {
                        Some((a, b)) => {
                            b.parse::<usize>().unwrap_or(0) + 1 - a.parse::<usize>().unwrap_or(0)
                        }
                        None => 1,
                    })
                    .sum::<usize>()
            })
            .unwrap_or(1)
            .max(1);
        let count = (logical / shared).max(1);
        let mut c = Node::new(name);
        c.text(
            "Size",
            Some(if count > 1 {
                format!("{count} x {size}")
            } else {
                size
            }),
        )
        .text(
            "Associativity",
            read("ways_of_associativity").map(|w| format!("{w}-way")),
        )
        .text(
            "Line size",
            read("coherency_line_size").map(|b| format!("{b} B")),
        )
        .text("Shared by", Some(format!("{shared} logical processors")));
        n.child(c);
    }
    n
}

fn vulnerabilities(root: &Root) -> Node {
    let dir = "/sys/devices/system/cpu/vulnerabilities";
    let mut n = Node::new("Vulnerabilities");
    for name in root.list(dir) {
        n.text(&name, root.read_trimmed(format!("{dir}/{name}")));
    }
    n
}

pub(super) fn processor(root: &Root, info: &CpuInfo) -> Node {
    let cpuinfo = root.read("/proc/cpuinfo").unwrap_or_default();
    let field = |k: &str| cpuinfo_field(&cpuinfo, k).map(str::to_owned);
    let mut n = Node::new("Processor").live("cpu");
    n.text("Model", Some(info.model.clone()))
        .text("Vendor", field("vendor_id"))
        .text("Family", field("cpu family"))
        .text("Model number", field("model"))
        .text("Stepping", field("stepping"))
        .text("Microcode", field("microcode"))
        .text("Sockets", Some(info.sockets.to_string()))
        .text("Cores", Some(info.cores.to_string()))
        .text("Logical processors", Some(info.logical.to_string()))
        .number("Base speed", khz(root, "base_frequency"), "mhz")
        .number("Minimum speed", khz(root, "cpuinfo_min_freq"), "mhz")
        .number("Maximum speed", info.max_mhz, "mhz")
        .text(
            "Frequency driver",
            root.read_trimmed(format!("{CPU0}/cpufreq/scaling_driver")),
        )
        .text(
            "Governor",
            root.read_trimmed(format!("{CPU0}/cpufreq/scaling_governor")),
        )
        .text("Virtualization", info.virtualization.clone());
    if info.numa.len() > 1 {
        n.text("NUMA nodes", Some(info.numa.len().to_string()));
    }
    let flags: Vec<&str> = cpuinfo_field(&cpuinfo, "flags")
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    let extensions: Vec<&str> = EXTENSIONS
        .iter()
        .filter(|(flag, _)| flags.contains(flag))
        .map(|(_, name)| *name)
        .collect();
    n.text("Instruction sets", Some(extensions.join(", ")));
    n.child(caches(root, info.logical));
    n.child(vulnerabilities(root));
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_first_processor_block() {
        let text = "processor\t: 0\nvendor_id\t: AuthenticAMD\ncpu family\t: 26\n\nprocessor\t: 1\nvendor_id\t: Other\n";
        assert_eq!(cpuinfo_field(text, "vendor_id"), Some("AuthenticAMD"));
        assert_eq!(cpuinfo_field(text, "cpu family"), Some("26"));
        assert_eq!(cpuinfo_field(text, "flags"), None);
    }
}

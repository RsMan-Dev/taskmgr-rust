//! Graphics cards, storage (with partitions) and network adapters.

use super::Node;
use crate::Root;
use crate::disk::DiskSnapshot;
use crate::gpu::GpuSnapshot;
use crate::net::NetSnapshot;

/// "16.0 GT/s PCIe x16 (maximum 16.0 GT/s PCIe x16)".
fn pcie_link(root: &Root, slot: &str) -> Option<String> {
    let dev = format!("/sys/bus/pci/devices/{slot}");
    let read = |f: &str| root.read_trimmed(format!("{dev}/{f}"));
    let now = format!(
        "{} x{}",
        read("current_link_speed")?,
        read("current_link_width")?
    );
    let max = format!("{} x{}", read("max_link_speed")?, read("max_link_width")?);
    Some(if now == max {
        now
    } else {
        format!("{now} (maximum {max})")
    })
}

pub(super) fn graphics(root: &Root, gpus: &[GpuSnapshot]) -> Node {
    let mut n = Node::new("Graphics");
    for g in gpus {
        let mut c =
            Node::new(format!("GPU {}: {}", g.index, g.name)).live(format!("gpu:{}", g.pci_slot));
        c.text("Vendor", Some(g.vendor.clone()))
            .text(
                "Type",
                Some(if g.dedicated {
                    "Dedicated"
                } else {
                    "Integrated"
                }),
            )
            .text(
                "Driver",
                Some(match &g.driver_version {
                    Some(v) => format!("{} {v}", g.driver),
                    None => g.driver.clone(),
                }),
            )
            .number("Video memory", g.memory_total.map(|b| b as f64), "bytes")
            .number("Maximum clock", g.core_max_mhz, "mhz")
            .number("Maximum memory clock", g.memory_max_mhz, "mhz")
            .number("Power limit", g.power_limit_watts, "watts")
            .text(
                "Video BIOS",
                root.read_trimmed(format!("/sys/bus/pci/devices/{}/vbios_version", g.pci_slot)),
            )
            .text("PCI Express link", pcie_link(root, &g.pci_slot))
            .text("PCI slot", Some(g.pci_slot.clone()));
        n.child(c);
    }
    n
}

/// Partitions of a disk with their size, file system and mount point.
fn partitions(root: &Root, disk: &str) -> Node {
    let mut n = Node::new("Partitions");
    for part in root.list(format!("/sys/block/{disk}")) {
        if !part.starts_with(disk) {
            continue;
        }
        let dir = format!("/sys/block/{disk}/{part}");
        let size = root
            .read_u64(format!("{dir}/size"))
            .map(|s| s as f64 * 512.0);
        let udev = root
            .read_trimmed(format!("{dir}/dev"))
            .map(|d| format!("b{d}"));
        let prop = |k: &str| udev.as_ref().and_then(|id| root.udev_property(id, k));
        let mut p = Node::new(part.clone());
        p.number("Size", size, "bytes")
            .text("File system", prop("ID_FS_TYPE"))
            .text("Label", prop("ID_FS_LABEL"))
            .text("Partition name", prop("ID_PART_ENTRY_NAME"));
        n.child(p);
    }
    n
}

pub(super) fn storage(root: &Root, disks: &[DiskSnapshot]) -> Node {
    let mut n = Node::new("Storage");
    for d in disks {
        let dev = format!("/sys/block/{}", d.name);
        let read = |f: &str| root.read_trimmed(format!("{dev}/{f}"));
        let udev = read("dev").map(|x| format!("b{x}"));
        let prop = |k: &str| udev.as_ref().and_then(|id| root.udev_property(id, k));
        let mut c =
            Node::new(format!("Disk {}: {}", d.index, d.model)).live(format!("disk:{}", d.name));
        c.text("Device", Some(format!("/dev/{}", d.name)))
            .text("Type", Some(d.kind.clone()))
            .number("Capacity", Some(d.capacity as f64), "bytes")
            .text("Interface", prop("ID_BUS").map(|b| b.to_uppercase()))
            .text("Serial number", prop("ID_SERIAL_SHORT"))
            .text(
                "Firmware",
                read("device/firmware_rev").or_else(|| read("device/rev")),
            )
            .text(
                "Logical sector",
                read("queue/logical_block_size").map(|b| format!("{b} B")),
            )
            .text(
                "Physical sector",
                read("queue/physical_block_size").map(|b| format!("{b} B")),
            )
            .text(
                "I/O scheduler",
                read("queue/scheduler").and_then(|s| {
                    let active = s.split_once('[')?.1.split_once(']')?.0.to_owned();
                    Some(active)
                }),
            )
            .text(
                "System disk",
                Some(if d.system_disk { "Yes" } else { "No" }),
            );
        c.child(partitions(root, &d.name));
        n.child(c);
    }
    n
}

pub(super) fn network(root: &Root, nets: &[NetSnapshot]) -> Node {
    let mut n = Node::new("Network");
    for net in nets {
        let dir = format!("/sys/class/net/{}", net.name);
        let read = |f: &str| root.read_trimmed(format!("{dir}/{f}"));
        let mut c =
            Node::new(format!("{}: {}", net.name, net.adapter)).live(format!("net:{}", net.name));
        c.text("Type", Some(net.kind.clone()))
            .text("Driver", root.link_name(format!("{dir}/device/driver")))
            .text("MAC address", net.mac.clone())
            .number("Link speed", net.link_mbps.map(|m| m as f64), "mbps")
            .text("Duplex", read("duplex"))
            .text("MTU", read("mtu"))
            .text("IPv4 address", Some(net.ipv4.join(", ")))
            .text("IPv6 address", Some(net.ipv6.join(", ")))
            .text("PCI slot", root.link_name(format!("{dir}/device")));
        n.child(c);
    }
    n
}

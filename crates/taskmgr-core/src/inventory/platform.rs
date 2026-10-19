//! System, firmware, motherboard and memory modules (DMI, through sysfs and
//! the udev database, readable without root).

use super::Node;
use crate::{Root, Snapshot, SystemInfo};

const DMI: &str = "/sys/class/dmi/id";

fn dmi(root: &Root, field: &str) -> Option<String> {
    root.read_trimmed(format!("{DMI}/{field}"))
}

/// SMBIOS chassis types that matter for a task manager.
fn chassis(code: &str) -> Option<&'static str> {
    Some(match code.parse::<u32>().ok()? {
        3 => "Desktop",
        4 => "Low profile desktop",
        6 => "Mini tower",
        7 => "Tower",
        8 => "Portable",
        9 => "Laptop",
        10 => "Notebook",
        13 => "All in one",
        14 => "Sub notebook",
        17 => "Main server chassis",
        23 => "Rack mount chassis",
        30 => "Tablet",
        31 => "Convertible",
        32 => "Detachable",
        35 => "Mini PC",
        36 => "Stick PC",
        _ => return None,
    })
}

/// Fourth byte of the SecureBoot EFI variable (the first four are flags).
fn secure_boot(root: &Root) -> Option<bool> {
    let dir = "/sys/firmware/efi/efivars";
    let name = root
        .list(dir)
        .into_iter()
        .find(|n| n.starts_with("SecureBoot-"))?;
    let bytes = std::fs::read(root.path(format!("{dir}/{name}"))).ok()?;
    bytes.get(4).map(|b| *b == 1)
}

pub(super) fn system(root: &Root, info: &SystemInfo) -> Node {
    let mut n = Node::new("System");
    n.text("Device name", info.hostname.clone())
        .text("Operating system", info.os.clone())
        .text("Kernel", info.kernel.as_ref().map(|k| format!("Linux {k}")))
        .text("Architecture", Some(info.architecture.clone()))
        .text("Desktop", info.desktop.clone())
        .text("Session", info.session.clone())
        .text("Manufacturer", dmi(root, "sys_vendor"))
        .text("Product", dmi(root, "product_name"))
        .text("Version", dmi(root, "product_version"))
        .text("Family", dmi(root, "product_family"))
        .text(
            "Chassis",
            dmi(root, "chassis_type").as_deref().and_then(chassis),
        );
    n
}

pub(super) fn board(root: &Root, info: &SystemInfo) -> Node {
    let mut n = Node::new("Motherboard");
    n.text("Manufacturer", dmi(root, "board_vendor"))
        .text("Model", dmi(root, "board_name"))
        .text("Version", dmi(root, "board_version"));
    let mut fw = Node::new("Firmware");
    fw.text("Type", Some(info.firmware.clone()))
        .text("Vendor", dmi(root, "bios_vendor"))
        .text("Version", dmi(root, "bios_version"))
        .text("Date", dmi(root, "bios_date"))
        .text("Revision", dmi(root, "bios_release"));
    if let Some(on) = secure_boot(root) {
        fw.text("Secure Boot", Some(if on { "Enabled" } else { "Disabled" }));
    }
    n.child(fw);
    n
}

pub(super) fn memory(root: &Root, s: &Snapshot) -> Node {
    let m = &s.memory;
    let mut n = Node::new("Memory").live("memory");
    n.number("Usable", Some(m.total as f64), "bytes").number(
        "Swap",
        Some(m.swap_total as f64).filter(|v| *v > 0.0),
        "bytes",
    );
    let prop = |k: &str| root.udev_property("+dmi:id", k);
    n.number(
        "Maximum capacity",
        prop("MEMORY_ARRAY_MAX_CAPACITY").and_then(|v| v.parse().ok()),
        "bytes",
    )
    .text("Slots", prop("MEMORY_ARRAY_NUM_DEVICES"))
    .text("Error correction", prop("MEMORY_ARRAY_ERROR_CORRECTION"));
    let slots: usize = prop("MEMORY_ARRAY_NUM_DEVICES")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    for i in 0..slots.max(m.dimms.len()) {
        let p = |k: &str| prop(&format!("MEMORY_DEVICE_{i}_{k}"));
        let size: Option<f64> = p("SIZE").and_then(|v| v.parse().ok());
        let locator = p("LOCATOR").unwrap_or_else(|| format!("Slot {i}"));
        let bank = p("BANK_LOCATOR");
        // Slots are often named per channel (DIMM 0, DIMM 1 on A and B).
        let mut d = Node::new(match &bank {
            Some(b) => format!("{b} / {locator}"),
            None => locator.clone(),
        });
        if size.is_none_or(|s| s == 0.0) {
            d.text("State", Some("Empty"));
            n.child(d);
            continue;
        }
        let dimm = m
            .dimms
            .iter()
            .find(|x| x.locator == locator && x.bank == bank);
        d.number("Size", size, "bytes")
            .text("Type", p("TYPE"))
            .text("Detail", p("TYPE_DETAIL"))
            .text("Form factor", p("FORM_FACTOR"))
            .number("Speed", p("SPEED_MTS").and_then(|v| v.parse().ok()), "mts")
            .number(
                "Configured speed",
                p("CONFIGURED_SPEED_MTS").and_then(|v| v.parse().ok()),
                "mts",
            )
            .text("Manufacturer", dimm.and_then(|x| x.manufacturer.clone()))
            .text("Part number", p("PART_NUMBER"))
            .text("Serial number", p("SERIAL_NUMBER"))
            .text("Rank", p("RANK"))
            .text("Data width", p("DATA_WIDTH").map(|w| format!("{w} bits")));
        n.child(d);
    }
    n
}

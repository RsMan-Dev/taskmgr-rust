//! Displays (from their EDID), audio cards, USB and PCI devices.

use std::collections::BTreeMap;

use super::Node;
use crate::Root;

/// What an EDID block says about a monitor.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Edid {
    pub vendor: String,
    pub name: Option<String>,
    pub year: u32,
    pub width_cm: u8,
    pub height_cm: u8,
    /// Native mode: width, height, refresh rate in Hz.
    pub mode: Option<(u32, u32, f64)>,
}

pub(super) fn parse_edid(b: &[u8]) -> Option<Edid> {
    if b.len() < 128 || b[..8] != [0, 255, 255, 255, 255, 255, 255, 0] {
        return None;
    }
    let id = u16::from_be_bytes([b[8], b[9]]);
    let letter = |shift: u16| char::from(b'A' - 1 + ((id >> shift) & 0x1f) as u8);
    let vendor: String = [letter(10), letter(5), letter(0)].iter().collect();
    let mut edid = Edid {
        vendor,
        year: 1990 + u32::from(b[17]),
        width_cm: b[21],
        height_cm: b[22],
        ..Edid::default()
    };
    for d in b[54..126].chunks(18) {
        if d[0] != 0 || d[1] != 0 {
            if edid.mode.is_none() {
                let clock = f64::from(u16::from_le_bytes([d[0], d[1]])) * 10_000.0;
                let w = u32::from(d[2]) | (u32::from(d[4] & 0xf0) << 4);
                let hblank = u32::from(d[3]) | (u32::from(d[4] & 0x0f) << 8);
                let h = u32::from(d[5]) | (u32::from(d[7] & 0xf0) << 4);
                let vblank = u32::from(d[6]) | (u32::from(d[7] & 0x0f) << 8);
                let total = f64::from((w + hblank) * (h + vblank));
                edid.mode = (total > 0.0).then(|| (w, h, clock / total));
            }
        } else if d[3] == 0xfc {
            let name: String = d[5..18]
                .iter()
                .take_while(|c| **c != 0x0a)
                .map(|c| char::from(*c))
                .collect();
            edid.name = Some(name.trim().to_owned());
        }
    }
    Some(edid)
}

pub(super) fn displays(root: &Root) -> Node {
    let mut n = Node::new("Displays");
    for conn in root.list("/sys/class/drm") {
        let Some((_, port)) = conn.split_once('-') else {
            continue;
        };
        let dir = format!("/sys/class/drm/{conn}");
        if root.read_trimmed(format!("{dir}/status")).as_deref() != Some("connected") {
            continue;
        }
        let Some(edid) = std::fs::read(root.path(format!("{dir}/edid")))
            .ok()
            .and_then(|b| parse_edid(&b))
        else {
            continue;
        };
        let title = edid.name.clone().unwrap_or_else(|| edid.vendor.clone());
        let mut c = Node::new(format!("{port}: {title}"));
        c.text("Manufacturer code", Some(edid.vendor.clone()))
            .text("Year", Some(edid.year.to_string()))
            .text(
                "Native resolution",
                edid.mode
                    .map(|(w, h, hz)| format!("{w} x {h} @ {hz:.0} Hz")),
            )
            .text(
                "Size",
                (edid.width_cm > 0).then(|| {
                    let inches = f64::from(edid.width_cm).hypot(f64::from(edid.height_cm)) / 2.54;
                    format!("{} x {} cm ({inches:.0}\")", edid.width_cm, edid.height_cm)
                }),
            )
            .text("Connector", Some(port.to_owned()));
        n.child(c);
    }
    n
}

pub(super) fn audio(root: &Root) -> Node {
    let mut n = Node::new("Audio");
    let text = root.read("/proc/asound/cards").unwrap_or_default();
    for line in text.lines().filter(|l| l.contains("]:")) {
        let Some((_, rest)) = line.split_once("]:") else {
            continue;
        };
        let (driver, name) = rest.split_once(" - ").unwrap_or(("", rest));
        let mut c = Node::new(name.trim().to_owned());
        c.text("Driver", Some(driver.trim().to_owned()));
        n.child(c);
    }
    n
}

pub(super) fn usb(root: &Root) -> Node {
    let mut n = Node::new("USB devices");
    for dev in root.list("/sys/bus/usb/devices") {
        // Interfaces (1-2:1.0) and root hubs (usb1) are not devices.
        if dev.contains(':') || dev.starts_with("usb") {
            continue;
        }
        let dir = format!("/sys/bus/usb/devices/{dev}");
        let read = |f: &str| root.read_trimmed(format!("{dir}/{f}"));
        let Some(product) = read("product") else {
            continue;
        };
        let mut c = Node::new(product);
        c.text("Manufacturer", read("manufacturer"))
            .text(
                "ID",
                read("idVendor")
                    .zip(read("idProduct"))
                    .map(|(v, p)| format!("{v}:{p}")),
            )
            .text("Speed", read("speed").map(|s| format!("{s} Mb/s")))
            .text("Port", Some(dev.clone()));
        n.child(c);
    }
    n
}

/// Every PCI device, grouped by class.
pub(super) fn pci(root: &Root) -> Node {
    let mut by_class: BTreeMap<String, Vec<Node>> = BTreeMap::new();
    for slot in root.list("/sys/bus/pci/devices") {
        let id = format!("+pci:{slot}");
        let prop = |k: &str| root.udev_property(&id, k);
        let class = prop("ID_PCI_SUBCLASS_FROM_DATABASE")
            .or_else(|| prop("ID_PCI_CLASS_FROM_DATABASE"))
            .unwrap_or_else(|| "Other".to_owned());
        let name = prop("ID_MODEL_FROM_DATABASE").unwrap_or_else(|| slot.clone());
        let mut c = Node::new(name);
        c.text("Vendor", prop("ID_VENDOR_FROM_DATABASE"))
            .text(
                "Driver",
                root.link_name(format!("/sys/bus/pci/devices/{slot}/driver")),
            )
            .text("Slot", Some(slot.clone()));
        by_class.entry(class).or_default().push(c);
    }
    let mut n = Node::new("PCI devices");
    for (class, devices) in by_class {
        let mut g = Node::new(class);
        for d in devices {
            g.child(d);
        }
        n.child(g);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_vendor_name_size_and_mode() {
        let mut b = vec![0u8; 128];
        b[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
        // "DEL": D=4, E=5, L=12.
        let id: u16 = (4 << 10) | (5 << 5) | 12;
        b[8..10].copy_from_slice(&id.to_be_bytes());
        b[17] = 30;
        b[21] = 60;
        b[22] = 34;
        // 2560x1440 @ ~60 Hz: 241.5 MHz, blanking 160 x 41.
        b[54..56].copy_from_slice(&24150u16.to_le_bytes());
        b[56] = (2560 & 0xff) as u8;
        b[57] = 160;
        b[58] = (((2560 >> 8) as u8) << 4) & 0xf0;
        b[59] = (1440 & 0xff) as u8;
        b[60] = 41;
        b[61] = (((1440 >> 8) as u8) << 4) & 0xf0;
        b[72..77].copy_from_slice(&[0, 0, 0, 0xfc, 0]);
        b[77..86].copy_from_slice(b"Monitor\n ");
        let e = parse_edid(&b).unwrap();
        assert_eq!(
            (e.vendor.as_str(), e.year, e.name.as_deref()),
            ("DEL", 2020, Some("Monitor"))
        );
        let (w, h, hz) = e.mode.unwrap();
        assert_eq!((w, h), (2560, 1440));
        assert!((hz - 59.95).abs() < 0.1, "{hz}");
    }
}

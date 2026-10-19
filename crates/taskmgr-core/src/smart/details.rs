//! Every SMART attribute of one drive and its last self-test, for the
//! detailed view.

use std::collections::HashMap;

use serde::Serialize;
use zbus::{Connection, OwnedObjectPath, Variant};

use super::{ATA, Attribute, NVME, Objects, Properties, get};

#[derive(Clone, Debug, Default, Serialize)]
pub struct SmartAttribute {
    /// ATA attribute number; none on NVMe.
    pub id: Option<u8>,
    /// UDisks2 name (`reallocated-sector-count`, `percent_used`).
    pub name: String,
    pub value: Option<i32>,
    pub worst: Option<i32>,
    pub threshold: Option<i32>,
    /// Raw value with its unit: `4 h`, `31 °C`, `12 sectors`, `230`.
    pub raw: String,
    /// "ok", "failing" or "failed-past".
    pub state: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SmartDetails {
    /// "ata" or "nvme".
    pub kind: &'static str,
    /// success, aborted, interrupted, fatal, error_unknown,
    /// error_electrical, error_servo, error_read, error_handling,
    /// inprogress (ATA); success, aborted, fatal, unknown, inprogress (NVMe).
    pub selftest: Option<String>,
    pub selftest_remaining: Option<i32>,
    pub attributes: Vec<SmartAttribute>,
}

/// UDisks2 units: 2 milliseconds, 3 sectors, 4 millikelvin.
fn pretty(value: i64, unit: i32) -> String {
    match unit {
        2 => format!("{} h", value / 3_600_000),
        3 => format!("{value} sectors"),
        4 => format!("{:.0} °C", value as f64 / 1000.0 - 273.15),
        _ => value.to_string(),
    }
}

fn ata_attribute(a: &Attribute) -> SmartAttribute {
    let (id, name, _, value, worst, threshold, raw, unit, _) = a;
    let known = |v: &i32| *v >= 0;
    SmartAttribute {
        id: Some(*id),
        name: name.clone(),
        value: Some(*value).filter(known),
        worst: Some(*worst).filter(known),
        threshold: Some(*threshold).filter(known),
        raw: pretty(*raw, *unit),
        state: if *threshold > 0 && *value >= 0 && *value <= *threshold {
            "failing"
        } else if *threshold > 0 && *worst >= 0 && *worst <= *threshold {
            "failed-past"
        } else {
            "ok"
        },
    }
}

fn nvme_attribute(name: &str, v: &Variant) -> Option<SmartAttribute> {
    let raw = match v {
        Variant::U8(n) => n.to_string(),
        Variant::U16(n) if name.ends_with("temp") => pretty(i64::from(*n) * 1000, 4),
        Variant::U16(n) => n.to_string(),
        Variant::U32(n) => n.to_string(),
        Variant::U64(n) if name.starts_with("total_data") => format!("{:.1} TB", *n as f64 / 1e12),
        Variant::U64(n) if name == "ctrl_busy_time" => format!("{n} min"),
        Variant::U64(n) => n.to_string(),
        Variant::Array(a) => a
            .iter()
            .filter_map(|t| u16::try_from(t).ok().filter(|k| *k > 0))
            .map(|k| pretty(i64::from(k) * 1000, 4))
            .collect::<Vec<_>>()
            .join(", "),
        _ => return None,
    };
    Some(SmartAttribute {
        name: name.to_owned(),
        raw,
        state: "ok",
        ..SmartAttribute::default()
    })
}

async fn read(block: &str) -> zbus::Result<Option<SmartDetails>> {
    let conn = Connection::system().await?;
    let manager = zbus::Proxy::new(
        &conn,
        "org.freedesktop.UDisks2",
        "/org/freedesktop/UDisks2",
        "org.freedesktop.DBus.ObjectManager",
    )
    .await?;
    let objects: Objects = manager.call("GetManagedObjects", &()).await?;
    let path = format!("/org/freedesktop/UDisks2/block_devices/{block}");
    let drive = objects
        .iter()
        .find(|(p, _)| p.as_str() == path)
        .and_then(|(_, i)| {
            get::<zbus::ObjectPath>(i.get("org.freedesktop.UDisks2.Block")?, "Drive")
        })
        .map(OwnedObjectPath::from);
    let Some(drive) = drive else {
        return Ok(None);
    };
    let Some(interfaces) = objects.get(&drive) else {
        return Ok(None);
    };
    let none: HashMap<&str, Variant> = HashMap::new();
    let (kind, iface, props): (&str, &str, &Properties) =
        match (interfaces.get(ATA), interfaces.get(NVME)) {
            (Some(p), _) => ("ata", ATA, p),
            (_, Some(p)) => ("nvme", NVME, p),
            _ => return Ok(None),
        };
    let proxy = zbus::Proxy::new(&conn, "org.freedesktop.UDisks2", drive.as_str(), iface).await?;
    let attributes = if kind == "ata" {
        let list: Vec<Attribute> = proxy.call("SmartGetAttributes", &(none,)).await?;
        list.iter().map(ata_attribute).collect()
    } else {
        let map: Properties = proxy.call("SmartGetAttributes", &(none,)).await?;
        let mut list: Vec<SmartAttribute> = map
            .iter()
            .filter_map(|(k, v)| nvme_attribute(k, v))
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    };
    Ok(Some(SmartDetails {
        kind,
        selftest: get::<String>(props, "SmartSelftestStatus").filter(|s| !s.is_empty()),
        selftest_remaining: get::<i32>(props, "SmartSelftestPercentRemaining").filter(|p| *p >= 0),
        attributes,
    }))
}

/// SMART attributes of a block device (`sda`, `nvme0n1`), when UDisks2 has them.
pub fn details(block: &str) -> Option<SmartDetails> {
    zbus::block_on(read(block)).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ata_states_and_units() {
        let a = |value, worst, threshold, raw, unit| {
            ata_attribute(&(
                5,
                "x".into(),
                0,
                value,
                worst,
                threshold,
                raw,
                unit,
                HashMap::new(),
            ))
        };
        assert_eq!(a(100, 100, 10, 0, 3).state, "ok");
        assert_eq!(a(9, 9, 10, 12, 3).state, "failing");
        assert_eq!(a(70, 43, 45, 0, 1).state, "failed-past");
        assert_eq!(a(65, 65, 0, 110_426_400_000, 2).raw, "30674 h");
        assert_eq!(a(70, 70, 0, 303_150, 4).raw, "30 °C");
    }
}

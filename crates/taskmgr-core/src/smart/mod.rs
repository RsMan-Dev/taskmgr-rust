//! Drive health (SMART) through UDisks2, which reads it as root and
//! publishes it to every user: no root needed here.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

mod details;

pub use details::{SmartAttribute, SmartDetails, details};
use zbus::Connection;
use zbus::fdo::ObjectManagerProxy;
use zbus::{ObjectPath, OwnedObjectPath, OwnedVariant, Variant};

use crate::dbus::udisks_ata::AtaProxy;
use crate::dbus::udisks_nvme::ControllerProxy;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Health {
    /// "good", "warning" or "failing".
    pub status: &'static str,
    /// Why the status is not "good": failing, attributes-failing,
    /// attributes-failed-past, bad-sectors, critical-warning, spare-low,
    /// worn, media-errors; overheated-past is a note only.
    pub reasons: Vec<&'static str>,
    pub power_on_hours: Option<u64>,
    /// NVMe: share of the rated endurance used, may exceed 100.
    pub wear_percent: Option<u64>,
    pub spare_percent: Option<u64>,
    pub written_bytes: Option<u64>,
    pub bad_sectors: Option<i64>,
    pub unsafe_shutdowns: Option<u64>,
    pub power_cycles: Option<u64>,
    /// Celsius, for drives without a hwmon sensor.
    pub temperature: Option<f64>,
}

type Properties = HashMap<String, OwnedVariant>;
type Objects = zbus::fdo::ManagedObjects;

const BUS: &str = "org.freedesktop.UDisks2";
const ATA: &str = "org.freedesktop.UDisks2.Drive.Ata";
const NVME: &str = "org.freedesktop.UDisks2.NVMe.Controller";

fn get<'a, T>(props: &'a Properties, key: &str) -> Option<T>
where
    T: TryFrom<&'a Variant<'a>>,
{
    props.get(key).and_then(|v| T::try_from(v).ok())
}

async fn managed_objects(conn: &Connection) -> zbus::Result<Objects> {
    let manager = ObjectManagerProxy::builder(conn)
        .destination(BUS)
        .path("/org/freedesktop/UDisks2")
        .build()
        .await?;
    Ok(manager.get_managed_objects().await?)
}

impl Health {
    fn finish(mut self, failing: &[&'static str]) -> Self {
        self.status = if self.reasons.iter().any(|r| failing.contains(r)) {
            "failing"
        } else if self.reasons.iter().all(|r| *r == "overheated-past") {
            "good"
        } else {
            "warning"
        };
        self
    }
}

/// ATA attribute: id, name, flags, value, worst, threshold, pretty, unit,
/// expansion.
type Attribute = (
    u8,
    String,
    u16,
    i32,
    i32,
    i32,
    i64,
    i32,
    HashMap<String, OwnedVariant>,
);

/// Temperature attributes (190, 194) that went over their limit once say
/// the drive overheated, not that it wears out.
async fn only_overheated(conn: &Connection, path: &OwnedObjectPath) -> bool {
    let proxy = AtaProxy::builder(conn).destination(BUS).path(path.as_str());
    let Ok(proxy) = proxy.build().await else {
        return false;
    };
    let Ok(attributes) = proxy.smart_get_attributes(HashMap::new()).await else {
        return false;
    };
    let past: Vec<u8> = attributes
        .iter()
        .filter(|a| a.5 > 0 && a.4 <= a.5)
        .map(|a| a.0)
        .collect();
    !past.is_empty() && past.iter().all(|id| matches!(id, 190 | 194))
}

async fn ata(conn: &Connection, path: &OwnedObjectPath, p: &Properties) -> Option<Health> {
    if !get::<bool>(p, "SmartSupported")? || !get::<bool>(p, "SmartEnabled")? {
        return None;
    }
    if get::<u64>(p, "SmartUpdated").unwrap_or(0) == 0 {
        return None;
    }
    let mut h = Health {
        power_on_hours: get::<u64>(p, "SmartPowerOnSeconds").map(|s| s / 3600),
        bad_sectors: get::<i64>(p, "SmartNumBadSectors").filter(|n| *n >= 0),
        temperature: get::<f64>(p, "SmartTemperature")
            .filter(|k| *k > 0.0)
            .map(|k| k - 273.15),
        ..Health::default()
    };
    if get::<bool>(p, "SmartFailing") == Some(true) {
        h.reasons.push("failing");
    }
    if get::<i32>(p, "SmartNumAttributesFailing").unwrap_or(0) > 0 {
        h.reasons.push("attributes-failing");
    }
    if get::<i32>(p, "SmartNumAttributesFailedInThePast").unwrap_or(0) > 0 {
        h.reasons.push(if only_overheated(conn, path).await {
            "overheated-past"
        } else {
            "attributes-failed-past"
        });
    }
    if h.bad_sectors.unwrap_or(0) > 0 {
        h.reasons.push("bad-sectors");
    }
    Some(h.finish(&["failing", "attributes-failing"]))
}

async fn nvme(conn: &Connection, path: &OwnedObjectPath, p: &Properties) -> Option<Health> {
    if get::<u64>(p, "SmartUpdated").unwrap_or(0) == 0 {
        return None;
    }
    let proxy = ControllerProxy::builder(conn)
        .destination(BUS)
        .path(path.as_str())
        .build()
        .await
        .ok()?;
    let a: Properties = proxy.smart_get_attributes(HashMap::new()).await.ok()?;
    let byte = |k: &str| get::<u8>(&a, k).map(u64::from);
    let mut h = Health {
        power_on_hours: get::<u64>(p, "SmartPowerOnHours"),
        wear_percent: byte("percent_used"),
        spare_percent: byte("avail_spare"),
        // The controller counts in units of 1000 sectors of 512 bytes;
        // UDisks2 already converts to bytes.
        written_bytes: get::<u64>(&a, "total_data_written"),
        unsafe_shutdowns: get::<u64>(&a, "unsafe_shutdowns"),
        power_cycles: get::<u64>(&a, "power_cycles"),
        temperature: get::<u16>(p, "SmartTemperature")
            .filter(|k| *k > 0)
            .map(|k| f64::from(k) - 273.15),
        ..Health::default()
    };
    let warnings: Vec<String> = p
        .get("SmartCriticalWarning")
        .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
        .unwrap_or_default();
    if !warnings.is_empty() {
        h.reasons.push("critical-warning");
    }
    if let (Some(spare), Some(threshold)) = (h.spare_percent, byte("spare_thresh"))
        && spare < threshold
    {
        h.reasons.push("spare-low");
    }
    if h.wear_percent.unwrap_or(0) >= 90 {
        h.reasons.push("worn");
    }
    if get::<u64>(&a, "media_errors").unwrap_or(0) > 0 {
        h.reasons.push("media-errors");
    }
    Some(h.finish(&["critical-warning", "spare-low"]))
}

async fn read_bus() -> zbus::Result<BTreeMap<String, Health>> {
    let conn = Connection::system().await?;
    let objects = managed_objects(&conn).await?;
    let mut drives: HashMap<&str, Health> = HashMap::new();
    for (path, interfaces) in &objects {
        let health = if let Some(p) = interfaces.get(ATA) {
            ata(&conn, path, p).await
        } else if let Some(p) = interfaces.get(NVME) {
            nvme(&conn, path, p).await
        } else {
            None
        };
        if let Some(h) = health {
            drives.insert(path.as_str(), h);
        }
    }
    // Keyed by kernel name (sda, nvme0n1), as the disk list is.
    let mut out = BTreeMap::new();
    for (path, interfaces) in &objects {
        let Some(block) = interfaces.get("org.freedesktop.UDisks2.Block") else {
            continue;
        };
        if interfaces.contains_key("org.freedesktop.UDisks2.Partition") {
            continue;
        }
        let drive = get::<ObjectPath>(block, "Drive");
        if let Some(h) = drive.and_then(|d| drives.get(d.as_str())) {
            let name = path.as_str().rsplit('/').next().unwrap_or_default();
            out.insert(name.to_owned(), h.clone());
        }
    }
    Ok(out)
}

/// Health of every drive UDisks2 knows, by block device name. Empty when
/// UDisks2 is not running.
pub fn read() -> BTreeMap<String, Health> {
    zbus::block_on(read_bus()).unwrap_or_default()
}

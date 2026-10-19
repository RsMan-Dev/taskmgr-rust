//! Wi-Fi link: network name, band, rate and signal from NetworkManager,
//! else the signal alone from /proc/net/wireless.

use serde::Serialize;
use zbus::Connection;
use zbus::OwnedObjectPath;

use crate::util::Root;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Wireless {
    pub ssid: Option<String>,
    pub frequency_mhz: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub signal_percent: Option<u8>,
}

const NM: &str = "org.freedesktop.NetworkManager";

async fn property<T>(conn: &Connection, path: &str, iface: &str, name: &str) -> zbus::Result<T>
where
    T: TryFrom<zbus::OwnedVariant> + zbus::zvariant::Type + for<'de> serde::Deserialize<'de>,
    T::Error: Into<zbus::Error>,
{
    zbus::Proxy::new(conn, NM, path, iface)
        .await?
        .get_property(name)
        .await
}

async fn from_network_manager(iface: &str) -> zbus::Result<Wireless> {
    let conn = Connection::system().await?;
    let nm = zbus::Proxy::new(&conn, NM, "/org/freedesktop/NetworkManager", NM).await?;
    let device: OwnedObjectPath = nm.call("GetDeviceByIpIface", &(iface,)).await?;
    let wifi = "org.freedesktop.NetworkManager.Device.Wireless";
    let bitrate: u32 = property(&conn, device.as_str(), wifi, "Bitrate").await?;
    let ap: OwnedObjectPath = property(&conn, device.as_str(), wifi, "ActiveAccessPoint").await?;
    let mut w = Wireless {
        bitrate_kbps: (bitrate > 0).then_some(bitrate),
        ..Wireless::default()
    };
    if ap.as_str() != "/" {
        let point = "org.freedesktop.NetworkManager.AccessPoint";
        let ssid: Vec<u8> = property(&conn, ap.as_str(), point, "Ssid").await?;
        w.ssid = Some(String::from_utf8_lossy(&ssid).into_owned()).filter(|s| !s.is_empty());
        w.frequency_mhz = property(&conn, ap.as_str(), point, "Frequency").await.ok();
        w.signal_percent = property(&conn, ap.as_str(), point, "Strength").await.ok();
    }
    Ok(w)
}

/// Link quality on the usual 0-70 scale of /proc/net/wireless.
fn proc_signal(text: &str, iface: &str) -> Option<u8> {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("{iface}:")))?;
    let quality: f64 = line
        .split_whitespace()
        .nth(2)?
        .trim_end_matches('.')
        .parse()
        .ok()?;
    Some((quality / 70.0 * 100.0).clamp(0.0, 100.0) as u8)
}

pub fn read(root: &Root, iface: &str) -> Wireless {
    let mut w = zbus::block_on(from_network_manager(iface)).unwrap_or_default();
    if w.signal_percent.is_none() {
        w.signal_percent = root
            .read("/proc/net/wireless")
            .and_then(|t| proc_signal(&t, iface));
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_from_proc() {
        let text = "Inter-| sta-|   Quality        |\n face | tus | link level noise |\n wlan0: 0000   56.  -54.  -256        0      0      0\n";
        assert_eq!(proc_signal(text, "wlan0"), Some(80));
        assert_eq!(proc_signal(text, "wlan1"), None);
    }
}

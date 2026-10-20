//! Wi-Fi link: network name, band, rate and signal from NetworkManager,
//! else the signal alone from /proc/net/wireless.

use serde::Serialize;
use zbus::Connection;

use crate::dbus::network_manager::NetworkManagerProxy;
use crate::dbus::nm_access_point::AccessPointProxy;
use crate::dbus::nm_wireless::WirelessProxy;
use crate::util::Root;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Wireless {
    pub ssid: Option<String>,
    pub frequency_mhz: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub signal_percent: Option<u8>,
}

const NM: &str = "org.freedesktop.NetworkManager";

async fn from_network_manager(iface: &str) -> zbus::Result<Wireless> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let device = nm.get_device_by_ip_iface(iface).await?;
    let wifi = WirelessProxy::builder(&conn)
        .destination(NM)
        .path(device)
        .build()
        .await?;
    let bitrate = wifi.bitrate().await?;
    let ap = wifi.active_access_point().await?;
    let mut w = Wireless {
        bitrate_kbps: (bitrate > 0).then_some(bitrate),
        ..Wireless::default()
    };
    if ap.as_str() != "/" {
        let point = AccessPointProxy::builder(&conn)
            .destination(NM)
            .path(ap)
            .build()
            .await?;
        let ssid = point.ssid().await?;
        w.ssid = Some(String::from_utf8_lossy(&ssid).into_owned()).filter(|s| !s.is_empty());
        w.frequency_mhz = point.frequency().await.ok();
        w.signal_percent = point.strength().await.ok();
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

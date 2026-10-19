//! Physical network adapters that are connected.

use std::collections::HashMap;

use serde::Serialize;

use crate::util::{History, Root, rate};
use crate::wireless::{self, Wireless};

/// D-Bus round trips: the Wi-Fi details are refreshed every few samples.
const WIRELESS_EVERY: u32 = 5;

#[derive(Clone, Debug, Default, Serialize)]
pub struct NetSnapshot {
    pub name: String,
    pub kind: String,
    pub adapter: String,
    pub link_mbps: Option<u64>,
    pub mac: Option<String>,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub wireless: Option<Wireless>,
    pub rx_bps: f64,
    pub tx_bps: f64,
    /// Bytes since the interface came up.
    pub rx_total: u64,
    pub tx_total: u64,
    pub rx_history: History,
    pub tx_history: History,
}

/// Model from udev's hardware database, else the kernel driver (systems
/// without udev, such as Alpine).
fn adapter_name(root: &Root, iface: &str) -> Option<String> {
    let idx = root.read_trimmed(format!("/sys/class/net/{iface}/ifindex"))?;
    root.udev_property(&format!("n{idx}"), "ID_MODEL_FROM_DATABASE")
        .or_else(|| root.link_name(format!("/sys/class/net/{iface}/device/driver")))
}

fn addresses() -> HashMap<String, (Vec<String>, Vec<String>)> {
    let mut out: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    let Ok(list) = nix::ifaddrs::getifaddrs() else {
        return out;
    };
    for ifa in list {
        let Some(addr) = ifa.address else { continue };
        let entry = out.entry(ifa.interface_name).or_default();
        if let Some(v4) = addr.as_sockaddr_in() {
            entry.0.push(v4.ip().to_string());
        } else if let Some(v6) = addr.as_sockaddr_in6() {
            entry.1.push(v6.ip().to_string());
        }
    }
    out
}

/// Bridges, veth and tunnels have no `device` link.
fn connected_adapters(root: &Root) -> Vec<String> {
    root.list("/sys/class/net")
        .into_iter()
        .filter(|n| {
            root.exists(format!("/sys/class/net/{n}/device"))
                && root
                    .read_trimmed(format!("/sys/class/net/{n}/operstate"))
                    .as_deref()
                    == Some("up")
        })
        .collect()
}

#[derive(Default)]
struct NetState {
    prev: Option<(u64, u64)>,
    wireless: Option<Wireless>,
    rx: History,
    tx: History,
}

#[derive(Default)]
pub struct NetSampler {
    state: HashMap<String, NetState>,
    samples: u32,
}

impl NetSampler {
    pub fn sample(&mut self, root: &Root, secs: f64) -> Vec<NetSnapshot> {
        let names = connected_adapters(root);
        self.state.retain(|k, _| names.contains(k));
        let mut addrs = addresses();
        let refresh = self.samples.is_multiple_of(WIRELESS_EVERY);
        self.samples += 1;
        names
            .into_iter()
            .map(|name| {
                let base = format!("/sys/class/net/{name}");
                let cur = (
                    root.read_u64(format!("{base}/statistics/rx_bytes"))
                        .unwrap_or(0),
                    root.read_u64(format!("{base}/statistics/tx_bytes"))
                        .unwrap_or(0),
                );
                let st = self.state.entry(name.clone()).or_default();
                let (rx_bps, tx_bps) = match st.prev {
                    Some(p) => (rate(p.0, cur.0, secs), rate(p.1, cur.1, secs)),
                    None => (0.0, 0.0),
                };
                if st.prev.is_some() {
                    st.rx.push(rx_bps);
                    st.tx.push(tx_bps);
                }
                st.prev = Some(cur);
                let (ipv4, ipv6) = addrs.remove(&name).unwrap_or_default();
                let wireless = root.exists(format!("{base}/wireless"));
                if wireless && (refresh || st.wireless.is_none()) {
                    st.wireless = Some(wireless::read(root, &name));
                }
                NetSnapshot {
                    wireless: st.wireless.clone(),
                    kind: if wireless { "Wi-Fi" } else { "Ethernet" }.to_owned(),
                    adapter: adapter_name(root, &name).unwrap_or_else(|| name.clone()),
                    link_mbps: root
                        .read_i64(format!("{base}/speed"))
                        .filter(|s| *s > 0)
                        .map(|s| s as u64),
                    mac: root.read_trimmed(format!("{base}/address")),
                    ipv4,
                    ipv6,
                    rx_bps,
                    tx_bps,
                    rx_total: cur.0,
                    tx_total: cur.1,
                    rx_history: st.rx.clone(),
                    tx_history: st.tx.clone(),
                    name,
                }
            })
            .collect()
    }
}

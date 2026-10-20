//! TCP and UDP sockets of the machine, from netlink sock_diag (/proc/net
//! as a fallback), Unix sockets from /proc/net/unix. The owning process
//! is found through /proc/<pid>/fd, readable for the user's own processes
//! only: other sockets are listed without one.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::Serialize;

use crate::Root;
use crate::process::Users;
use crate::sockdiag::{self, Socket};

#[derive(Clone, Debug, Serialize)]
pub struct Connection {
    /// "TCP" or "UDP".
    pub protocol: &'static str,
    pub ipv6: bool,
    pub local: String,
    pub local_port: u16,
    pub remote: String,
    pub remote_port: u16,
    /// established, listen, syn-sent, syn-recv, fin-wait, time-wait,
    /// close-wait, last-ack, closing, close; "" for unconnected UDP.
    pub state: &'static str,
    pub user: String,
    pub pid: Option<u32>,
    /// Who can reach it. Listening: "all" (every interface), "local" (this
    /// machine), "lan" or "internet" (one address); connected: where the
    /// other end is; "" for Unix sockets.
    pub scope: &'static str,
}

fn reach(ip: &IpAddr) -> &'static str {
    if ip.is_unspecified() {
        return "all";
    }
    if ip.is_loopback() {
        return "local";
    }
    if ip.is_multicast() {
        return "lan";
    }
    let private = match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
    };
    if private { "lan" } else { "internet" }
}

fn state(tcp: bool, code: u8) -> &'static str {
    match (tcp, code) {
        (_, 0x01) => "established",
        (true, 0x02) => "syn-sent",
        (true, 0x03) => "syn-recv",
        (true, 0x04 | 0x05) => "fin-wait",
        (true, 0x06) => "time-wait",
        (true, 0x07) => "close",
        (true, 0x08) => "close-wait",
        (true, 0x09) => "last-ack",
        (true, 0x0A) => "listen",
        (true, 0x0B) => "closing",
        _ => "",
    }
}

/// "0100007F:0035" (32-bit words in host order) to an address and port.
fn endpoint(s: &str) -> Option<(IpAddr, u16)> {
    let (addr, port) = s.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let mut bytes = Vec::with_capacity(16);
    for i in (0..addr.len()).step_by(8) {
        let word = u32::from_str_radix(addr.get(i..i + 8)?, 16).ok()?;
        bytes.extend_from_slice(&word.to_ne_bytes());
    }
    let ip = match bytes.len() {
        4 => IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
        16 => {
            let v6 = Ipv6Addr::from(<[u8; 16]>::try_from(bytes).ok()?);
            v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4)
        }
        _ => return None,
    };
    Some((ip, port))
}

/// The same sockets from /proc/net, when netlink sock_diag is unavailable.
fn proc_sockets(root: &Root) -> Vec<Socket> {
    let mut out = Vec::new();
    for (file, tcp, ipv6) in [
        ("tcp", true, false),
        ("tcp6", true, true),
        ("udp", false, false),
        ("udp6", false, true),
    ] {
        let text = root.read(format!("/proc/net/{file}")).unwrap_or_default();
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 10 {
                continue;
            }
            let (Some((local, local_port)), Some((remote, remote_port))) =
                (endpoint(f[1]), endpoint(f[2]))
            else {
                continue;
            };
            out.push(Socket {
                tcp,
                ipv6,
                local,
                local_port,
                remote,
                remote_port,
                state: u8::from_str_radix(f[3], 16).unwrap_or(0),
                uid: f[7].parse::<u32>().unwrap_or(u32::MAX),
                inode: f[9].parse::<u64>().unwrap_or(0),
            });
        }
    }
    out
}

pub fn list(root: &Root) -> Vec<Connection> {
    let owners = crate::netflow::inode_pids(root);
    let mut users = Users::default();
    let sockets = sockdiag::for_root(root).unwrap_or_else(|| proc_sockets(root));
    let mut out: Vec<Connection> = sockets
        .into_iter()
        .map(|s| {
            let state = state(s.tcp, s.state);
            let listening = state == "listen" || (!s.tcp && s.remote.is_unspecified());
            Connection {
                scope: reach(if listening { &s.local } else { &s.remote }),
                protocol: if s.tcp { "TCP" } else { "UDP" },
                ipv6: s.ipv6,
                local: s.local.to_string(),
                local_port: s.local_port,
                remote: if s.remote.is_unspecified() {
                    String::new()
                } else {
                    s.remote.to_string()
                },
                remote_port: s.remote_port,
                state,
                user: users.name(root, s.uid),
                pid: (s.inode != 0)
                    .then(|| owners.get(&s.inode).copied())
                    .flatten(),
            }
        })
        .collect();
    out.extend(unix_sockets(root, &owners));
    out
}

/// Named Unix sockets (a path or an abstract @name): the local services.
/// Anonymous ones are pairs between two processes, left out.
fn unix_sockets(root: &Root, owners: &std::collections::HashMap<u64, u32>) -> Vec<Connection> {
    let text = root.read("/proc/net/unix").unwrap_or_default();
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let path = f.get(7)?;
            let listening = u32::from_str_radix(f.get(3)?, 16).ok()? & 0x0001_0000 != 0;
            let connected = *f.get(5)? == "03";
            let inode = f.get(6)?.parse::<u64>().ok()?;
            Some(Connection {
                protocol: "Unix",
                ipv6: false,
                local: (*path).to_owned(),
                local_port: 0,
                remote: String::new(),
                remote_port: 0,
                state: if listening {
                    "listen"
                } else if connected {
                    "established"
                } else {
                    ""
                },
                user: String::new(),
                pid: owners.get(&inode).copied(),
                scope: "",
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_host_order_addresses() {
        assert_eq!(
            endpoint("0100007F:0035"),
            Some((IpAddr::V4(Ipv4Addr::LOCALHOST), 53))
        );
        assert_eq!(
            endpoint("00000000000000000000000001000000:01BB"),
            Some((IpAddr::V6(Ipv6Addr::LOCALHOST), 443))
        );
        // ::ffff:192.168.1.10, shown as IPv4.
        assert_eq!(
            endpoint("0000000000000000FFFF00000A01A8C0:1F90"),
            Some((IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)), 8080))
        );
    }

    #[test]
    fn reads_proc_net() {
        let dir = std::env::temp_dir().join(format!("taskmgr-conn-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("proc/net")).unwrap();
        std::fs::create_dir_all(dir.join("etc")).unwrap();
        std::fs::write(
            dir.join("etc/passwd"),
            "alice:x:1000:1000::/home/alice:/bin/sh\n",
        )
        .unwrap();
        let header = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";
        std::fs::write(
            dir.join("proc/net/tcp"),
            format!("{header}   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1\n   1: 0A01A8C0:C350 22D8B85D:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 0 1\n"),
        )
        .unwrap();
        let list = list(&Root::new(&dir));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[0].state, list[0].local_port, list[0].remote.as_str()),
            ("listen", 8080, "")
        );
        assert_eq!(list[1].remote, "93.184.216.34");
        assert_eq!((list[1].remote_port, list[1].user.as_str()), (443, "alice"));
        assert_eq!((list[0].scope, list[1].scope), ("local", "internet"));
    }
}

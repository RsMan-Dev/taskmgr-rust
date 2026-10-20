//! Per-process network traffic. Packet headers are read from an AF_PACKET
//! socket (root only), attributed to a local port, then to the process
//! owning a socket on that port. Used by the `taskmgr-helper` binary,
//! which the interface starts through pkexec.

use std::collections::HashMap;

use crate::util::Root;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Proto {
    Tcp,
    Udp,
}

/// `(protocol, local port)` of an IPv4 or IPv6 packet without link header.
pub fn local_port(packet: &[u8], outgoing: bool) -> Option<(Proto, u16)> {
    let (proto, l4) = match packet.first()? >> 4 {
        4 => {
            let ihl = usize::from(packet[0] & 0x0f) * 4;
            (*packet.get(9)?, ihl)
        }
        6 => (*packet.get(6)?, 40),
        _ => return None,
    };
    let proto = match proto {
        6 => Proto::Tcp,
        17 => Proto::Udp,
        _ => return None,
    };
    let port = |o: usize| {
        Some(u16::from_be_bytes([
            *packet.get(l4 + o)?,
            *packet.get(l4 + o + 1)?,
        ]))
    };
    // Sent packets carry the local port as source, received ones as destination.
    Some((proto, if outgoing { port(0)? } else { port(2)? }))
}

/// Socket inode per local port, from netlink sock_diag, else from
/// /proc/net/{tcp,udp}{,6}.
pub fn port_inodes(root: &Root) -> HashMap<(Proto, u16), u64> {
    let mut map = HashMap::new();
    if let Some(sockets) = crate::sockdiag::for_root(root) {
        for s in sockets.into_iter().filter(|s| s.inode != 0) {
            let proto = if s.tcp { Proto::Tcp } else { Proto::Udp };
            map.entry((proto, s.local_port)).or_insert(s.inode);
        }
        return map;
    }
    for (file, proto) in [
        ("tcp", Proto::Tcp),
        ("tcp6", Proto::Tcp),
        ("udp", Proto::Udp),
        ("udp6", Proto::Udp),
    ] {
        for line in root
            .read(format!("/proc/net/{file}"))
            .unwrap_or_default()
            .lines()
            .skip(1)
        {
            let f: Vec<&str> = line.split_whitespace().collect();
            let port = f
                .get(1)
                .and_then(|a| a.rsplit_once(':'))
                .and_then(|(_, p)| u16::from_str_radix(p, 16).ok());
            let inode = f
                .get(9)
                .and_then(|i| i.parse::<u64>().ok())
                .filter(|i| *i != 0);
            if let (Some(port), Some(inode)) = (port, inode) {
                map.entry((proto, port)).or_insert(inode);
            }
        }
    }
    map
}

/// Process owning each socket inode, by scanning every /proc/<pid>/fd.
pub fn inode_pids(root: &Root) -> HashMap<u64, u32> {
    let mut map = HashMap::new();
    for pid in root.list("/proc") {
        let Ok(n) = pid.parse::<u32>() else { continue };
        for fd in root.list(format!("/proc/{pid}/fd")) {
            let Ok(target) = std::fs::read_link(root.path(format!("/proc/{pid}/fd/{fd}"))) else {
                continue;
            };
            if let Some(inode) = target
                .to_str()
                .and_then(|t| t.strip_prefix("socket:["))
                .and_then(|t| t.strip_suffix(']'))
                .and_then(|t| t.parse::<u64>().ok())
            {
                map.entry(inode).or_insert(n);
            }
        }
    }
    map
}

/// Bytes per local port, split received / sent.
#[derive(Default)]
pub struct Counters(HashMap<(Proto, u16), (u64, u64)>);

impl Counters {
    pub fn add(&mut self, packet: &[u8], length: usize, outgoing: bool) {
        if let Some(key) = local_port(packet, outgoing) {
            let e = self.0.entry(key).or_default();
            if outgoing {
                e.1 += length as u64;
            } else {
                e.0 += length as u64;
            }
        }
    }

    /// Attributes the counted bytes to processes and resets the counters.
    pub fn drain_per_pid(&mut self, root: &Root) -> HashMap<u32, (u64, u64)> {
        let ports = port_inodes(root);
        let owners = inode_pids(root);
        let mut out: HashMap<u32, (u64, u64)> = HashMap::new();
        for (key, (rx, tx)) in self.0.drain() {
            if let Some(pid) = ports.get(&key).and_then(|i| owners.get(i)) {
                let e = out.entry(*pid).or_default();
                e.0 += rx;
                e.1 += tx;
            }
        }
        out
    }
}

pub type Rates = HashMap<u32, (f64, f64)>;

/// Latest line printed by the root helper.
#[derive(Clone, Debug, Default)]
pub struct HelperData {
    pub net: Rates,
    /// Bytes read and written so far, per pid: readable by root only.
    pub io: HashMap<u32, u64>,
    /// Per logical processor, from MSRs (see `msr`).
    pub vcore: Option<Vec<f64>>,
}

#[derive(serde::Deserialize)]
struct HelperLine {
    net: HashMap<String, (f64, f64)>,
    #[serde(default)]
    io: HashMap<String, u64>,
    vcore: Option<Vec<f64>>,
}

/// The helper sits next to the interface binary, or on the PATH.
pub fn helper_path() -> Option<String> {
    // Set by the AppImage, where the binary runs through the bundled loader.
    if let Ok(p) = std::env::var("TASKMGR_HELPER") {
        return Some(p);
    }
    let beside = std::env::current_exe()
        .ok()?
        .parent()?
        .join("taskmgr-helper");
    if beside.is_file() {
        return Some(beside.to_string_lossy().into_owned());
    }
    crate::util::find_program("taskmgr-helper", &[])
}

/// Starts the helper through pkexec and stores each line it prints in
/// `latest`; `on_start` runs once the first line arrives. Blocks until the
/// helper stops. The helper's stdin stays open for as long as this runs:
/// when the interface exits, the pipe closes and the helper quits.
pub fn run_helper(
    latest: &std::sync::Mutex<Option<HelperData>>,
    mut on_start: impl FnMut(),
) -> Result<(), String> {
    use std::io::{BufRead, Read};
    let helper = helper_path().ok_or("taskmgr-helper not found.")?;
    crate::admin::run_as_root(&[helper], |mut command| {
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let _stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("no output")?;
        let mut stderr = child.stderr.take().ok_or("no error output")?;
        let errors = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });
        let mut started = false;
        for line in std::io::BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(parsed) = serde_json::from_str::<HelperLine>(&line) else {
                continue;
            };
            let net: Rates = parsed
                .net
                .into_iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
                .collect();
            if let Ok(mut l) = latest.lock() {
                *l = Some(HelperData {
                    net,
                    io: parsed
                        .io
                        .into_iter()
                        .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
                        .collect(),
                    vcore: parsed.vcore,
                });
            }
            if !started {
                started = true;
                on_start();
            }
        }
        if let Ok(mut l) = latest.lock() {
            *l = None;
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        let stderr = errors.join().unwrap_or_default();
        Ok(match started {
            true => crate::admin::Attempt::Started,
            false => crate::admin::Attempt::Ended(status, stderr),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipv4(proto: u8, src: u16, dst: u16) -> Vec<u8> {
        let mut p = vec![
            0x45, 0, 0, 40, 0, 0, 0, 0, 64, proto, 0, 0, 10, 0, 0, 1, 10, 0, 0, 2,
        ];
        p.extend(src.to_be_bytes());
        p.extend(dst.to_be_bytes());
        p
    }

    #[test]
    fn ports_follow_direction() {
        let p = ipv4(6, 51000, 443);
        assert_eq!(local_port(&p, true), Some((Proto::Tcp, 51000)));
        assert_eq!(local_port(&p, false), Some((Proto::Tcp, 443)));
        assert_eq!(
            local_port(&ipv4(17, 27015, 3478), true),
            Some((Proto::Udp, 27015))
        );
        assert_eq!(local_port(&ipv4(1, 0, 0), true), None);
        let mut v6 = vec![0x60, 0, 0, 0, 0, 8, 17, 64];
        v6.extend([0u8; 32]);
        v6.extend(5353u16.to_be_bytes());
        v6.extend(5354u16.to_be_bytes());
        assert_eq!(local_port(&v6, false), Some((Proto::Udp, 5354)));
        assert_eq!(local_port(&[0x45, 0, 0], true), None);
    }

    #[test]
    fn counters_split_directions() {
        let mut c = Counters::default();
        c.add(&ipv4(6, 51000, 443), 1500, true);
        c.add(&ipv4(6, 443, 51000), 900, false);
        assert_eq!(c.0[&(Proto::Tcp, 51000)], (900, 1500));
    }
}

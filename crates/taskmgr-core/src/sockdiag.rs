//! TCP and UDP sockets of the machine through netlink sock_diag (what `ss`
//! uses): the kernel documents /proc/net/{tcp,udp}{,6} as obsolete, and the
//! netlink dump is cheaper and consistent per message.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::fd::{AsRawFd, OwnedFd};

use nix::sys::socket::{
    AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType, recv, sendto, socket,
};

use crate::util::Root;

const SOCK_DIAG_BY_FAMILY: u16 = 20;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLM_F_REQUEST: u16 = 0x1;
const NLM_F_DUMP: u16 = 0x300;
const HEADER: usize = 16;
const REQUEST: usize = 56;
const MESSAGE: usize = 72;
const IPPROTO_TCP: u8 = 6;
const IPPROTO_UDP: u8 = 17;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Socket {
    pub tcp: bool,
    pub ipv6: bool,
    /// IPv4-mapped IPv6 addresses are given as IPv4, as /proc/net shows them.
    pub local: IpAddr,
    pub local_port: u16,
    pub remote: IpAddr,
    pub remote_port: u16,
    /// Kernel TCP state code (TCP_ESTABLISHED = 1 ...); UDP uses 1 when
    /// connected and 7 (TCP_CLOSE) otherwise, as in /proc/net/udp.
    pub state: u8,
    pub uid: u32,
    pub inode: u64,
}

/// Every TCP and UDP socket, IPv4 and IPv6, of the current network namespace.
pub fn sockets() -> io::Result<Vec<Socket>> {
    let fd = socket(
        AddressFamily::Netlink,
        SockType::Datagram,
        SockFlag::SOCK_CLOEXEC,
        SockProtocol::NetlinkSockDiag,
    )?;
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    for (seq, (tcp, ipv6)) in [(true, false), (true, true), (false, false), (false, true)]
        .into_iter()
        .enumerate()
    {
        dump(&fd, seq as u32 + 1, tcp, ipv6, &mut buf, &mut out)?;
    }
    Ok(out)
}

/// The sockets for a live system root; None for a fixture tree or when
/// netlink is unavailable, so callers fall back to /proc/net.
pub fn for_root(root: &Root) -> Option<Vec<Socket>> {
    if !root.is_system() {
        return None;
    }
    sockets().ok()
}

fn dump(
    fd: &OwnedFd,
    seq: u32,
    tcp: bool,
    ipv6: bool,
    buf: &mut [u8],
    out: &mut Vec<Socket>,
) -> io::Result<()> {
    let req = request(seq, tcp, ipv6);
    sendto(
        fd.as_raw_fd(),
        &req,
        &NetlinkAddr::new(0, 0),
        MsgFlags::empty(),
    )?;
    loop {
        let n = recv(fd.as_raw_fd(), buf, MsgFlags::empty())?;
        if n == 0 || parse(&buf[..n], seq, tcp, out)? {
            return Ok(());
        }
    }
}

fn request(seq: u32, tcp: bool, ipv6: bool) -> Vec<u8> {
    let mut req = Vec::with_capacity(HEADER + REQUEST);
    req.extend_from_slice(&((HEADER + REQUEST) as u32).to_ne_bytes());
    req.extend_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
    req.extend_from_slice(&(NLM_F_REQUEST | NLM_F_DUMP).to_ne_bytes());
    req.extend_from_slice(&seq.to_ne_bytes());
    req.extend_from_slice(&0u32.to_ne_bytes());
    let family = if ipv6 { libc::AF_INET6 } else { libc::AF_INET } as u8;
    let protocol = if tcp { IPPROTO_TCP } else { IPPROTO_UDP };
    req.extend_from_slice(&[family, protocol, 0, 0]);
    req.extend_from_slice(&u32::MAX.to_ne_bytes());
    req.resize(HEADER + REQUEST, 0);
    req
}

/// One datagram of the reply; true once the dump is over.
fn parse(buf: &[u8], seq: u32, tcp: bool, out: &mut Vec<Socket>) -> io::Result<bool> {
    let mut at = 0;
    while at + HEADER <= buf.len() {
        let len = u32_at(buf, at) as usize;
        if len < HEADER || at + len > buf.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad netlink length",
            ));
        }
        let kind = u16::from_ne_bytes([buf[at + 4], buf[at + 5]]);
        let ours = u32_at(buf, at + 8) == seq;
        let body = &buf[at + HEADER..at + len];
        at += (len + 3) & !3;
        if !ours {
            continue;
        }
        match kind {
            NLMSG_DONE => return Ok(true),
            NLMSG_ERROR => {
                let errno = if body.len() >= 4 {
                    u32_at(body, 0) as i32
                } else {
                    0
                };
                if errno != 0 {
                    return Err(io::Error::from_raw_os_error(-errno));
                }
            }
            SOCK_DIAG_BY_FAMILY => out.extend(message(body, tcp)),
            _ => {}
        }
    }
    Ok(false)
}

fn message(b: &[u8], tcp: bool) -> Option<Socket> {
    if b.len() < MESSAGE {
        return None;
    }
    let ipv6 = i32::from(b[0]) == libc::AF_INET6;
    let addr = |o: usize| {
        if ipv6 {
            let v6 = Ipv6Addr::from(<[u8; 16]>::try_from(&b[o..o + 16]).ok()?);
            Some(v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4))
        } else {
            Some(IpAddr::V4(Ipv4Addr::new(
                b[o],
                b[o + 1],
                b[o + 2],
                b[o + 3],
            )))
        }
    };
    Some(Socket {
        tcp,
        ipv6,
        state: b[1],
        local_port: u16::from_be_bytes([b[4], b[5]]),
        remote_port: u16::from_be_bytes([b[6], b[7]]),
        local: addr(8)?,
        remote: addr(24)?,
        uid: u32_at(b, 64),
        inode: u64::from(u32_at(b, 68)),
    })
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_ne_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(len: usize, kind: u16, seq: u32) -> Vec<u8> {
        let mut h = (len as u32).to_ne_bytes().to_vec();
        h.extend_from_slice(&kind.to_ne_bytes());
        h.extend_from_slice(&2u16.to_ne_bytes());
        h.extend_from_slice(&seq.to_ne_bytes());
        h.extend_from_slice(&0u32.to_ne_bytes());
        h
    }

    fn diag(seq: u32, family: u8, state: u8, ports: (u16, u16), src: &[u8], dst: &[u8]) -> Vec<u8> {
        let mut m = header(HEADER + MESSAGE, SOCK_DIAG_BY_FAMILY, seq);
        let mut b = vec![0u8; MESSAGE];
        b[0] = family;
        b[1] = state;
        b[4..6].copy_from_slice(&ports.0.to_be_bytes());
        b[6..8].copy_from_slice(&ports.1.to_be_bytes());
        b[8..8 + src.len()].copy_from_slice(src);
        b[24..24 + dst.len()].copy_from_slice(dst);
        b[64..68].copy_from_slice(&1000u32.to_ne_bytes());
        b[68..72].copy_from_slice(&12345u32.to_ne_bytes());
        m.extend(b);
        m
    }

    fn done(seq: u32) -> Vec<u8> {
        let mut m = header(HEADER + 4, NLMSG_DONE, seq);
        m.extend_from_slice(&0i32.to_ne_bytes());
        m
    }

    #[test]
    fn request_layout() {
        let r = request(7, false, true);
        assert_eq!(r.len(), 72);
        assert_eq!(u32_at(&r, 0), 72);
        assert_eq!(u16::from_ne_bytes([r[4], r[5]]), SOCK_DIAG_BY_FAMILY);
        assert_eq!(u16::from_ne_bytes([r[6], r[7]]), 0x301);
        assert_eq!(u32_at(&r, 8), 7);
        assert_eq!((r[16], r[17]), (libc::AF_INET6 as u8, IPPROTO_UDP));
        assert_eq!(u32_at(&r, 20), u32::MAX);
    }

    #[test]
    fn parses_multipart_dump() {
        let v4 = libc::AF_INET as u8;
        let v6 = libc::AF_INET6 as u8;
        let mapped = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 192, 168, 1, 10];
        let mut first = diag(3, v4, 10, (8080, 0), &[127, 0, 0, 1], &[0; 4]);
        first.extend(diag(
            3,
            v6,
            1,
            (50000, 443),
            &mapped,
            &Ipv6Addr::LOCALHOST.octets(),
        ));
        // A reply to another request is ignored.
        first.extend(diag(9, v4, 1, (1, 2), &[1; 4], &[2; 4]));
        let mut out = Vec::new();
        assert!(!parse(&first, 3, true, &mut out).unwrap());
        assert!(parse(&done(3), 3, true, &mut out).unwrap());
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0],
            Socket {
                tcp: true,
                ipv6: false,
                local: IpAddr::V4(Ipv4Addr::LOCALHOST),
                local_port: 8080,
                remote: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                remote_port: 0,
                state: 10,
                uid: 1000,
                inode: 12345,
            }
        );
        assert!(out[1].ipv6);
        assert_eq!(out[1].local, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)));
        assert_eq!(out[1].remote, IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert_eq!((out[1].local_port, out[1].remote_port), (50000, 443));
    }

    #[test]
    fn reports_errors() {
        let mut err = header(HEADER + 20, NLMSG_ERROR, 1);
        err.extend_from_slice(&(-libc::ENOENT).to_ne_bytes());
        err.extend_from_slice(&[0; 16]);
        let e = parse(&err, 1, false, &mut Vec::new()).unwrap_err();
        assert_eq!(e.raw_os_error(), Some(libc::ENOENT));
        let mut short = done(1);
        short[0] = 200;
        assert!(parse(&short, 1, false, &mut Vec::new()).is_err());
    }
}

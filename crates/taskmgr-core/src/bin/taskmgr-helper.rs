//! Root helper started by the interface through pkexec. Prints one JSON line
//! per second, `{"net": {"<pid>": [received B/s, sent B/s]}, "vcore": [V]}`,
//! and exits when its standard input closes (the interface quit).

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use taskmgr_core::netflow::Counters;
use taskmgr_core::util::Root;

fn main() {
    // SAFETY: plain socket syscalls on a descriptor owned by this function.
    let fd = unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_DGRAM,
            (libc::ETH_P_ALL as u16).to_be() as i32,
        )
    };
    if fd < 0 {
        eprintln!("taskmgr-helper: {}", std::io::Error::last_os_error());
        std::process::exit(1);
    }
    let timeout = libc::timeval {
        tv_sec: 0,
        tv_usec: 200_000,
    };
    // SAFETY: valid fd and a correctly sized option value.
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            std::ptr::from_ref(&timeout).cast(),
            std::mem::size_of::<libc::timeval>() as u32,
        );
    }
    std::thread::spawn(|| {
        let mut sink = [0u8; 64];
        while std::io::stdin().read(&mut sink).is_ok_and(|n| n > 0) {}
        std::process::exit(0);
    });

    let root = Root::system();
    taskmgr_core::msr::ensure_module(&root);
    let loopback = std::fs::read_to_string("/sys/class/net/lo/ifindex")
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok());
    let mut counters = Counters::default();
    let mut buf = [0u8; 128];
    let mut last = Instant::now();
    loop {
        // SAFETY: zeroed sockaddr_ll is valid; lengths match the buffers.
        let mut addr: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
        let mut addr_len = std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t;
        // MSG_TRUNC returns the real packet size while copying only headers.
        let n = unsafe {
            libc::recvfrom(
                fd,
                buf.as_mut_ptr().cast(),
                buf.len(),
                libc::MSG_TRUNC,
                std::ptr::from_mut(&mut addr).cast(),
                &raw mut addr_len,
            )
        };
        if n > 0 && Some(addr.sll_ifindex) != loopback {
            let len = n as usize;
            counters.add(
                &buf[..len.min(buf.len())],
                len,
                addr.sll_pkttype == libc::PACKET_OUTGOING,
            );
        }
        let elapsed = last.elapsed();
        if elapsed >= Duration::from_secs(1) {
            last = Instant::now();
            let secs = elapsed.as_secs_f64();
            let net: serde_json::Map<String, serde_json::Value> = counters
                .drain_per_pid(&root)
                .into_iter()
                .map(|(pid, (rx, tx))| {
                    (
                        pid.to_string(),
                        serde_json::json!([rx as f64 / secs, tx as f64 / secs]),
                    )
                })
                .collect();
            let line =
                serde_json::json!({ "net": net, "vcore": taskmgr_core::msr::core_voltages(&root) });
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{line}").and_then(|_| out.flush()).is_err() {
                std::process::exit(0);
            }
        }
    }
}

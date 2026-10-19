//! Root helper started by the interface through pkexec. Prints one JSON line
//! per second, `{"net": {"<pid>": [received B/s, sent B/s]}, "vcore": [V]}`,
//! and exits when its standard input closes (the interface quit).

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use taskmgr_core::netflow::Counters;
use taskmgr_core::util::Root;

use nix::sys::socket::{
    AddressFamily, LinkAddr, MsgFlags, SockFlag, SockProtocol, SockType, recvmsg, setsockopt,
    socket, sockopt,
};
use nix::sys::time::TimeVal;

fn main() {
    let socket = match socket(
        AddressFamily::Packet,
        SockType::Datagram,
        SockFlag::empty(),
        SockProtocol::EthAll,
    ) {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("taskmgr-helper: {e}");
            std::process::exit(1);
        }
    };
    // Wakes up regularly to print even when no packet arrives.
    let _ = setsockopt(&socket, sockopt::ReceiveTimeout, &TimeVal::new(0, 200_000));
    std::thread::spawn(|| {
        let mut sink = [0u8; 64];
        while std::io::stdin().read(&mut sink).is_ok_and(|n| n > 0) {}
        std::process::exit(0);
    });

    let root = Root::system();
    taskmgr_core::msr::ensure_module(&root);
    let loopback = std::fs::read_to_string("/sys/class/net/lo/ifindex")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok());
    let mut counters = Counters::default();
    let mut buf = [0u8; 128];
    let mut last = Instant::now();
    loop {
        // MSG_TRUNC returns the real packet size while copying only headers.
        let mut iov = [std::io::IoSliceMut::new(&mut buf)];
        let received = recvmsg::<LinkAddr>(socket.as_raw_fd(), &mut iov, None, MsgFlags::MSG_TRUNC)
            .ok()
            .and_then(|m| Some((m.bytes, m.address?)));
        if let Some((len, addr)) = received
            && len > 0
            && Some(addr.ifindex()) != loopback
        {
            counters.add(
                &buf[..len.min(buf.len())],
                len,
                addr.pkttype() == libc::PACKET_OUTGOING,
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

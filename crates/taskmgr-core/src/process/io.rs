//! Disk activity. Per process, /proc/<pid>/io is shown only for the user's
//! own processes outside sandboxes, or to the root helper; for the others,
//! the cgroup's io.stat (systemd unit, app scope) is readable by anyone.

use std::collections::{HashMap, HashSet};

use super::procfs::io_bytes;
use crate::util::Root;

/// Bytes read and written so far by every process; run by the root helper.
pub fn io_totals(root: &Root) -> HashMap<u32, u64> {
    let mut text = String::new();
    root.list("/proc")
        .into_iter()
        .filter_map(|e| {
            let pid: u32 = e.parse().ok()?;
            root.read_into(format!("/proc/{pid}/io"), &mut text)
                .then(|| io_bytes(&text))
                .flatten()
                .map(|bytes| (pid, bytes))
        })
        .collect()
}

/// Bytes moved by the devices for one cgroup and its children.
fn stat_bytes(text: &str) -> u64 {
    text.split_whitespace()
        .filter_map(|f| {
            f.strip_prefix("rbytes=")
                .or_else(|| f.strip_prefix("wbytes="))
        })
        .filter_map(|v| v.parse::<u64>().ok())
        .sum()
}

#[derive(Default)]
pub(super) struct CgroupIo {
    prev: HashMap<String, u64>,
}

impl CgroupIo {
    /// Bytes per second of each cgroup in `cgroups` since the last call.
    pub fn rates(
        &mut self,
        root: &Root,
        cgroups: &HashSet<&str>,
        secs: f64,
    ) -> HashMap<String, f64> {
        let mut rates = HashMap::new();
        let mut next = HashMap::new();
        for cgroup in cgroups.iter().filter(|c| c.len() > 1) {
            let Some(total) = root
                .read(format!("/sys/fs/cgroup{cgroup}/io.stat"))
                .map(|t| stat_bytes(&t))
            else {
                continue;
            };
            if let Some(prev) = self.prev.get(*cgroup).filter(|_| secs > 0.0) {
                rates.insert(
                    (*cgroup).to_owned(),
                    total.saturating_sub(*prev) as f64 / secs,
                );
            }
            next.insert((*cgroup).to_owned(), total);
        }
        self.prev = next;
        rates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_reads_and_writes_of_every_device() {
        let text = "253:0 rbytes=4096 wbytes=8192 rios=1 wios=2 dbytes=0 dios=0\n\
                    259:0 rbytes=100 wbytes=0 rios=1 wios=0 dbytes=0 dios=0\n";
        assert_eq!(stat_bytes(text), 12388);
        assert_eq!(stat_bytes(""), 0);
    }
}

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::Serialize;

use crate::hwmon::{self, Chip};
use crate::util::{History, Root, rate};

#[derive(Clone, Debug, Default, Serialize)]
pub struct DiskSnapshot {
    pub index: usize,
    pub name: String,
    pub model: String,
    pub kind: String,
    pub capacity: u64,
    pub mounts: Vec<String>,
    pub system_disk: bool,
    pub active: f64,
    pub read_bps: f64,
    pub write_bps: f64,
    pub response_ms: f64,
    pub temperature: Option<f64>,
    pub active_history: History,
    pub read_history: History,
    pub write_history: History,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Counters {
    ios: u64,
    io_ms: u64,
    read_bytes: u64,
    write_bytes: u64,
    busy_ms: u64,
}

fn parse_diskstats(text: &str) -> HashMap<String, Counters> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let n = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let name = (*f.get(2)?).to_owned();
            Some((
                name,
                Counters {
                    ios: n(3) + n(7),
                    io_ms: n(6) + n(10),
                    read_bytes: n(5) * 512,
                    write_bytes: n(9) * 512,
                    busy_ms: n(12),
                },
            ))
        })
        .collect()
}

fn is_physical(root: &Root, name: &str) -> bool {
    const VIRTUAL: [&str; 5] = ["loop", "ram", "zram", "dm-", "md"];
    !VIRTUAL.iter().any(|p| name.starts_with(p))
        && root.exists(format!("/sys/block/{name}/device"))
        && root
            .read_u64(format!("/sys/block/{name}/size"))
            .unwrap_or(0)
            > 0
}

fn model(root: &Root, name: &str) -> String {
    let dev = root
        .read_trimmed(format!("/sys/block/{name}/dev"))
        .unwrap_or_default();
    let udev = format!("b{dev}");
    // sysfs truncates SATA models to 16 characters; udev has the full one.
    root.udev_property(&udev, "ID_MODEL_ENC")
        .or_else(|| root.udev_property(&udev, "ID_MODEL"))
        .or_else(|| root.read_trimmed(format!("/sys/block/{name}/device/model")))
        .unwrap_or_else(|| name.to_owned())
}

fn kind(root: &Root, name: &str) -> String {
    let path = root
        .canonical(format!("/sys/block/{name}"))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = if name.starts_with("nvme") {
        "SSD (NVMe)"
    } else if root.read_u64(format!("/sys/block/{name}/queue/rotational")) == Some(1) {
        "HDD"
    } else if name.starts_with("mmcblk") {
        "Carte SD"
    } else if name.starts_with("sr") {
        "Optique"
    } else {
        "SSD"
    };
    if path.contains("/usb") {
        format!("{base} (USB)")
    } else if root.read_u64(format!("/sys/block/{name}/removable")) == Some(1) {
        format!("{base} (amovible)")
    } else {
        base.to_owned()
    }
}

fn backing_disks(root: &Root, name: &str, out: &mut BTreeSet<String>) {
    let class = format!("/sys/class/block/{name}");
    if root.exists(format!("{class}/partition")) {
        if let Some(parent) = root
            .canonical(&class)
            .and_then(|p| Some(p.parent()?.file_name()?.to_string_lossy().into_owned()))
        {
            out.insert(parent);
        }
        return;
    }
    let slaves = root.list(format!("{class}/slaves"));
    if slaves.is_empty() {
        out.insert(name.to_owned());
    }
    for s in slaves {
        backing_disks(root, &s, out);
    }
}

/// Bind mounts nested under a mount of the same device are dropped.
fn mounts_by_disk(root: &Root) -> BTreeMap<String, Vec<String>> {
    let mounts = root.read("/proc/mounts").unwrap_or_default();
    let mut by_source: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in mounts.lines() {
        let mut f = line.split_whitespace();
        let (Some(src), Some(target)) = (f.next(), f.next()) else {
            continue;
        };
        if !src.starts_with("/dev/") {
            continue;
        }
        let target = target.replace("\\040", " ");
        let points = by_source.entry(src.to_owned()).or_default();
        if !points
            .iter()
            .any(|p| p == "/" || target.starts_with(&format!("{p}/")))
        {
            points.push(target);
        }
    }
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (src, points) in by_source {
        let dev = root
            .canonical(&src)
            .and_then(|p| Some(p.file_name()?.to_string_lossy().into_owned()))
            .unwrap_or_else(|| {
                Path::new(&src)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
        let mut disks = BTreeSet::new();
        backing_disks(root, &dev, &mut disks);
        for d in disks {
            out.entry(d).or_default().extend(points.iter().cloned());
        }
    }
    for points in out.values_mut() {
        points.sort();
        points.dedup();
    }
    out
}

#[derive(Default)]
struct DiskState {
    prev: Option<Counters>,
    active: History,
    read: History,
    write: History,
}

#[derive(Default)]
pub struct DiskSampler {
    state: HashMap<String, DiskState>,
}

impl DiskSampler {
    pub fn sample(&mut self, root: &Root, chips: &[Chip], secs: f64) -> Vec<DiskSnapshot> {
        let stats = parse_diskstats(&root.read("/proc/diskstats").unwrap_or_default());
        let mounts = mounts_by_disk(root);
        let names: Vec<String> = root
            .list("/sys/block")
            .into_iter()
            .filter(|n| is_physical(root, n))
            .collect();
        self.state.retain(|k, _| names.contains(k));
        names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let st = self.state.entry(name.clone()).or_default();
                let cur = stats.get(&name).copied().unwrap_or_default();
                let (active, read_bps, write_bps, response_ms) = match st.prev {
                    Some(p) => {
                        let ios = cur.ios.saturating_sub(p.ios);
                        (
                            (rate(p.busy_ms, cur.busy_ms, secs) / 10.0).min(100.0),
                            rate(p.read_bytes, cur.read_bytes, secs),
                            rate(p.write_bytes, cur.write_bytes, secs),
                            if ios > 0 {
                                cur.io_ms.saturating_sub(p.io_ms) as f64 / ios as f64
                            } else {
                                0.0
                            },
                        )
                    }
                    None => (0.0, 0.0, 0.0, 0.0),
                };
                if st.prev.is_some() {
                    st.active.push(active);
                    st.read.push(read_bps);
                    st.write.push(write_bps);
                }
                st.prev = Some(cur);
                let points = mounts.get(&name).cloned().unwrap_or_default();
                let temperature = root
                    .canonical(format!("/sys/block/{name}/device"))
                    .and_then(|d| hwmon::device_temperature(root, chips, &d, &["Composite"]));
                DiskSnapshot {
                    index,
                    model: model(root, &name),
                    kind: kind(root, &name),
                    capacity: root
                        .read_u64(format!("/sys/block/{name}/size"))
                        .unwrap_or(0)
                        * 512,
                    system_disk: points.iter().any(|p| p == "/"),
                    mounts: points,
                    active,
                    read_bps,
                    write_bps,
                    response_ms,
                    temperature,
                    active_history: st.active.clone(),
                    read_history: st.read.clone(),
                    write_history: st.write.clone(),
                    name,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diskstats_fields() {
        let s = parse_diskstats(" 259 0 nvme0n1 10 0 200 30 5 0 100 20 0 40 50 0 0 0 0\n");
        let c = s["nvme0n1"];
        assert_eq!(c.ios, 15);
        assert_eq!(c.io_ms, 50);
        assert_eq!(c.read_bytes, 200 * 512);
        assert_eq!(c.write_bytes, 100 * 512);
        assert_eq!(c.busy_ms, 40);
    }
}

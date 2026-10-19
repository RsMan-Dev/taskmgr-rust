//! Per-process use of the GPUs run by DRM drivers (amdgpu, i915, xe,
//! nouveau, panfrost...), from the `drm-*` keys of /proc/<pid>/fdinfo.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::GpuProcess;
use crate::util::Root;

/// Listing every descriptor of every process is the costly part: it is
/// redone for new processes and every few samples only.
const RESCAN_EVERY: u32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Busy {
    render: u64,
    encode: u64,
    decode: u64,
    /// xe counts GPU cycles against a total instead of nanoseconds.
    total: u64,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct Client {
    pdev: String,
    id: u64,
    busy: Busy,
    memory: Option<u64>,
}

#[derive(Default)]
pub struct DrmClients {
    fds: HashMap<u32, Vec<String>>,
    samples: u32,
    prev: HashMap<(u32, String, u64), Busy>,
    last: Option<Instant>,
}

fn kib(v: &str) -> Option<u64> {
    let mut it = v.split_whitespace();
    let n: u64 = it.next()?.parse().ok()?;
    Some(match it.next() {
        Some("KiB") => n * 1024,
        Some("MiB") => n * 1024 * 1024,
        Some("GiB") => n * 1024 * 1024 * 1024,
        _ => n,
    })
}

pub(super) fn parse(text: &str) -> Option<Client> {
    let mut c = Client::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim();
        let num = || v.split_whitespace().next()?.parse::<u64>().ok();
        if k == "drm-pdev" {
            c.pdev = v.to_owned();
        } else if k == "drm-client-id" {
            c.id = num()?;
        } else if let Some(class) = k
            .strip_prefix("drm-engine-")
            .or_else(|| k.strip_prefix("drm-cycles-"))
        {
            let n = num().unwrap_or(0);
            if class.contains("enc") {
                c.busy.encode = c.busy.encode.max(n);
            } else if class.contains("dec") || class.starts_with("video") || class == "vcs" {
                c.busy.decode = c.busy.decode.max(n);
            } else if !class.starts_with("capacity") && !class.contains("jpeg") {
                c.busy.render = c.busy.render.max(n);
            }
        } else if let Some(class) = k.strip_prefix("drm-total-cycles-") {
            if matches!(class, "rcs" | "ccs") {
                c.busy.total = num().unwrap_or(0);
            }
        } else if matches!(k, "drm-resident-vram" | "drm-memory-vram") {
            c.memory = kib(v);
        }
    }
    (!c.pdev.is_empty()).then_some(c)
}

fn percent(cur: u64, old: u64, span: f64) -> f64 {
    (cur.saturating_sub(old) as f64 / span * 100.0).clamp(0.0, 100.0)
}

impl DrmClients {
    fn drm_fds(root: &Root, pid: u32) -> Vec<String> {
        let dir = format!("/proc/{pid}/fd");
        root.list(&dir)
            .into_iter()
            .filter(|fd| {
                std::fs::read_link(root.path(format!("{dir}/{fd}")))
                    .is_ok_and(|t| t.starts_with("/dev/dri/"))
            })
            .collect()
    }

    /// Processes using each GPU, by PCI slot.
    pub fn sample(&mut self, root: &Root) -> HashMap<String, Vec<GpuProcess>> {
        let now = Instant::now();
        let span = self.last.map(|t| now.duration_since(t).as_secs_f64() * 1e9);
        self.last = Some(now);
        let rescan = self.samples.is_multiple_of(RESCAN_EVERY);
        self.samples += 1;
        let pids: HashSet<u32> = root
            .list("/proc")
            .iter()
            .filter_map(|p| p.parse().ok())
            .collect();
        self.fds.retain(|pid, _| pids.contains(pid));
        let mut out: HashMap<String, Vec<GpuProcess>> = HashMap::new();
        let mut seen = HashMap::new();
        for pid in pids {
            if rescan || !self.fds.contains_key(&pid) {
                self.fds.insert(pid, Self::drm_fds(root, pid));
            }
            let mut clients: Vec<Client> = Vec::new();
            for fd in self.fds.get(&pid).into_iter().flatten() {
                let Some(c) = root
                    .read(format!("/proc/{pid}/fdinfo/{fd}"))
                    .and_then(|t| parse(&t))
                else {
                    continue;
                };
                if !clients.iter().any(|o| o.pdev == c.pdev && o.id == c.id) {
                    clients.push(c);
                }
            }
            for c in clients {
                let key = (pid, c.pdev.clone(), c.id);
                let old = self.prev.get(&key).copied();
                seen.insert(key, c.busy);
                let (usage, encoder, decoder) = match (old, span) {
                    (Some(o), _) if c.busy.total > 0 => {
                        let total = c.busy.total.saturating_sub(o.total).max(1) as f64;
                        (percent(c.busy.render, o.render, total), None, None)
                    }
                    (Some(o), Some(span)) => (
                        percent(c.busy.render, o.render, span),
                        Some(percent(c.busy.encode, o.encode, span)),
                        Some(percent(c.busy.decode, o.decode, span)),
                    ),
                    _ => (0.0, None, None),
                };
                let list = out.entry(c.pdev).or_default();
                match list.iter_mut().find(|p| p.pid == pid) {
                    Some(p) => {
                        p.usage = (p.usage + usage).min(100.0);
                        p.memory = p.memory.max(c.memory);
                    }
                    None => list.push(GpuProcess {
                        pid,
                        usage,
                        memory: c.memory,
                        encoder,
                        decoder,
                    }),
                }
            }
        }
        self.prev = seen;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_amdgpu_and_xe_clients() {
        let amd = "drm-driver:\tamdgpu\ndrm-client-id:\t81\ndrm-pdev:\t0000:03:00.0\n\
                   drm-resident-vram:\t12 MiB\ndrm-engine-gfx:\t500 ns\ndrm-engine-enc:\t7 ns\n\
                   drm-engine-dec:\t9 ns\ndrm-engine-jpeg:\t3 ns\n";
        let c = parse(amd).unwrap();
        assert_eq!((c.pdev.as_str(), c.id), ("0000:03:00.0", 81));
        assert_eq!(
            c.busy,
            Busy {
                render: 500,
                encode: 7,
                decode: 9,
                total: 0
            }
        );
        assert_eq!(c.memory, Some(12 * 1024 * 1024));
        let xe = "drm-client-id: 4\ndrm-pdev: 0000:00:02.0\ndrm-cycles-rcs: 30\n\
                  drm-total-cycles-rcs: 100\ndrm-cycles-vcs: 5\n";
        assert_eq!(
            parse(xe).unwrap().busy,
            Busy {
                render: 30,
                encode: 0,
                decode: 5,
                total: 100
            }
        );
        assert_eq!(parse("pos: 0\nflags: 02\n"), None);
    }
}

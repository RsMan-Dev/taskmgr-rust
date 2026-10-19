use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Serialize, Serializer};

pub const HISTORY_LEN: usize = 60;

/// A sensor whose read takes longer than this (NVMe drives answer each
/// temperature read with a command, up to 10 ms) is only re-read every
/// `SLOW_REFRESH`.
const SLOW_READ: Duration = Duration::from_micros(500);
const SLOW_REFRESH: Duration = Duration::from_secs(5);

/// Last value and read time of each slow sensor file.
type SlowReads = HashMap<PathBuf, (Instant, Option<i64>)>;

/// Root of /proc and /sys: `/` in production, a fixture tree in tests.
/// Clones share the cache of slow sensors.
#[derive(Clone, Debug)]
pub struct Root {
    base: PathBuf,
    slow: Arc<Mutex<SlowReads>>,
}

impl Root {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            base: path.into(),
            slow: Arc::default(),
        }
    }

    pub fn system() -> Self {
        Self::new("/")
    }

    pub fn path(&self, abs: impl AsRef<Path>) -> PathBuf {
        let abs = abs.as_ref();
        self.base.join(abs.strip_prefix("/").unwrap_or(abs))
    }

    pub fn read(&self, abs: impl AsRef<Path>) -> Option<String> {
        fs::read_to_string(self.path(abs)).ok()
    }

    pub fn read_trimmed(&self, abs: impl AsRef<Path>) -> Option<String> {
        self.read(abs)
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }

    pub fn read_u64(&self, abs: impl AsRef<Path>) -> Option<u64> {
        self.read(abs)?.trim().parse().ok()
    }

    pub fn read_i64(&self, abs: impl AsRef<Path>) -> Option<i64> {
        self.read(abs)?.trim().parse().ok()
    }

    /// A hardware sensor value (hwmon), read again at once when cheap and
    /// at most every few seconds when the read is slow.
    pub fn read_sensor(&self, abs: impl AsRef<Path>) -> Option<i64> {
        let path = self.path(abs);
        let mut slow = self.slow.lock().ok()?;
        if let Some((at, value)) = slow.get(&path)
            && at.elapsed() < SLOW_REFRESH
        {
            return *value;
        }
        let started = Instant::now();
        let value = fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok());
        if started.elapsed() > SLOW_READ {
            slow.insert(path, (Instant::now(), value));
        } else {
            slow.remove(&path);
        }
        value
    }

    pub fn list(&self, abs: impl AsRef<Path>) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.path(abs))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    pub fn exists(&self, abs: impl AsRef<Path>) -> bool {
        self.path(abs).exists()
    }

    pub fn canonical(&self, abs: impl AsRef<Path>) -> Option<PathBuf> {
        let real = fs::canonicalize(self.path(abs)).ok()?;
        let base = fs::canonicalize(&self.base).ok()?;
        real.strip_prefix(&base)
            .ok()
            .map(|p| Path::new("/").join(p))
    }

    pub fn link_name(&self, abs: impl AsRef<Path>) -> Option<String> {
        let target = fs::read_link(self.path(abs)).ok()?;
        Some(target.file_name()?.to_string_lossy().into_owned())
    }

    pub fn udev_property(&self, id: &str, key: &str) -> Option<String> {
        let data = self.read(format!("/run/udev/data/{id}"))?;
        let prefix = format!("E:{key}=");
        data.lines()
            .find_map(|l| l.strip_prefix(&prefix))
            .map(unescape_udev)
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }
}

fn unescape_udev(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && bytes.get(i + 1) == Some(&b'x')
            && i + 3 < bytes.len()
            // Bytes, not &str: a multi-byte character may follow `\x`.
            && let Some(v) = std::str::from_utf8(&bytes[i + 2..i + 4]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 4;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Starts empty: the UI draws only what was measured.
#[derive(Clone, Debug, Default)]
pub struct History(VecDeque<f64>);

impl History {
    pub fn push(&mut self, v: f64) {
        if self.0.len() >= HISTORY_LEN {
            self.0.pop_front();
        }
        self.0.push_back(v);
    }

    pub fn last(&self) -> f64 {
        self.0.back().copied().unwrap_or(0.0)
    }

    pub fn max(&self) -> f64 {
        self.0.iter().copied().fold(0.0, f64::max)
    }
}

impl Serialize for History {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|v| (v * 100.0).round() / 100.0))
    }
}

/// Named series for the small graphs under a main graph.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Charts(std::collections::BTreeMap<&'static str, History>);

impl Charts {
    /// Missing values are skipped, so a series exists only once measured.
    pub fn push(&mut self, name: &'static str, value: Option<f64>) {
        if let Some(v) = value {
            self.0.entry(name).or_default().push(v);
        }
    }
}

pub fn rate(prev: u64, cur: u64, secs: f64) -> f64 {
    if secs <= 0.0 || cur < prev {
        0.0
    } else {
        (cur - prev) as f64 / secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udev_escapes_are_decoded() {
        assert_eq!(unescape_udev(r"Samsung\x20SSD\x20980"), "Samsung SSD 980");
        assert_eq!(unescape_udev("plain"), "plain");
        assert_eq!(unescape_udev(r"\xé"), r"\xé");
    }

    #[test]
    fn history_keeps_fixed_length() {
        let mut h = History::default();
        for i in 0..100 {
            h.push(i as f64);
        }
        assert_eq!(h.0.len(), HISTORY_LEN);
        assert_eq!(h.last(), 99.0);
        assert_eq!(h.0[0], 40.0);
    }
}

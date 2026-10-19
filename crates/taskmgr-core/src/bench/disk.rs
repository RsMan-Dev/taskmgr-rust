//! Disk throughput and random reads, on a temporary file in a chosen
//! folder. The file is dropped from the page cache after writing, so reads
//! measure the drive, not memory.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nix::fcntl::{PosixFadviseAdvice, posix_fadvise};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct DiskScore {
    pub folder: String,
    /// Megabytes per second.
    pub write: f64,
    pub read: f64,
    /// Random 4 KiB reads per second.
    pub random_iops: f64,
    pub size_bytes: u64,
}

const BLOCK: usize = 1 << 20;
const PAGE: u64 = 4096;

/// Removes the file even when the test stops early.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn forget_cache(file: &File) {
    let _ = posix_fadvise(file, 0, 0, PosixFadviseAdvice::POSIX_FADV_DONTNEED);
}

/// `size`: bytes written, capped to a tenth of the free space.
pub fn run(
    folder: &Path,
    size: u64,
    stop: &AtomicBool,
    progress: &dyn Fn(f64),
) -> Result<DiskScore, String> {
    let free = nix::sys::statvfs::statvfs(folder)
        .map(|s| s.blocks_available() * s.fragment_size())
        .map_err(|e| e.to_string())?;
    let size = size.min(free / 10) / BLOCK as u64 * BLOCK as u64;
    if size < 64 * BLOCK as u64 {
        return Err("Not enough free space in this folder.".to_owned());
    }
    let tmp = TempFile(folder.join(format!(".taskmgr-bench-{}", std::process::id())));
    let block = vec![0x5Au8; BLOCK];

    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .read(true)
        .open(&tmp.0)
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut written = 0u64;
    while written < size {
        if stop.load(Ordering::Relaxed) {
            return Err("Stopped.".to_owned());
        }
        file.write_all(&block).map_err(|e| e.to_string())?;
        written += BLOCK as u64;
        progress(written as f64 / size as f64 * 0.4);
    }
    file.sync_all().map_err(|e| e.to_string())?;
    let write = size as f64 / started.elapsed().as_secs_f64() / 1e6;
    forget_cache(&file);
    drop(file);

    let mut file = File::open(&tmp.0).map_err(|e| e.to_string())?;
    forget_cache(&file);
    let mut buf = vec![0u8; BLOCK];
    let started = Instant::now();
    let mut read_bytes = 0u64;
    while read_bytes < size {
        if stop.load(Ordering::Relaxed) {
            return Err("Stopped.".to_owned());
        }
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        read_bytes += n as u64;
        progress(0.4 + read_bytes as f64 / size as f64 * 0.3);
    }
    let read = read_bytes as f64 / started.elapsed().as_secs_f64() / 1e6;

    forget_cache(&file);
    let pages = size / PAGE;
    let mut page = vec![0u8; PAGE as usize];
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let span = Duration::from_secs(5);
    let started = Instant::now();
    let mut reads = 0u64;
    while started.elapsed() < span && !stop.load(Ordering::Relaxed) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        file.read_exact_at(&mut page, (x % pages) * PAGE)
            .map_err(|e| e.to_string())?;
        reads += 1;
        if reads.is_multiple_of(256) {
            progress(0.7 + started.elapsed().as_secs_f64() / span.as_secs_f64() * 0.3);
        }
    }
    let random_iops = reads as f64 / started.elapsed().as_secs_f64();
    progress(1.0);
    Ok(DiskScore {
        folder: folder.display().to_string(),
        write,
        read,
        random_iops,
        size_bytes: size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!("taskmgr-disk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let stop = AtomicBool::new(false);
        let r = run(&dir, 64 * BLOCK as u64, &stop, &|_| {});
        let left = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
        std::fs::remove_dir_all(&dir).ok();
        // A small temporary filesystem may refuse the size: then it says so.
        match r {
            Ok(s) => assert!(s.write > 0.0 && s.read > 0.0 && s.random_iops > 0.0),
            Err(e) => assert!(e.contains("free space"), "{e}"),
        }
        assert_eq!(left, 0);
    }
}

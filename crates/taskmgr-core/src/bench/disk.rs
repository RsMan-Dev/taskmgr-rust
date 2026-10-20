//! Disk throughput and random reads on a temporary file. Random data, which
//! btrfs or ZFS cannot compress; reads bypass the page cache (O_DIRECT)
//! where the filesystem allows it, else go through the dropped cache.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nix::fcntl::{FcntlArg, OFlag, PosixFadviseAdvice, fcntl, posix_fadvise};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct DiskScore {
    pub folder: String,
    /// Megabytes per second.
    pub write: f64,
    pub read: f64,
    /// Random 4 KiB reads per second.
    pub random_iops: f64,
    pub random_iops_parallel: f64,
    pub size_bytes: u64,
    /// Reads bypassed the page cache.
    pub direct: bool,
}

const BLOCK: usize = 1 << 20;
const PAGE: usize = 4096;
const PARALLEL: usize = 8;
const RANDOM_SPAN: Duration = Duration::from_secs(3);

/// A buffer aligned on a page, as O_DIRECT requires.
struct Aligned {
    bytes: Vec<u8>,
    start: usize,
    len: usize,
}

impl Aligned {
    fn new(len: usize) -> Self {
        let bytes = vec![0u8; len + PAGE];
        let start = bytes.as_ptr().align_offset(PAGE).min(PAGE);
        Self { bytes, start, len }
    }

    fn get(&mut self) -> &mut [u8] {
        &mut self.bytes[self.start..self.start + self.len]
    }
}

fn next(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

fn fill_random(block: &mut [u8], x: &mut u64) {
    for word in block.as_chunks_mut::<8>().0 {
        *word = next(x).to_le_bytes();
    }
}

/// O_DIRECT from now on if the filesystem allows it, else a dropped cache.
fn bypass_cache(file: &File) -> bool {
    let direct = fcntl(file, FcntlArg::F_GETFL)
        .map(|f| OFlag::from_bits_retain(f) | OFlag::O_DIRECT)
        .and_then(|f| fcntl(file, FcntlArg::F_SETFL(f)))
        .is_ok()
        && file.read_at(Aligned::new(PAGE).get(), 0).is_ok();
    if !direct {
        let _ = fcntl(file, FcntlArg::F_GETFL)
            .map(|f| OFlag::from_bits_retain(f) - OFlag::O_DIRECT)
            .and_then(|f| fcntl(file, FcntlArg::F_SETFL(f)));
        let _ = posix_fadvise(file, 0, 0, PosixFadviseAdvice::POSIX_FADV_DONTNEED);
    }
    direct
}

fn random_reads(file: &File, size: u64, threads: usize, stop: &AtomicBool) -> f64 {
    let pages = size / PAGE as u64;
    let done = AtomicU64::new(0);
    let started = Instant::now();
    std::thread::scope(|s| {
        for t in 0..threads {
            let done = &done;
            s.spawn(move || {
                let mut page = Aligned::new(PAGE);
                let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ (t as u64 + 1);
                let mut reads = 0;
                while started.elapsed() < RANDOM_SPAN && !stop.load(Ordering::Relaxed) {
                    let at = next(&mut x) % pages * PAGE as u64;
                    if file.read_exact_at(page.get(), at).is_err() {
                        break;
                    }
                    reads += 1;
                }
                done.fetch_add(reads, Ordering::Relaxed);
            });
        }
    });
    done.load(Ordering::Relaxed) as f64 / started.elapsed().as_secs_f64()
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
    // Unlinked at once: the file goes away with the descriptor, even if the
    // app is killed during the test.
    let path = folder.join(format!(".taskmgr-bench-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    // Fresh random data for every block, made outside the timed part.
    let mut block = vec![0u8; BLOCK];
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    let mut busy = Duration::ZERO;
    let mut written = 0u64;
    while written < size {
        if stop.load(Ordering::Relaxed) {
            return Err("Stopped.".to_owned());
        }
        fill_random(&mut block, &mut x);
        let started = Instant::now();
        file.write_all(&block).map_err(|e| e.to_string())?;
        busy += started.elapsed();
        written += BLOCK as u64;
        progress(written as f64 / size as f64 * 0.4);
    }
    let started = Instant::now();
    file.sync_all().map_err(|e| e.to_string())?;
    busy += started.elapsed();
    let write = size as f64 / busy.as_secs_f64() / 1e6;

    let direct = bypass_cache(&file);
    let mut buf = Aligned::new(BLOCK);
    let started = Instant::now();
    let mut read_bytes = 0u64;
    while read_bytes < size {
        if stop.load(Ordering::Relaxed) {
            return Err("Stopped.".to_owned());
        }
        let n = file
            .read_at(buf.get(), read_bytes)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        read_bytes += n as u64;
        progress(0.4 + read_bytes as f64 / size as f64 * 0.2);
    }
    let read = read_bytes as f64 / started.elapsed().as_secs_f64() / 1e6;

    let random_iops = random_reads(&file, size, 1, stop);
    progress(0.8);
    let random_iops_parallel = random_reads(&file, size, PARALLEL, stop);
    progress(1.0);
    if stop.load(Ordering::Relaxed) {
        return Err("Stopped.".to_owned());
    }
    Ok(DiskScore {
        folder: folder.display().to_string(),
        write,
        read,
        random_iops,
        random_iops_parallel,
        size_bytes: size,
        direct,
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
            Ok(s) => assert!(
                s.write > 0.0
                    && s.read > 0.0
                    && s.random_iops > 0.0
                    && s.random_iops_parallel > 0.0
            ),
            Err(e) => assert!(e.contains("free space"), "{e}"),
        }
        assert_eq!(left, 0);
    }

    #[test]
    fn random_blocks_do_not_repeat() {
        let (mut a, mut b) = (vec![0u8; PAGE], vec![0u8; PAGE]);
        let mut x = 1;
        fill_random(&mut a, &mut x);
        fill_random(&mut b, &mut x);
        assert_ne!(a, b);
    }
}

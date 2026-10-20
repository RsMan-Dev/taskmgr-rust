//! The CPU score's workloads: integer and vector units, branches and caches,
//! as real programs use them. Deterministic, a fraction of a millisecond.

fn next(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

pub fn integer(seed: u64) -> u64 {
    let mut x = seed | 1;
    let mut acc = 0u64;
    for _ in 0..200_000 {
        acc = acc.wrapping_add(next(&mut x).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    }
    acc
}

/// In the first-level cache, with accumulators the compiler vectorizes.
pub fn float(seed: u64) -> u64 {
    let a: Vec<f64> = (0..1024).map(|i| ((seed + i) % 97) as f64 * 0.01).collect();
    let b: Vec<f64> = (0..1024)
        .map(|i| ((seed * 3 + i) % 89) as f64 * 0.02)
        .collect();
    let mut acc = [0f64; 8];
    for round in 0..1600 {
        let scale = 1.0 + round as f64 * 1e-6;
        for (ca, cb) in a.as_chunks::<8>().0.iter().zip(b.as_chunks::<8>().0) {
            for k in 0..8 {
                acc[k] += ca[k] * cb[k] * scale;
            }
        }
    }
    acc.iter().sum::<f64>().to_bits()
}

pub fn sort(seed: u64) -> u64 {
    let mut x = seed | 1;
    let mut v: Vec<u32> = (0..24_000).map(|_| next(&mut x) as u32).collect();
    v.sort_unstable();
    u64::from(v[v.len() / 2]) ^ u64::from(v[0])
}

const WORDS: [&str; 12] = [
    "the", "task", "manager", "process", "memory", "disk", "linux", "kernel", "thread", "score",
    "cache", "group",
];

/// Repeated sequences in text, as LZ compressors find them.
pub fn compress(seed: u64) -> u64 {
    let mut x = seed | 1;
    let mut text = Vec::with_capacity(66_000);
    while text.len() < 64_000 {
        text.extend_from_slice(WORDS[(next(&mut x) % 12) as usize].as_bytes());
        text.push(b' ');
    }
    let mut table = vec![0u32; 4096];
    let (mut i, mut out) = (0usize, 0u64);
    while i + 4 <= text.len() {
        let key = u32::from_le_bytes([text[i], text[i + 1], text[i + 2], text[i + 3]]);
        let slot = (key.wrapping_mul(2_654_435_761) >> 20) as usize;
        let candidate = table[slot] as usize;
        table[slot] = i as u32;
        if candidate < i && text[candidate..candidate + 4] == text[i..i + 4] {
            let mut len = 4;
            while i + len < text.len() && text[candidate + len] == text[i + len] {
                len += 1;
            }
            out = out
                .wrapping_mul(31)
                .wrapping_add((len + i - candidate) as u64);
            i += len;
        } else {
            out = out.wrapping_add(u64::from(text[i]));
            i += 1;
        }
    }
    out
}

pub type Workload = fn(u64) -> u64;

pub const ALL: [(&str, Workload); 4] = [
    ("integer", integer),
    ("float", float),
    ("sort", sort),
    ("compress", compress),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workloads_are_deterministic() {
        for (name, work) in ALL {
            assert_eq!(work(5), work(5), "{name}");
            assert_ne!(work(5), work(6), "{name}");
        }
    }
}

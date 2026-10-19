//! Busy and kernel time from /proc/stat.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Times {
    busy: u64,
    /// System, interrupt and softirq time.
    kernel: u64,
    total: u64,
}

pub(super) fn parse_stat(stat: &str) -> Option<Times> {
    parse_line(stat.lines().find(|l| l.starts_with("cpu "))?)
}

/// Per logical processor, in `/proc/stat` order.
pub(super) fn parse_cores(stat: &str) -> Vec<Times> {
    stat.lines()
        .filter(|l| l.starts_with("cpu") && !l.starts_with("cpu "))
        .filter_map(parse_line)
        .collect()
}

/// Busy and kernel percent between two readings.
pub(super) fn busy_percent(prev: Times, cur: Times) -> Option<(f64, f64)> {
    let span = cur.total.checked_sub(prev.total).filter(|s| *s > 0)? as f64;
    Some((
        cur.busy.saturating_sub(prev.busy) as f64 / span * 100.0,
        cur.kernel.saturating_sub(prev.kernel) as f64 / span * 100.0,
    ))
}

fn parse_line(line: &str) -> Option<Times> {
    let v: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse().ok())
        .collect();
    // user nice system idle iowait irq softirq steal (guest is already in user)
    let total: u64 = v.iter().take(8).sum();
    let at = |i: usize| v.get(i).copied().unwrap_or(0);
    Some(Times {
        busy: total.saturating_sub(at(3) + at(4)),
        kernel: at(2) + at(5) + at(6),
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_excludes_idle_and_iowait() {
        let t = parse_stat("cpu  10 0 5 80 5 0 0 0 0 0\ncpu0 1 2 3\n").unwrap();
        assert_eq!(
            t,
            Times {
                busy: 15,
                kernel: 5,
                total: 100
            }
        );
    }
}

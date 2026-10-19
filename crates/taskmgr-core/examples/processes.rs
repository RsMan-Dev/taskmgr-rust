use std::{
    thread,
    time::{Duration, Instant},
};

use taskmgr_core::{ProcessSampler, Root};

fn main() {
    let root = Root::system();
    let w = taskmgr_core::windows::WindowWatcher::start();
    let mut s = ProcessSampler::new(&root);
    s.sample(&root, 0.0, None);
    let start = Instant::now();
    thread::sleep(Duration::from_secs(1));
    let windows = w.as_ref().and_then(|w| w.windows());
    for g in s.sample(&root, start.elapsed().as_secs_f64(), windows.as_ref()) {
        let via = if g.via.is_empty() {
            String::new()
        } else {
            format!("  via {}", g.via.join(", "))
        };
        println!(
            "[{:?}] {} ({}) {:.1}% {} Mo  icon={}{via}",
            g.kind,
            g.name,
            g.processes.len(),
            g.cpu,
            g.memory / 1_048_576,
            g.icon.as_deref().unwrap_or("-")
        );
        for p in &g.processes {
            println!(
                "    {:>7} {:<32} {:?} {:.1}% {} Mo",
                p.pid,
                p.name,
                p.role,
                p.cpu,
                p.memory / 1_048_576
            );
        }
    }
}

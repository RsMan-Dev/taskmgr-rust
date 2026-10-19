use std::{thread, time::Duration};

use taskmgr_core::{ProcessSampler, Root};

fn main() {
    let root = Root::system();
    let mut s = ProcessSampler::new(&root);
    s.sample(&root, 0.0);
    thread::sleep(Duration::from_secs(1));
    for g in s.sample(&root, 1.0) {
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

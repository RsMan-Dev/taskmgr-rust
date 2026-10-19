//! Time spent per sample by the samplers and JSON encoding.
use std::time::Instant;

fn main() {
    let root = taskmgr_core::Root::system();
    let mut sampler = taskmgr_core::Sampler::new(root.clone());
    sampler.per_process = std::env::args().nth(1).as_deref() != Some("system-only");
    let mut processes = taskmgr_core::ProcessSampler::new(&root);
    let (mut s_t, mut p_t, mut j_t, mut bytes) = (0.0, 0.0, 0.0, 0);
    let n = 10;
    for i in 0..=n {
        let t = Instant::now();
        let snap = sampler.sample();
        let t1 = Instant::now();
        let groups = processes.sample(&root, 1.0, None, &snap.gpus, None);
        let t2 = Instant::now();
        let json = serde_json::to_string(&groups).unwrap_or_default()
            + &serde_json::to_string(&snap).unwrap_or_default();
        let t3 = Instant::now();
        if i > 0 {
            s_t += (t1 - t).as_secs_f64();
            p_t += (t2 - t1).as_secs_f64();
            j_t += (t3 - t2).as_secs_f64();
            bytes = json.len();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let ms = |v: f64| v / n as f64 * 1000.0;
    println!(
        "system {:.2} ms, processes {:.2} ms, json {:.2} ms, {} bytes",
        ms(s_t),
        ms(p_t),
        ms(j_t),
        bytes
    );
}

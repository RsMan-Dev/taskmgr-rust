//! Prints two snapshots one second apart, as JSON.
use std::{thread, time::Duration};

fn main() {
    let mut s = taskmgr_core::Sampler::new(taskmgr_core::Root::system());
    s.sample();
    thread::sleep(Duration::from_secs(1));
    let mut snap = serde_json::to_value(s.sample()).unwrap();
    // Histories are noise for a quick look.
    strip(&mut snap);
    println!("{}", serde_json::to_string_pretty(&snap).unwrap());
    println!(
        "{}",
        serde_json::to_string_pretty(&taskmgr_core::system_info(&taskmgr_core::Root::system()))
            .unwrap()
    );
}

fn strip(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(m) => {
            m.retain(|k, _| !k.ends_with("history"));
            m.values_mut().for_each(strip);
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
        _ => {}
    }
}

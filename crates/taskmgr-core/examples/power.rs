fn main() {
    let root = taskmgr_core::Root::system();
    println!(
        "{}",
        serde_json::to_string_pretty(&taskmgr_core::power::read(&root)).unwrap()
    );
    let b = taskmgr_core::battery::BatterySampler::default().sample(&root);
    println!("{}", serde_json::to_string_pretty(&b).unwrap());
    if let Some(p) = std::env::args().nth(1) {
        println!("set {p}: {:?}", taskmgr_core::power::set_profile(&p));
    }
}

fn main() {
    let dev = std::env::args().nth(1).unwrap_or_else(|| "sda".into());
    println!(
        "{}",
        serde_json::to_string_pretty(&taskmgr_core::smart::details(&dev)).unwrap_or_default()
    );
}

fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&taskmgr_core::smart::read()).unwrap()
    );
}

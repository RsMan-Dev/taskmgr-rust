fn main() {
    let list = taskmgr_core::connections::list(&taskmgr_core::Root::system());
    println!("{}", serde_json::to_string_pretty(&list).unwrap());
}

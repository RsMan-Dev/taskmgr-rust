//! `services` lists; `services <start|stop|restart> <name> [--user]` acts.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [action, name, rest @ ..] = args.as_slice() {
        let user = rest.iter().any(|a| a == "--user");
        println!("{:?}", taskmgr_core::services::control(name, user, action));
        return;
    }
    let list = taskmgr_core::services::list();
    let user = list.iter().filter(|s| s.user).count();
    println!("{} services ({} user)", list.len(), user);
    for s in list.iter().filter(|s| s.active == "active").take(8) {
        println!(
            "{:<5} {:<40} {:<8} {:<9} {}",
            if s.user { "user" } else { "sys" },
            s.name,
            s.active,
            s.file_state,
            s.description
        );
    }
}

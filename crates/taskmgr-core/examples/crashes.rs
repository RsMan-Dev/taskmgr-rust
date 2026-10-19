fn main() {
    let root = taskmgr_core::Root::system();
    match taskmgr_core::crashes::list(&root, None) {
        Ok(list) => {
            for c in &list {
                println!(
                    "{} {:7} {:28} {:28} {:?} {:?} {:?} trace={}",
                    c.time_us / 1_000_000,
                    c.kind,
                    c.app,
                    c.process,
                    c.signal,
                    c.exit_status,
                    c.unit,
                    c.trace.lines().count()
                );
            }
        }
        Err(e) => println!("error: {e}"),
    }
}

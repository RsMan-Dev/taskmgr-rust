use std::{thread, time::Duration};

fn main() {
    let Some(w) = taskmgr_core::windows::WindowWatcher::start() else {
        eprintln!("KWin scripting unavailable");
        return;
    };
    thread::sleep(Duration::from_secs(2));
    for (pid, list) in w.windows().unwrap_or_default() {
        for win in list {
            println!(
                "{pid:>7} {:<24} {:<24} {}",
                win.desktop, win.class, win.caption
            );
        }
    }
}

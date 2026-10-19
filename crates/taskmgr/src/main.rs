mod monitor;

fn main() {
    cxx_qt::init_qml_module!("org.taskmgr");
    let args: Vec<String> = std::env::args().collect();
    let code = monitor::startup::taskmgr_run(&args);
    taskmgr_core::windows::unload();
    std::process::exit(code);
}

mod monitor;

fn main() {
    cxx_qt::init_qml_module!("org.taskmgr");
    let args: Vec<String> = std::env::args().collect();
    let accent = taskmgr_core::appearance::accent()
        .map(|(r, g, b)| format!("{r},{g},{b}"))
        .unwrap_or_default();
    let code = monitor::startup::taskmgr_run(&args, &accent);
    taskmgr_core::windows::unload();
    std::process::exit(code);
}

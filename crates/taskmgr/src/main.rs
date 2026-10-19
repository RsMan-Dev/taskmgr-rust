mod monitor;

use std::ffi::{CString, c_char, c_int};

unsafe extern "C" {
    fn taskmgr_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
}

fn main() {
    cxx_qt::init_qml_module!("org.taskmgr");

    let args: Vec<CString> = std::env::args()
        .filter_map(|a| CString::new(a).ok())
        .collect();
    let mut argv: Vec<*mut c_char> = args.iter().map(|a| a.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut());
    // SAFETY: argv points into `args`, which outlives the event loop.
    let code = unsafe { taskmgr_run(args.len() as c_int, argv.as_mut_ptr()) };
    taskmgr_core::windows::unload();
    std::process::exit(code);
}

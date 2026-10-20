//! Entry point of the C++ side: application object, theme and QML engine.
//! Kept beside the monitor bridge: cxx-qt takes one directory per module.

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("taskmgr/src/cpp/app.h");

        /// Runs the event loop; returns the exit code. `accent`: the
        /// desktop's accent colour as "r,g,b", or empty.
        fn taskmgr_run(args: &Vec<String>, accent: &str) -> i32;
    }
}

pub use ffi::taskmgr_run;

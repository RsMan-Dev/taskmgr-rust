//! Entry point of the C++ side: application object, theme and QML engine.
//! Kept beside the monitor bridge: cxx-qt takes one directory per module.

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("taskmgr/src/cpp/app.h");

        /// Runs the event loop; returns the exit code.
        fn taskmgr_run(args: &Vec<String>) -> i32;
    }
}

pub use ffi::taskmgr_run;

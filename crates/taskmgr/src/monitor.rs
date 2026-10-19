use std::pin::Pin;
use std::thread;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use taskmgr_core::{Root, Sampler};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, snapshot)]
        #[qproperty(QString, info)]
        type SystemMonitor = super::SystemMonitorRust;

        #[qinvokable]
        fn start(self: Pin<&mut SystemMonitor>);
    }

    impl cxx_qt::Threading for SystemMonitor {}
}

const INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct SystemMonitorRust {
    snapshot: QString,
    info: QString,
    started: bool,
}

impl qobject::SystemMonitor {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let info =
            serde_json::to_string(&taskmgr_core::system_info(&Root::system())).unwrap_or_default();
        self.as_mut().set_info(QString::from(&info));

        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let mut sampler = Sampler::new(Root::system());
            loop {
                let started = Instant::now();
                let json = serde_json::to_string(&sampler.sample()).unwrap_or_default();
                let queued = qt_thread.queue(move |o| o.set_snapshot(QString::from(&json)));
                if queued.is_err() {
                    break;
                }
                thread::sleep(INTERVAL.saturating_sub(started.elapsed()));
            }
        });
    }
}

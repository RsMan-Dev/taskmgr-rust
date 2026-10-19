use std::pin::Pin;
use std::thread;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use taskmgr_core::{ProcessSampler, Root, Sampler};

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
        #[qproperty(QString, processes)]
        type SystemMonitor = super::SystemMonitorRust;

        #[qinvokable]
        fn start(self: Pin<&mut SystemMonitor>);

        #[qinvokable]
        fn terminate(self: &SystemMonitor, pids: &QString, force: bool) -> QString;
    }

    impl cxx_qt::Threading for SystemMonitor {}
}

const INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct SystemMonitorRust {
    snapshot: QString,
    info: QString,
    processes: QString,
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
            let root = Root::system();
            let mut sampler = Sampler::new(root.clone());
            let mut processes = ProcessSampler::new(&root);
            let mut last = Instant::now();
            loop {
                let started = Instant::now();
                let json = serde_json::to_string(&sampler.sample()).unwrap_or_default();
                let groups = processes.sample(&root, started.duration_since(last).as_secs_f64());
                last = started;
                let procs = serde_json::to_string(&groups).unwrap_or_default();
                let queued = qt_thread.queue(move |mut o| {
                    o.as_mut().set_snapshot(QString::from(&json));
                    o.set_processes(QString::from(&procs));
                });
                if queued.is_err() {
                    break;
                }
                thread::sleep(INTERVAL.saturating_sub(started.elapsed()));
            }
        });
    }

    /// `pids` is comma-separated. Returns an error message, empty on success.
    pub fn terminate(&self, pids: &QString, force: bool) -> QString {
        let pids: Vec<i32> = pids
            .to_string()
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        QString::from(if taskmgr_core::process::terminate(&pids, force) {
            ""
        } else {
            "Accès refusé : ce processus appartient à un autre utilisateur ou au système."
        })
    }
}

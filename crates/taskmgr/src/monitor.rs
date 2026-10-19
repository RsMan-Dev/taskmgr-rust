use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use taskmgr_core::process::{PriorityError, Signal, Target};
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
        #[qproperty(QString, services)]
        #[qproperty(QString, autostart)]
        #[qproperty(QString, energy_request, cxx_name = "energyRequest")]
        #[qproperty(QString, action_result, cxx_name = "actionResult")]
        #[qproperty(QString, network_access, cxx_name = "networkAccess")]
        type SystemMonitor = super::SystemMonitorRust;

        #[qinvokable]
        fn start(self: Pin<&mut SystemMonitor>);

        /// 0 pauses sampling.
        #[qinvokable]
        #[cxx_name = "setInterval"]
        fn set_interval(self: &SystemMonitor, milliseconds: i32);

        #[qinvokable]
        #[cxx_name = "setProcessesVisible"]
        fn set_processes_visible(self: &SystemMonitor, visible: bool);

        /// Starts the root helper for per-process network traffic.
        #[qinvokable]
        #[cxx_name = "enableNetworkMonitor"]
        fn enable_network_monitor(self: Pin<&mut SystemMonitor>);

        /// Asks KWin to keep the window above others (Wayland).
        #[qinvokable]
        #[cxx_name = "setKeepAbove"]
        fn set_keep_above(self: &SystemMonitor, above: bool);

        #[qinvokable]
        #[cxx_name = "requestEnergyAccess"]
        fn request_energy_access(self: Pin<&mut SystemMonitor>);

        /// `signal`: term, kill, stop, cont. Returns "" or "denied".
        #[qinvokable]
        #[cxx_name = "sendSignal"]
        fn send_signal(self: &SystemMonitor, targets: &QString, signal: &QString) -> QString;

        /// Returns "", "denied" (root needed), "gone" or an error message.
        #[qinvokable]
        #[cxx_name = "setPriority"]
        fn set_priority(self: &SystemMonitor, target: &QString, nice: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "setPriorityAsAdmin"]
        fn set_priority_as_admin(self: Pin<&mut SystemMonitor>, pid: i32, nice: i32);

        #[qinvokable]
        #[cxx_name = "refreshServices"]
        fn refresh_services(self: Pin<&mut SystemMonitor>);

        #[qinvokable]
        #[cxx_name = "serviceAction"]
        fn service_action(
            self: Pin<&mut SystemMonitor>,
            name: &QString,
            user: bool,
            action: &QString,
        );

        #[qinvokable]
        #[cxx_name = "refreshAutostart"]
        fn refresh_autostart(self: Pin<&mut SystemMonitor>);

        #[qinvokable]
        #[cxx_name = "setAutostartEnabled"]
        fn set_autostart_enabled(
            self: Pin<&mut SystemMonitor>,
            id: &QString,
            enabled: bool,
        ) -> QString;
    }

    impl cxx_qt::Threading for SystemMonitor {}
}

pub struct SystemMonitorRust {
    snapshot: QString,
    info: QString,
    processes: QString,
    services: QString,
    autostart: QString,
    energy_request: QString,
    action_result: QString,
    network_access: QString,
    started: bool,
    net: Arc<std::sync::Mutex<Option<taskmgr_core::netflow::HelperData>>>,
    interval_ms: Arc<AtomicU64>,
    processes_visible: Arc<AtomicBool>,
}

impl Default for SystemMonitorRust {
    fn default() -> Self {
        Self {
            snapshot: QString::default(),
            info: QString::default(),
            processes: QString::default(),
            services: QString::default(),
            autostart: QString::default(),
            energy_request: QString::default(),
            action_result: QString::default(),
            network_access: QString::default(),
            started: false,
            net: Arc::default(),
            interval_ms: Arc::new(AtomicU64::new(1000)),
            processes_visible: Arc::new(AtomicBool::new(true)),
        }
    }
}

/// `{"action": ..., "ok": bool, "message": ...}` for actionResult.
fn result_json(action: &str, result: Result<(), String>) -> QString {
    let value = match result {
        Ok(()) => serde_json::json!({ "action": action, "ok": true, "message": "" }),
        Err(e) => serde_json::json!({ "action": action, "ok": false, "message": e }),
    };
    QString::from(&value.to_string())
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
        let interval = self.interval_ms.clone();
        let processes_visible = self.processes_visible.clone();
        let net = self.net.clone();
        thread::spawn(move || {
            let root = Root::system();
            let mut sampler = Sampler::new(root.clone());
            let mut processes = ProcessSampler::new(&root);
            let watcher = taskmgr_core::windows::WindowWatcher::start();
            let mut last = Instant::now();
            loop {
                let started = Instant::now();
                let helper = net.lock().ok().and_then(|n| n.clone());
                sampler.fallback_voltage = helper
                    .as_ref()
                    .and_then(|h| h.vcore.as_ref())
                    .filter(|v| !v.is_empty())
                    .map(|v| v.iter().sum::<f64>() / v.len() as f64);
                let snapshot = sampler.sample();
                let json = serde_json::to_string(&snapshot).unwrap_or_default();
                // The process list is the costly part: skipped while no
                // page shows it.
                let procs = processes_visible.load(Ordering::Relaxed).then(|| {
                    let windows = watcher.as_ref().and_then(|w| w.windows());
                    let rates = helper.map(|h| h.net);
                    let groups = processes.sample(
                        &root,
                        started.duration_since(last).as_secs_f64(),
                        windows.as_ref(),
                        &snapshot.gpus,
                        rates.as_ref(),
                    );
                    last = started;
                    serde_json::to_string(&groups).unwrap_or_default()
                });
                let queued = qt_thread.queue(move |mut o| {
                    o.as_mut().set_snapshot(QString::from(&json));
                    if let Some(p) = procs {
                        o.set_processes(QString::from(&p));
                    }
                });
                if queued.is_err() {
                    break;
                }
                loop {
                    let ms = interval.load(Ordering::Relaxed);
                    if ms > 0 && started.elapsed() >= Duration::from_millis(ms) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        });
    }

    pub fn set_keep_above(&self, above: bool) {
        thread::spawn(move || taskmgr_core::windows::keep_above(above));
    }

    pub fn set_interval(&self, milliseconds: i32) {
        self.interval_ms
            .store(milliseconds.max(0) as u64, Ordering::Relaxed);
    }

    pub fn set_processes_visible(&self, visible: bool) {
        self.processes_visible.store(visible, Ordering::Relaxed);
    }

    /// `networkAccess`: "pending", then "active"; back to "" when the helper
    /// stops, or the error message.
    pub fn enable_network_monitor(mut self: Pin<&mut Self>) {
        if matches!(
            self.network_access.to_string().as_str(),
            "pending" | "active"
        ) {
            return;
        }
        self.as_mut().set_network_access(QString::from("pending"));
        let qt_thread = self.qt_thread();
        let net = self.net.clone();
        thread::spawn(move || {
            let on_start = qt_thread.clone();
            let r = taskmgr_core::netflow::run_helper(&net, move || {
                let _ = on_start.queue(|o| o.set_network_access(QString::from("active")));
            });
            let state = r.err().unwrap_or_default();
            let _ = qt_thread.queue(move |o| o.set_network_access(QString::from(&state)));
        });
    }

    /// `energyRequest` becomes "pending", then "granted" or the error.
    pub fn request_energy_access(mut self: Pin<&mut Self>) {
        if self.energy_request.to_string() == "pending" {
            return;
        }
        self.as_mut().set_energy_request(QString::from("pending"));
        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let result = match taskmgr_core::energy::grant_access(&Root::system()) {
                Ok(()) => "granted".to_owned(),
                Err(e) => e,
            };
            let _ = qt_thread.queue(move |o| o.set_energy_request(QString::from(&result)));
        });
    }

    pub fn send_signal(&self, targets: &QString, signal: &QString) -> QString {
        let Some(signal) = Signal::parse(&signal.to_string()) else {
            return QString::from("invalid");
        };
        let targets = Target::parse_list(&targets.to_string());
        QString::from(if taskmgr_core::process::send(&targets, signal) {
            ""
        } else {
            "denied"
        })
    }

    pub fn set_priority(&self, target: &QString, nice: i32) -> QString {
        let Some(target) = Target::parse_list(&target.to_string()).into_iter().next() else {
            return QString::from("gone");
        };
        QString::from(&match taskmgr_core::process::set_priority(target, nice) {
            Ok(()) => String::new(),
            Err(PriorityError::Denied) => "denied".to_owned(),
            Err(PriorityError::Gone) => "gone".to_owned(),
            Err(PriorityError::Failed(e)) => e,
        })
    }

    /// pkexec waits for the password dialog: run it off the GUI thread.
    pub fn set_priority_as_admin(self: Pin<&mut Self>, pid: i32, nice: i32) {
        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let r = taskmgr_core::process::set_priority_as_admin(pid, nice);
            let _ = qt_thread.queue(move |o| o.set_action_result(result_json("priority", r)));
        });
    }

    pub fn refresh_services(self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let json = serde_json::to_string(&taskmgr_core::services::list()).unwrap_or_default();
            let _ = qt_thread.queue(move |o| o.set_services(QString::from(&json)));
        });
    }

    pub fn service_action(self: Pin<&mut Self>, name: &QString, user: bool, action: &QString) {
        let (name, action) = (name.to_string(), action.to_string());
        let qt_thread = self.qt_thread();
        thread::spawn(move || {
            let r = taskmgr_core::services::control(&name, user, &action);
            let json = serde_json::to_string(&taskmgr_core::services::list()).unwrap_or_default();
            let _ = qt_thread.queue(move |mut o| {
                o.as_mut().set_services(QString::from(&json));
                o.set_action_result(result_json("service", r));
            });
        });
    }

    pub fn refresh_autostart(self: Pin<&mut Self>) {
        let json = serde_json::to_string(&taskmgr_core::autostart::entries(&Root::system()))
            .unwrap_or_default();
        self.set_autostart(QString::from(&json));
    }

    pub fn set_autostart_enabled(mut self: Pin<&mut Self>, id: &QString, enabled: bool) -> QString {
        let r = taskmgr_core::autostart::set_enabled(&Root::system(), &id.to_string(), enabled);
        self.as_mut().refresh_autostart();
        QString::from(&r.err().map(|e| e.to_string()).unwrap_or_default())
    }
}

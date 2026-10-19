//! Bridge between the sampling core and QML. Results are JSON strings;
//! anything slow runs off the GUI thread.

mod actions;
mod queries;
mod sampling;
pub mod startup;

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::QString;
use serde::Serialize;

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
        #[qproperty(QString, health)]
        #[qproperty(QString, connections)]
        #[qproperty(QString, power)]
        #[qproperty(QString, crashes)]
        #[qproperty(QString, journal)]
        #[qproperty(QString, process_details, cxx_name = "processDetails")]
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

        #[qinvokable]
        #[cxx_name = "enableNetworkMonitor"]
        fn enable_network_monitor(self: Pin<&mut SystemMonitor>);

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

        /// `since_secs`: seconds since the epoch, 0 for everything kept.
        #[qinvokable]
        #[cxx_name = "refreshCrashes"]
        fn refresh_crashes(self: Pin<&mut SystemMonitor>, since_secs: i64);

        /// `tag` comes back with the result, so stale answers are ignored.
        #[qinvokable]
        #[cxx_name = "requestJournal"]
        fn request_journal(self: Pin<&mut SystemMonitor>, query: &QString, tag: &QString);

        #[qinvokable]
        #[cxx_name = "refreshPower"]
        fn refresh_power(self: Pin<&mut SystemMonitor>);

        #[qinvokable]
        #[cxx_name = "setPowerProfile"]
        fn set_power_profile(self: Pin<&mut SystemMonitor>, profile: &QString);

        #[qinvokable]
        #[cxx_name = "refreshConnections"]
        fn refresh_connections(self: Pin<&mut SystemMonitor>);

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

#[derive(Default)]
pub struct SystemMonitorRust {
    snapshot: QString,
    info: QString,
    processes: QString,
    services: QString,
    autostart: QString,
    energy_request: QString,
    action_result: QString,
    network_access: QString,
    health: QString,
    connections: QString,
    power: QString,
    crashes: QString,
    journal: QString,
    process_details: QString,
    started: bool,
    net: Arc<Mutex<Option<taskmgr_core::netflow::HelperData>>>,
    interval_ms: Arc<AtomicU64>,
    processes_visible: Arc<AtomicBool>,
}

type Monitor = qobject::SystemMonitor;

fn json(value: &impl Serialize) -> QString {
    QString::from(&serde_json::to_string(value).unwrap_or_default())
}

/// `{"action": ..., "ok": bool, "message": ...}` for actionResult.
fn result_json(action: &str, result: Result<(), String>) -> QString {
    let message = result.as_ref().err().cloned().unwrap_or_default();
    json(&serde_json::json!({ "action": action, "ok": result.is_ok(), "message": message }))
}

/// Runs `work` on its own thread and hands the result to `apply` on the
/// GUI thread.
fn background<T: Send + 'static>(
    qt: CxxQtThread<Monitor>,
    work: impl FnOnce() -> T + Send + 'static,
    apply: impl FnOnce(Pin<&mut Monitor>, T) + Send + 'static,
) {
    std::thread::spawn(move || {
        let value = work();
        let _ = qt.queue(move |o| apply(o, value));
    });
}

impl Monitor {
    fn spawn<T: Send + 'static>(
        self: &Pin<&mut Self>,
        work: impl FnOnce() -> T + Send + 'static,
        apply: impl FnOnce(Pin<&mut Monitor>, T) + Send + 'static,
    ) {
        background(self.qt_thread(), work, apply);
    }
}

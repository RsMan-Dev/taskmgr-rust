use std::pin::Pin;

use cxx_qt::Threading;
use cxx_qt_lib::QString;
use taskmgr_core::Root;
use taskmgr_core::process::{PriorityError, Signal, Target};

use super::{Monitor, json, result_json};

impl Monitor {
    /// Returns "" or "denied".
    pub fn send_signal(&self, targets: &QString, signal: &QString) -> QString {
        let Some(signal) = Signal::parse(&signal.to_string()) else {
            return QString::from("invalid");
        };
        let targets = Target::parse_list(&targets.to_string());
        let ok = taskmgr_core::process::send(&targets, signal);
        QString::from(if ok { "" } else { "denied" })
    }

    /// Returns "", "denied" (root needed), "gone" or an error message.
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

    pub fn set_priority_as_admin(self: Pin<&mut Self>, pid: i32, nice: i32) {
        self.spawn(
            move || taskmgr_core::process::set_priority_as_admin(pid, nice),
            |o, r| o.set_action_result(result_json("priority", r)),
        );
    }

    pub fn service_action(self: Pin<&mut Self>, name: &QString, user: bool, action: &QString) {
        let (name, action) = (name.to_string(), action.to_string());
        self.spawn(
            move || {
                let r = taskmgr_core::services::control(&name, user, &action);
                (r, json(&taskmgr_core::services::list()))
            },
            |mut o, (r, list)| {
                o.as_mut().set_services(list);
                o.set_action_result(result_json("service", r));
            },
        );
    }

    pub fn set_autostart_enabled(mut self: Pin<&mut Self>, id: &QString, enabled: bool) -> QString {
        let r = taskmgr_core::autostart::set_enabled(&Root::system(), &id.to_string(), enabled);
        self.as_mut().refresh_autostart();
        QString::from(&r.err().map(|e| e.to_string()).unwrap_or_default())
    }

    pub fn set_power_profile(self: Pin<&mut Self>, profile: &QString) {
        let profile = profile.to_string();
        self.spawn(
            move || {
                let r = taskmgr_core::power::set_profile(&profile);
                (r, json(&taskmgr_core::power::read(&Root::system())))
            },
            |mut o, (r, power)| {
                o.as_mut().set_power(power);
                o.set_action_result(result_json("power", r));
            },
        );
    }

    /// `energyRequest` becomes "pending", then "granted" or the error.
    pub fn request_energy_access(mut self: Pin<&mut Self>) {
        if self.energy_request.to_string() == "pending" {
            return;
        }
        self.as_mut().set_energy_request(QString::from("pending"));
        self.spawn(
            || match taskmgr_core::energy::grant_access(&Root::system()) {
                Ok(()) => "granted".to_owned(),
                Err(e) => e,
            },
            |o, r| o.set_energy_request(QString::from(&r)),
        );
    }

    /// `networkAccess`: "pending", then "active"; back to "" when the root
    /// helper stops, or the error message.
    pub fn enable_network_monitor(mut self: Pin<&mut Self>) {
        if matches!(
            self.network_access.to_string().as_str(),
            "pending" | "active"
        ) {
            return;
        }
        self.as_mut().set_network_access(QString::from("pending"));
        let on_start = self.qt_thread();
        let net = self.net.clone();
        self.spawn(
            move || {
                let r = taskmgr_core::netflow::run_helper(&net, move || {
                    let _ = on_start.queue(|o| o.set_network_access(QString::from("active")));
                });
                r.err().unwrap_or_default()
            },
            |o, state| o.set_network_access(QString::from(&state)),
        );
    }
}

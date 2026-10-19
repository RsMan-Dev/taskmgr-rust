use std::pin::Pin;

use cxx_qt_lib::QString;
use taskmgr_core::Root;

use super::{Monitor, json};

impl Monitor {
    pub fn refresh_services(self: Pin<&mut Self>) {
        self.spawn(
            || json(&taskmgr_core::services::list()),
            |o, v| o.set_services(v),
        );
    }

    pub fn refresh_autostart(self: Pin<&mut Self>) {
        let entries = json(&taskmgr_core::autostart::entries(&Root::system()));
        self.set_autostart(entries);
    }

    pub fn refresh_connections(self: Pin<&mut Self>) {
        self.spawn(
            || json(&taskmgr_core::connections::list(&Root::system())),
            |o, v| o.set_connections(v),
        );
    }

    pub fn refresh_power(self: Pin<&mut Self>) {
        self.spawn(
            || json(&taskmgr_core::power::read(&Root::system())),
            |o, v| o.set_power(v),
        );
    }

    /// `since_secs`: seconds since the epoch, 0 for everything kept.
    pub fn refresh_crashes(self: Pin<&mut Self>, since_secs: i64) {
        let since = u64::try_from(since_secs)
            .ok()
            .filter(|s| *s > 0)
            .map(|s| s * 1_000_000);
        self.spawn(
            move || match taskmgr_core::crashes::list(&Root::system(), since) {
                Ok(list) => json(&serde_json::json!({ "crashes": list, "error": "" })),
                Err(e) => json(&serde_json::json!({ "crashes": [], "error": e })),
            },
            |o, v| o.set_crashes(v),
        );
    }

    pub fn request_journal(self: Pin<&mut Self>, query: &QString, tag: &QString) {
        let (query, tag) = (query.to_string(), tag.to_string());
        self.spawn(
            move || {
                let result = serde_json::from_str::<taskmgr_core::journal::Query>(&query)
                    .map_err(|e| e.to_string())
                    .and_then(|q| taskmgr_core::journal::entries(&q));
                match result {
                    Ok(entries) => {
                        json(&serde_json::json!({ "tag": tag, "entries": entries, "error": "" }))
                    }
                    Err(e) => json(&serde_json::json!({ "tag": tag, "entries": [], "error": e })),
                }
            },
            |o, v| o.set_journal(v),
        );
    }
}

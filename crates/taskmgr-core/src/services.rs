//! systemd services of the system and of the user session, over D-Bus.
//! Actions on system units go through polkit, which asks for the password.

use std::collections::BTreeMap;

use serde::Serialize;
use zbus::Connection;

use crate::dbus::systemd_manager::ManagerProxy;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Service {
    pub name: String,
    pub description: String,
    /// active, inactive, failed, activating, deactivating, reloading.
    pub active: String,
    pub sub: String,
    /// enabled, disabled, static, masked, alias, ... ("" when generated).
    pub file_state: String,
    pub user: bool,
}

async fn manager(conn: &Connection) -> zbus::Result<ManagerProxy<'static>> {
    ManagerProxy::builder(conn)
        .destination("org.freedesktop.systemd1")
        .path("/org/freedesktop/systemd1")
        .build()
        .await
}

async fn list_bus(conn: &Connection, user: bool) -> zbus::Result<Vec<Service>> {
    let m = manager(conn).await?;
    let patterns = ["*.service"];
    let units = m.list_units_by_patterns(&[], &patterns).await?;
    let files = m.list_unit_files_by_patterns(&[], &patterns).await?;
    let mut by_name: BTreeMap<String, Service> = BTreeMap::new();
    for (path, state) in files {
        let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
        if name.contains("@.") || state == "alias" {
            continue;
        }
        let description = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("Description="))
                    .map(|d| d.trim().to_owned())
            })
            .unwrap_or_default();
        by_name.insert(
            name.clone(),
            Service {
                name,
                description,
                file_state: state,
                active: "inactive".into(),
                sub: "dead".into(),
                user,
            },
        );
    }
    for (name, description, _load, active, sub, ..) in units {
        let s = by_name.entry(name.clone()).or_insert_with(|| Service {
            name,
            user,
            ..Default::default()
        });
        if !description.is_empty() && description != s.name {
            s.description = description;
        }
        s.active = active;
        s.sub = sub;
    }
    // Desktop app instances and generated autostart units are shown by the
    // Processes and Startup pages, not as services.
    Ok(by_name
        .into_values()
        .filter(|s| {
            !(s.name.starts_with("app-") && (s.name.contains('@') || s.file_state == "transient"))
        })
        .collect())
}

/// System then user services. A bus that cannot be reached is skipped.
pub fn list() -> Vec<Service> {
    zbus::block_on(async {
        let mut out = Vec::new();
        if let Ok(conn) = Connection::system().await {
            out.extend(list_bus(&conn, false).await.unwrap_or_default());
        }
        if let Ok(conn) = Connection::session().await {
            out.extend(list_bus(&conn, true).await.unwrap_or_default());
        }
        out
    })
}

/// `action`: start, stop, restart, enable, disable. Blocks until done,
/// password dialog included.
pub fn control(name: &str, user: bool, action: &str) -> Result<(), String> {
    if !name.ends_with(".service") || name.contains('/') {
        return Err("Invalid unit name.".into());
    }
    zbus::block_on(async {
        let conn = if user {
            Connection::session().await
        } else {
            Connection::system().await
        }
        .map_err(|e| e.to_string())?;
        let m = manager(&conn).await.map_err(|e| e.to_string())?;
        let result = match action {
            "start" => m.start_unit(name, "replace").await.map(|_| ()),
            "stop" => m.stop_unit(name, "replace").await.map(|_| ()),
            "restart" => m.restart_unit(name, "replace").await.map(|_| ()),
            "enable" => m.enable_unit_files(&[name], false, false).await.map(|_| ()),
            "disable" => m.disable_unit_files(&[name], false).await.map(|_| ()),
            _ => return Err("Unknown action.".into()),
        };
        // zbus turns the standard D-Bus errors into `fdo::Error`, others
        // (systemd's own) stay method errors.
        let denied = "Authorization denied or cancelled.";
        result.map_err(|e| match e {
            zbus::Error::FDO(e)
                if matches!(
                    *e,
                    zbus::fdo::Error::AccessDenied(_)
                        | zbus::fdo::Error::InteractiveAuthorizationRequired(_)
                ) =>
            {
                denied.to_owned()
            }
            zbus::Error::MethodError(n, _, _)
                if n.as_str().ends_with("AccessDenied")
                    || n.as_str().contains("InteractiveAuthorization") =>
            {
                denied.to_owned()
            }
            other => other.to_string(),
        })?;
        if matches!(action, "enable" | "disable") {
            let _ = m.reload().await;
        }
        Ok(())
    })
}

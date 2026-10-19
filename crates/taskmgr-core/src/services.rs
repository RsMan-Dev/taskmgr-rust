//! systemd services of the system and of the user session, over D-Bus.
//! Actions on system units go through polkit, which asks for the password.

use std::collections::BTreeMap;

use serde::Serialize;
use zbus::Connection;
use zbus::proxy::MethodFlags;

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

type Unit = (
    String,
    String,
    String,
    String,
    String,
    String,
    zbus::OwnedObjectPath,
    u32,
    String,
    zbus::OwnedObjectPath,
);

async fn manager(conn: &Connection) -> zbus::Result<zbus::Proxy<'static>> {
    zbus::Proxy::new(
        conn,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )
    .await
}

async fn list_bus(conn: &Connection, user: bool) -> zbus::Result<Vec<Service>> {
    let m = manager(conn).await?;
    let patterns = vec!["*.service"];
    let none: Vec<&str> = Vec::new();
    let units: Vec<Unit> = m.call("ListUnitsByPatterns", &(&none, &patterns)).await?;
    let files: Vec<(String, String)> = m
        .call("ListUnitFilesByPatterns", &(&none, &patterns))
        .await?;
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
        let flags = MethodFlags::ALLOW_INTERACTIVE_AUTH;
        let result = match action {
            "start" | "stop" | "restart" => {
                let method = match action {
                    "start" => "StartUnit",
                    "stop" => "StopUnit",
                    _ => "RestartUnit",
                };
                m.call_with_flags::<_, _, zbus::OwnedObjectPath>(method, flags, &(name, "replace"))
                    .await
                    .map(|_| ())
            }
            "enable" => m
                .call_with_flags::<_, _, (bool, Vec<(String, String, String)>)>(
                    "EnableUnitFiles",
                    flags,
                    &(vec![name], false, false),
                )
                .await
                .map(|_| ()),
            "disable" => m
                .call_with_flags::<_, _, Vec<(String, String, String)>>(
                    "DisableUnitFiles",
                    flags,
                    &(vec![name], false),
                )
                .await
                .map(|_| ()),
            _ => return Err("Unknown action.".into()),
        };
        result.map_err(|e| match e {
            zbus::Error::MethodError(n, msg, _)
                if n.as_str().ends_with("AccessDenied")
                    || n.as_str().contains("InteractiveAuthorization") =>
            {
                msg.unwrap_or_else(|| "Authorization denied or cancelled.".into())
            }
            other => other.to_string(),
        })?;
        if matches!(action, "enable" | "disable") {
            let _ = m.call_with_flags::<_, _, ()>("Reload", flags, &()).await;
        }
        Ok(())
    })
}

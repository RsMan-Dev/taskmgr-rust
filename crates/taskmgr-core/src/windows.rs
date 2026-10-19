//! Window titles on Wayland: only the compositor knows them. A KWin script
//! pushes the window list to this process over D-Bus on every change.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use zbus::Connection;

/// One script per instance, so two windows of the app do not steal it from
/// each other.
fn plugin(pid: u32) -> String {
    format!("taskmgr-windows-{pid}")
}
// D-Bus interface names cannot contain a dash.
const INTERFACE: &str = "fr.rsman.taskmgr_rust.Windows";

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Window {
    pub pid: u32,
    pub caption: String,
    #[serde(rename = "cls")]
    pub class: String,
    pub desktop: String,
}

type Shared = Arc<Mutex<Option<Vec<Window>>>>;

struct Receiver(Shared);

#[zbus::interface(name = "fr.rsman.taskmgr_rust.Windows")]
impl Receiver {
    fn update(&self, json: String) {
        if let Ok(list) = serde_json::from_str::<Vec<Window>>(&json)
            && let Ok(mut w) = self.0.lock()
        {
            *w = Some(list);
        }
    }
}

fn script(service: &str) -> String {
    format!(
        r#"function send() {{
    const list = workspace.windowList()
        .filter(w => w.normalWindow && !w.skipTaskbar)
        .map(w => ({{ pid: w.pid, caption: w.caption, cls: w.resourceClass, desktop: w.desktopFileName }}));
    callDBus("{service}", "/windows", "{INTERFACE}", "Update", JSON.stringify(list));
}}
function watch(w) {{
    w.captionChanged.connect(send);
}}
workspace.windowList().forEach(watch);
workspace.windowAdded.connect(w => {{ watch(w); send(); }});
workspace.windowRemoved.connect(send);
send();
"#
    )
}

pub struct WindowWatcher {
    conn: Connection,
    shared: Shared,
    /// KWin reads the file when the script runs, so it lives as long as we do.
    path: String,
}

impl WindowWatcher {
    /// `None` outside KDE Plasma (no KWin scripting on the session bus).
    pub fn start() -> Option<Self> {
        zbus::block_on(Self::start_async())
    }

    async fn start_async() -> Option<Self> {
        let shared: Shared = Arc::default();
        let service = format!("fr.rsman.taskmgr_rust.instance{}", std::process::id());
        let conn = zbus::conn::Builder::session()
            .name(service.as_str())
            .serve_at("/windows", Receiver(shared.clone()))
            .build()
            .await
            .ok()?;
        let path = script_path();
        remove_stale_scripts(&conn, &path).await;
        std::fs::write(&path, script(&service)).ok()?;

        let scripting = |method: &'static str| {
            (
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                method,
            )
        };
        let (d, p, i, m) = scripting("unloadScript");
        let _ = conn
            .call_method(d, p, i, m, &(plugin(std::process::id()),))
            .await;
        let (d, p, i, m) = scripting("loadScript");
        let reply = conn
            .call_method(d, p, i, m, &(path.as_str(), plugin(std::process::id())))
            .await;
        let id: i32 = reply.ok()?.body().deserialize().ok()?;
        if id < 0 {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        conn.call_method(
            Some("org.kde.KWin"),
            format!("/Scripting/Script{id}").as_str(),
            Some("org.kde.kwin.Script"),
            "run",
            &(),
        )
        .await
        .ok()?;
        Some(Self { conn, shared, path })
    }

    /// Windows by pid; `None` until KWin has sent a first list.
    pub fn windows(&self) -> Option<HashMap<u32, Vec<Window>>> {
        let list = self.shared.lock().ok()?.clone()?;
        let mut map: HashMap<u32, Vec<Window>> = HashMap::new();
        for w in list {
            map.entry(w.pid).or_default().push(w);
        }
        Some(map)
    }
}

/// Unloads the script; also call it at exit, as the sampling thread that
/// owns the watcher is never joined.
pub fn unload() {
    let _ = zbus::block_on(async {
        let conn = Connection::session().await?;
        conn.call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "unloadScript",
            &(plugin(std::process::id()),),
        )
        .await
    });
    let _ = std::fs::remove_file(script_path());
}

/// Scripts and files left by instances that were killed.
async fn remove_stale_scripts(conn: &Connection, current: &str) {
    let Some(dir) = std::path::Path::new(current).parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let pid = name
            .strip_prefix("taskmgr-windows-")
            .and_then(|n| n.strip_suffix(".js"));
        if let Some(p) = pid.and_then(|p| p.parse::<u32>().ok())
            && !std::path::Path::new(&format!("/proc/{p}")).exists()
        {
            let _ = std::fs::remove_file(e.path());
            let _ = conn
                .call_method(
                    Some("org.kde.KWin"),
                    "/Scripting",
                    Some("org.kde.kwin.Scripting"),
                    "unloadScript",
                    &(plugin(p),),
                )
                .await;
        }
    }
}

fn script_path() -> String {
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    format!("{dir}/taskmgr-windows-{}.js", std::process::id())
}

impl Drop for WindowWatcher {
    fn drop(&mut self) {
        let _ = zbus::block_on(self.conn.call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "unloadScript",
            &(plugin(std::process::id()),),
        ));
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Keeps this process's windows above others (or not) through a one-shot
/// KWin script: Wayland gives clients no "always on top" request.
/// Returns false when KWin scripting is not available.
pub fn keep_above(above: bool) -> bool {
    let pid = std::process::id();
    let name = format!("taskmgr-keepabove-{pid}");
    let path = format!("{}.keepabove.js", script_path().trim_end_matches(".js"));
    let code = format!(
        "workspace.windowList().filter(w => w.pid == {pid}).forEach(w => w.keepAbove = {above});"
    );
    if std::fs::write(&path, code).is_err() {
        return false;
    }
    let scripting = (
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
    );
    let ok = zbus::block_on(async {
        let conn = Connection::session().await.ok()?;
        let (d, p, i) = scripting;
        let _ = conn
            .call_method(d, p, i, "unloadScript", &(name.as_str(),))
            .await;
        let reply = conn
            .call_method(d, p, i, "loadScript", &(path.as_str(), name.as_str()))
            .await
            .ok()?;
        let id: i32 = reply.body().deserialize().ok()?;
        let script = format!("/Scripting/Script{id}");
        conn.call_method(d, script.as_str(), Some("org.kde.kwin.Script"), "run", &())
            .await
            .ok()?;
        let _ = conn
            .call_method(d, p, i, "unloadScript", &(name.as_str(),))
            .await;
        Some(())
    })
    .is_some();
    let _ = std::fs::remove_file(&path);
    ok
}

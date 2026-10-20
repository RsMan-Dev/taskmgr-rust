//! Window titles on Wayland: only the compositor knows them. A KWin script
//! pushes the window list to this process over D-Bus on every change.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use zbus::Connection;

use crate::dbus::kwin_script::ScriptProxy;
use crate::dbus::kwin_scripting::ScriptingProxy;

/// One script per instance, so two windows of the app do not steal it from
/// each other.
fn plugin(pid: u32) -> String {
    format!("taskmgr-windows-{pid}")
}
// D-Bus interface names cannot contain a dash.
const INTERFACE: &str = "fr.rsman.taskmgr_rust.Windows";
const KWIN: &str = "org.kde.KWin";

async fn scripting(conn: &Connection) -> zbus::Result<ScriptingProxy<'static>> {
    ScriptingProxy::builder(conn)
        .destination(KWIN)
        .path("/Scripting")
        .build()
        .await
}

async fn run_script(conn: &Connection, id: i32) -> Option<()> {
    let script = ScriptProxy::builder(conn)
        .destination(KWIN)
        .path(format!("/Scripting/Script{id}"))
        .build()
        .await
        .ok()?;
    script.run().await.ok()
}

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

        let kwin = scripting(&conn).await.ok()?;
        let name = plugin(std::process::id());
        let _ = kwin.unload_script(&name).await;
        let id = kwin.load_script(&path, &name).await.ok()?;
        if id < 0 {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        run_script(&conn, id).await?;
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
        scripting(&conn)
            .await?
            .unload_script(&plugin(std::process::id()))
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
    let kwin = scripting(conn).await.ok();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let pid = name
            .strip_prefix("taskmgr-windows-")
            .and_then(|n| n.strip_suffix(".js"));
        if let Some(p) = pid.and_then(|p| p.parse::<u32>().ok())
            && !std::path::Path::new(&format!("/proc/{p}")).exists()
        {
            let _ = std::fs::remove_file(e.path());
            if let Some(kwin) = &kwin {
                let _ = kwin.unload_script(&plugin(p)).await;
            }
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
        let _ = zbus::block_on(async {
            scripting(&self.conn)
                .await?
                .unload_script(&plugin(std::process::id()))
                .await
        });
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
    let ok = zbus::block_on(async {
        let conn = Connection::session().await.ok()?;
        let kwin = scripting(&conn).await.ok()?;
        let _ = kwin.unload_script(&name).await;
        let id = kwin.load_script(&path, &name).await.ok()?;
        run_script(&conn, id).await?;
        let _ = kwin.unload_script(&name).await;
        Some(())
    })
    .is_some();
    let _ = std::fs::remove_file(&path);
    ok
}

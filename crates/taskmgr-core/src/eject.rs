//! Safe removal of a drive through UDisks2: its file systems are unmounted,
//! then it is powered off. A busy file system names the processes holding
//! it, from their working directory and open files.

use std::collections::HashMap;

use serde::Serialize;
use zbus::{Connection, OwnedObjectPath, OwnedVariant, Variant};

use crate::util::Root;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Blocker {
    pub pid: u32,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EjectResult {
    pub ok: bool,
    pub message: String,
    pub blockers: Vec<Blocker>,
}

type Properties = HashMap<String, OwnedVariant>;
type Objects = HashMap<OwnedObjectPath, HashMap<String, Properties>>;

const BUS: &str = "org.freedesktop.UDisks2";

fn prop<'a, T: TryFrom<&'a Variant<'a>>>(p: Option<&'a Properties>, key: &str) -> Option<T> {
    p?.get(key).and_then(|v| T::try_from(v).ok())
}

/// Processes whose working directory, root, program or an open file lies
/// under one of the mount points.
pub fn blockers(root: &Root, mounts: &[String]) -> Vec<Blocker> {
    let under = |t: &str| {
        mounts.iter().any(|m| {
            t == m
                || t.strip_prefix(m.as_str())
                    .is_some_and(|r| r.starts_with('/'))
        })
    };
    let mut out = Vec::new();
    for pid in root
        .list("/proc")
        .iter()
        .filter_map(|p| p.parse::<u32>().ok())
    {
        let dir = format!("/proc/{pid}");
        let links = ["cwd", "root", "exe"]
            .iter()
            .map(|l| format!("{dir}/{l}"))
            .chain(
                root.list(format!("{dir}/fd"))
                    .into_iter()
                    .map(|fd| format!("{dir}/fd/{fd}")),
            );
        let held = links
            .filter_map(|l| std::fs::read_link(root.path(l)).ok())
            .map(|t| t.to_string_lossy().into_owned())
            .find(|t| under(t));
        if let Some(path) = held {
            out.push(Blocker {
                pid,
                name: root.read_trimmed(format!("{dir}/comm")).unwrap_or_default(),
                path,
            });
        }
    }
    out
}

fn mount_points(fs: &Properties) -> Vec<String> {
    fs.get("MountPoints")
        .and_then(|v| Vec::<Vec<u8>>::try_from(v.try_clone().ok()?).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|m| String::from_utf8_lossy(m.split(|b| *b == 0).next().unwrap_or(&[])).into_owned())
        .collect()
}

async fn run(root: &Root, block: &str) -> zbus::Result<EjectResult> {
    let conn = Connection::system().await?;
    let manager = zbus::Proxy::new(
        &conn,
        BUS,
        "/org/freedesktop/UDisks2",
        "org.freedesktop.DBus.ObjectManager",
    )
    .await?;
    let objects: Objects = manager.call("GetManagedObjects", &()).await?;
    let block_iface = "org.freedesktop.UDisks2.Block";
    let own = format!("/org/freedesktop/UDisks2/block_devices/{block}");
    let drive: Option<OwnedObjectPath> = objects
        .iter()
        .find(|(p, _)| p.as_str() == own)
        .and_then(|(_, i)| prop::<zbus::ObjectPath>(i.get(block_iface), "Drive"))
        .map(OwnedObjectPath::from)
        .filter(|d| d.as_str() != "/");
    if !objects.keys().any(|p| p.as_str() == own) {
        return Ok(EjectResult {
            message: "UDisks2 does not know this drive.".to_owned(),
            ..EjectResult::default()
        });
    }
    let none: HashMap<&str, Variant> = HashMap::new();
    for (path, ifaces) in &objects {
        // The device itself, its partitions, or anything on the same drive.
        let on_drive = drive.as_ref().is_some_and(|d| {
            prop::<zbus::ObjectPath>(ifaces.get(block_iface), "Drive").is_some_and(|x| x == **d)
        });
        let partition =
            prop::<zbus::ObjectPath>(ifaces.get("org.freedesktop.UDisks2.Partition"), "Table")
                .is_some_and(|t| t.as_str() == own);
        let ours = on_drive || partition || path.as_str() == own;
        let Some(fs) = ifaces
            .get("org.freedesktop.UDisks2.Filesystem")
            .filter(|_| ours)
        else {
            continue;
        };
        let mounts = mount_points(fs);
        if mounts.is_empty() {
            continue;
        }
        let proxy = zbus::Proxy::new(
            &conn,
            BUS,
            path.as_str(),
            "org.freedesktop.UDisks2.Filesystem",
        )
        .await?;
        if let Err(e) = proxy.call::<_, _, ()>("Unmount", &(&none,)).await {
            return Ok(EjectResult {
                message: e.to_string(),
                blockers: blockers(root, &mounts),
                ..EjectResult::default()
            });
        }
    }
    let Some(drive) = drive else {
        return Ok(EjectResult {
            ok: true,
            message: "Unmounted; this drive cannot be powered off.".to_owned(),
            blockers: Vec::new(),
        });
    };
    let d = objects
        .get(&drive)
        .and_then(|i| i.get("org.freedesktop.UDisks2.Drive"));
    let proxy =
        zbus::Proxy::new(&conn, BUS, drive.as_str(), "org.freedesktop.UDisks2.Drive").await?;
    let method = if prop::<bool>(d, "CanPowerOff") == Some(true) {
        "PowerOff"
    } else if prop::<bool>(d, "Ejectable") == Some(true) {
        "Eject"
    } else {
        return Ok(EjectResult {
            ok: true,
            message: "Unmounted; this drive cannot be powered off.".to_owned(),
            blockers: Vec::new(),
        });
    };
    proxy.call::<_, _, ()>(method, &(&none,)).await?;
    Ok(EjectResult {
        ok: true,
        ..EjectResult::default()
    })
}

pub fn eject(root: &Root, block: &str) -> EjectResult {
    zbus::block_on(run(root, block)).unwrap_or_else(|e| EjectResult {
        message: e.to_string(),
        ..EjectResult::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_processes_inside_a_mount() {
        let base = std::env::temp_dir().join(format!("taskmgr-eject-{}", std::process::id()));
        let fd = base.join("proc/42/fd");
        std::fs::create_dir_all(&fd).unwrap();
        std::fs::create_dir_all(base.join("proc/43")).unwrap();
        std::fs::write(base.join("proc/42/comm"), "vlc\n").unwrap();
        std::os::unix::fs::symlink("/media/usb/film.mkv", fd.join("7")).unwrap();
        std::os::unix::fs::symlink("/media/usb-other", base.join("proc/43/cwd")).unwrap();
        let found = blockers(&Root::new(&base), &["/media/usb".to_owned()]);
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(
            found,
            vec![Blocker {
                pid: 42,
                name: "vlc".into(),
                path: "/media/usb/film.mkv".into()
            }]
        );
    }
}

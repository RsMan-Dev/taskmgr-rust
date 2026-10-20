//! Power settings: the system power profile (power-profiles-daemon, which
//! lets the active session switch it without a password) and the cpufreq
//! policy behind it.

use serde::Serialize;
use zbus::Connection;

use crate::Root;
use crate::dbus::power_profiles::PowerProfilesProxy;

#[derive(Clone, Debug, Default, Serialize)]
pub struct PowerSettings {
    /// power-saver, balanced or performance; None without the daemon.
    pub profile: Option<String>,
    pub profiles: Vec<String>,
    /// Why performance is held back (e.g. "lap-detected"), "" otherwise.
    pub degraded: String,
    pub governor: Option<String>,
    pub driver: Option<String>,
    /// Energy/performance preference of the CPU (intel_pstate, amd-pstate).
    pub epp: Option<String>,
}

const BUSES: [(&str, &str); 2] = [
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
    ),
    ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
];

/// The legacy name serves the same interface under its own name.
async fn daemon(conn: &Connection) -> Option<PowerProfilesProxy<'static>> {
    for (name, path) in BUSES {
        let proxy = PowerProfilesProxy::builder(conn)
            .destination(name)
            .path(path)
            .interface(name)
            .build()
            .await;
        if let Ok(p) = proxy
            && p.active_profile().await.is_ok()
        {
            return Some(p);
        }
    }
    None
}

pub fn read(root: &Root) -> PowerSettings {
    let cpu = |f: &str| root.read_trimmed(format!("/sys/devices/system/cpu/cpu0/cpufreq/{f}"));
    let mut s = PowerSettings {
        governor: cpu("scaling_governor"),
        driver: cpu("scaling_driver"),
        epp: cpu("energy_performance_preference"),
        ..PowerSettings::default()
    };
    let _ = zbus::block_on(async {
        let conn = Connection::system().await.ok()?;
        let p = daemon(&conn).await?;
        s.profile = p.active_profile().await.ok();
        s.degraded = p.performance_degraded().await.unwrap_or_default();
        let list = p.profiles().await.unwrap_or_default();
        s.profiles = list
            .iter()
            .filter_map(|m| String::try_from(m.get("Profile")?.try_clone().ok()?).ok())
            .collect();
        Some(())
    });
    s
}

pub fn set_profile(profile: &str) -> Result<(), String> {
    zbus::block_on(async {
        let conn = Connection::system().await.map_err(|e| e.to_string())?;
        let p = daemon(&conn)
            .await
            .ok_or("power-profiles-daemon is not running.")?;
        p.set_active_profile(profile).await.map_err(|e| match e {
            zbus::Error::FDO(e) if matches!(*e, zbus::fdo::Error::AccessDenied(_)) => {
                "Authorization denied.".to_owned()
            }
            e => e.to_string(),
        })
    })
}

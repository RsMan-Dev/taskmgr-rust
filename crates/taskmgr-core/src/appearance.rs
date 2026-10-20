//! The desktop's accent colour, from the XDG settings portal.

use std::time::Duration;

use crate::dbus::portal_settings::SettingsProxy;

/// Components outside 0..1 mean "no accent chosen".
fn to_rgb((r, g, b): (f64, f64, f64)) -> Option<(u8, u8, u8)> {
    let byte = |c: f64| (0.0..=1.0).contains(&c).then(|| (c * 255.0).round() as u8);
    Some((byte(r)?, byte(g)?, byte(b)?))
}

async fn read() -> Option<(u8, u8, u8)> {
    let conn = zbus::Connection::session().await.ok()?;
    let settings = SettingsProxy::builder(&conn)
        .destination("org.freedesktop.portal.Desktop")
        .path("/org/freedesktop/portal/desktop")
        .build()
        .await
        .ok()?;
    let value = settings
        .read_one("org.freedesktop.appearance", "accent-color")
        .await
        .ok()?;
    to_rgb(<(f64, f64, f64)>::try_from(value).ok()?)
}

/// None without a portal or an answer within half a second.
pub fn accent() -> Option<(u8, u8, u8)> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(zbus::block_on(read()));
    });
    rx.recv_timeout(Duration::from_millis(500)).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_portal_components() {
        assert_eq!(to_rgb((0.337255, 0.341176, 0.960784)), Some((86, 87, 245)));
        assert_eq!(to_rgb((-1.0, -1.0, -1.0)), None);
    }
}

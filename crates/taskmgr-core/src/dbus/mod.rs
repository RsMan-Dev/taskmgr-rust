//! Typed D-Bus clients, generated from the services' own interface XML in
//! `dbus/`: update the XML, then run `dbus/generate.sh`.

pub mod kwin_script;
// Argument names come verbatim from the introspected camelCase XML.
#[allow(non_snake_case)]
pub mod kwin_scripting;
pub mod network_manager;
pub mod nm_access_point;
pub mod nm_wireless;
pub mod portal_settings;
pub mod power_profiles;
pub mod systemd_manager;
pub mod udisks_ata;
pub mod udisks_drive;
pub mod udisks_filesystem;
pub mod udisks_nvme;

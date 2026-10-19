//! Where a process sits in the systemd cgroup tree.

use super::naming::basename;

pub(super) enum Location {
    AppUnit(String),
    DBus(String),
    UserUnit(String),
    Session(String),
    SystemUnit(String),
    Other(String),
}

pub(super) fn unescape_unit(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("\\x") {
        out.push_str(&rest[..i]);
        match rest
            .get(i + 2..i + 4)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            Some(b) => {
                out.push(b as char);
                rest = &rest[i + 4..];
            }
            None => {
                out.push_str("\\x");
                rest = &rest[i + 2..];
            }
        }
    }
    out.push_str(rest);
    out
}

pub(super) fn locate(cgroup: &str) -> Location {
    let parts: Vec<&str> = cgroup.split('/').filter(|p| !p.is_empty()).collect();
    let after = |slice: &str| {
        let i = parts.iter().position(|p| *p == slice)?;
        parts[i + 1..].iter().find(|p| !p.ends_with(".slice"))
    };
    if let Some(unit) = after("app.slice") {
        return if let Some(rest) = unit.strip_prefix("dbus-:") {
            let name = rest.split_once('-').map_or(rest, |(_, n)| n);
            Location::DBus(name.split_once('@').map_or(name, |(n, _)| n).to_owned())
        } else if unit.starts_with("app-") {
            Location::AppUnit((*unit).to_owned())
        } else {
            Location::UserUnit((*unit).to_owned())
        };
    }
    for slice in ["session.slice", "background.slice"] {
        if let Some(unit) = after(slice) {
            return Location::UserUnit((*unit).to_owned());
        }
    }
    if let Some(s) = parts
        .iter()
        .find(|p| p.starts_with("session-") && p.ends_with(".scope"))
    {
        return Location::Session((*s).to_owned());
    }
    if let Some(u) = parts
        .iter()
        .find(|p| p.starts_with("user@") && p.ends_with(".service"))
    {
        return Location::UserUnit((*u).to_owned());
    }
    if let Some(unit) = after("system.slice") {
        return Location::SystemUnit((*unit).to_owned());
    }
    match parts.as_slice() {
        [unit] => Location::SystemUnit((*unit).to_owned()),
        _ => Location::Other(cgroup.to_owned()),
    }
}

/// `app-[<launcher>-]<id>-<random>.scope` or `app-[<launcher>-]<id>@<random>.service`.
pub(super) fn app_unit_id(unit: &str) -> String {
    let body = unit.strip_prefix("app-").unwrap_or(unit);
    let id = if let Some(s) = body.strip_suffix(".service") {
        s.split_once('@').map_or(s, |(a, _)| a)
    } else {
        let s = body.strip_suffix(".scope").unwrap_or(body);
        match s.rsplit_once('-') {
            Some((head, tail))
                if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                head
            }
            _ => s,
        }
    };
    unescape_unit(id)
}

pub(super) fn appimage_name(path: &str) -> String {
    let file = basename(path);
    let stem = file
        .len()
        .checked_sub(9)
        .filter(|&i| file[i..].eq_ignore_ascii_case(".appimage"))
        .map_or(file, |i| &file[..i]);
    let stem = ["-x86_64", "-x86-64", "-amd64", "-aarch64"]
        .iter()
        .find_map(|s| stem.strip_suffix(s))
        .unwrap_or(stem);
    stem.to_owned()
}

pub(super) fn humanize(id: &str) -> String {
    let last = id.rsplit('.').next().unwrap_or(id);
    let mut c = last.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

pub(super) fn unit_name(unit: &str) -> String {
    let n = unit.rsplit_once('.').map_or(unit, |(n, _)| n);
    n.split_once('@').map_or(n, |(base, _)| base).to_owned()
}

/// `mnt-MASS_HDD.mount` is the mount point `/mnt/MASS_HDD`.
pub(super) fn mount_point(unit: &str) -> Option<String> {
    let path = unit.strip_suffix(".mount")?;
    Some(format!("/{}", unescape_unit(&path.replace('-', "/"))))
}

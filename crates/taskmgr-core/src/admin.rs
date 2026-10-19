//! Commands run as root through pkexec, which asks for the password with
//! the desktop's polkit agent.

use std::process::{Command, ExitStatus};

use crate::energy::find_program;

/// pkexec refuses to start, before any password prompt, when `$SHELL` is
/// not listed in /etc/shells: a shell from `nix develop`, a custom
/// launcher. The login shell, else /bin/sh, replaces it then.
fn usable_shell(current: Option<&str>, shells: &str, login: Option<&str>) -> Option<String> {
    let listed = |s: &str| shells.lines().any(|l| l.trim() == s);
    match current {
        Some(s) if listed(s) => None,
        _ => [login, Some("/bin/sh")]
            .into_iter()
            .flatten()
            .find(|s| listed(s))
            .map(str::to_owned),
    }
}

/// A sandbox that sets no_new_privs (appimage-run, bwrap, some containers)
/// makes setuid programs such as pkexec run without their privileges.
fn no_new_privs(status: &str) -> bool {
    status.lines().any(|l| {
        l.strip_prefix("NoNewPrivs:")
            .is_some_and(|v| v.trim() == "1")
    })
}

pub(crate) fn pkexec() -> Result<Command, String> {
    if no_new_privs(&std::fs::read_to_string("/proc/self/status").unwrap_or_default()) {
        return Err("Administrator rights are blocked by the sandbox this app runs in (no_new_privs, as under appimage-run).".to_owned());
    }
    let path = find_program("pkexec", &["/run/wrappers/bin/pkexec", "/usr/bin/pkexec"])
        .ok_or("pkexec not found (polkit package).")?;
    let mut command = Command::new(path);
    let shells = std::fs::read_to_string("/etc/shells").unwrap_or_default();
    let login = nix::unistd::User::from_uid(nix::unistd::getuid())
        .ok()
        .flatten()
        .map(|u| u.shell.to_string_lossy().into_owned());
    let current = std::env::var("SHELL").ok();
    if let Some(shell) = usable_shell(current.as_deref(), &shells, login.as_deref()) {
        command.env("SHELL", shell);
    }
    Ok(command)
}

/// 126: the dialog was refused or closed; 127: pkexec could not ask at all
/// (no polkit agent running, environment it rejects).
pub(crate) fn outcome(status: ExitStatus) -> Result<(), String> {
    match status.code() {
        Some(0) => Ok(()),
        Some(126) => Err("Authorization denied or cancelled.".to_owned()),
        Some(127) => Err("Administrator rights could not be requested: no polkit agent, or pkexec refused to start.".to_owned()),
        _ => Err(format!("The command failed ({status}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_a_sandbox_without_new_privileges() {
        assert!(no_new_privs("Name:\tx\nNoNewPrivs:\t1\nSeccomp:\t0\n"));
        assert!(!no_new_privs("NoNewPrivs:\t0\n"));
    }

    #[test]
    fn replaces_a_shell_pkexec_would_reject() {
        let shells = "/run/current-system/sw/bin/bash\n/bin/sh\n";
        let nix = Some("/nix/store/x-bash/bin/bash");
        let login = Some("/run/current-system/sw/bin/bash");
        assert_eq!(usable_shell(login, shells, login), None);
        assert_eq!(usable_shell(nix, shells, login).as_deref(), login);
        assert_eq!(
            usable_shell(nix, shells, Some("/usr/bin/fish")).as_deref(),
            Some("/bin/sh")
        );
        assert_eq!(usable_shell(None, shells, login).as_deref(), login);
    }
}

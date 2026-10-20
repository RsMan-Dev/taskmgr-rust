//! Commands run as root, the password being asked by the desktop's polkit
//! agent: through pkexec, else run0 (systemd 256+). run0 needs no setuid
//! bit, so it also works where no_new_privs disables pkexec.

use std::process::{Command, ExitStatus};

use crate::util::find_program;

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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tool {
    Pkexec,
    Run0,
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Done,
    Denied,
    /// The tool could not ask for the password: the next one may.
    NotAsked,
    Failed,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Pkexec => "pkexec",
            Tool::Run0 => "run0",
        }
    }

    fn verdict(self, code: Option<i32>, stderr: &str) -> Verdict {
        match (self, code) {
            (_, Some(0)) => Verdict::Done,
            (Tool::Pkexec, Some(126)) => Verdict::Denied,
            (Tool::Pkexec, Some(127)) => Verdict::NotAsked,
            // run0 reports its own failures before the command started.
            (Tool::Run0, _) if stderr.contains("Failed to start transient") => {
                if stderr.contains("Access denied") {
                    Verdict::Denied
                } else {
                    Verdict::NotAsked
                }
            }
            _ => Verdict::Failed,
        }
    }
}

fn tools() -> Result<Vec<(Tool, String)>, String> {
    let sandboxed = no_new_privs(&std::fs::read_to_string("/proc/self/status").unwrap_or_default());
    let pkexec = find_program("pkexec", &["/run/wrappers/bin/pkexec", "/usr/bin/pkexec"])
        .filter(|_| !sandboxed)
        .map(|p| (Tool::Pkexec, p));
    let run0 = find_program(
        "run0",
        &["/run/current-system/sw/bin/run0", "/usr/bin/run0"],
    )
    .map(|p| (Tool::Run0, p));
    let found: Vec<_> = pkexec.into_iter().chain(run0).collect();
    match found.is_empty() {
        false => Ok(found),
        true if sandboxed => Err("Administrator rights are blocked by the sandbox this app runs in (no_new_privs, as under appimage-run).".to_owned()),
        true => Err("Neither pkexec (polkit) nor run0 (systemd) was found.".to_owned()),
    }
}

fn command(tool: Tool, path: &str, argv: &[String]) -> Command {
    let mut command = Command::new(path);
    if tool == Tool::Pkexec {
        let shells = std::fs::read_to_string("/etc/shells").unwrap_or_default();
        let login = nix::unistd::User::from_uid(nix::unistd::getuid())
            .ok()
            .flatten()
            .map(|u| u.shell.to_string_lossy().into_owned());
        let current = std::env::var("SHELL").ok();
        if let Some(shell) = usable_shell(current.as_deref(), &shells, login.as_deref()) {
            command.env("SHELL", shell);
        }
    }
    command.args(argv);
    command
}

/// What running the command gave: it ended (exit status and error output),
/// or it is a long-running helper that did start.
pub(crate) enum Attempt {
    Ended(ExitStatus, String),
    Started,
}

/// Runs `argv` as root with the first tool able to ask for the password.
/// `run` starts the prepared command and waits for it.
pub(crate) fn run_as_root(
    argv: &[String],
    mut run: impl FnMut(Command) -> Result<Attempt, String>,
) -> Result<(), String> {
    let mut reasons = Vec::new();
    for (tool, path) in tools()? {
        let Attempt::Ended(status, stderr) = run(command(tool, &path, argv))? else {
            return Ok(());
        };
        match tool.verdict(status.code(), &stderr) {
            Verdict::Done => return Ok(()),
            Verdict::Denied => return Err("Authorization denied or cancelled.".to_owned()),
            Verdict::Failed => return Err(format!("The command failed ({status}).")),
            Verdict::NotAsked => {
                let last = stderr.lines().rfind(|l| !l.trim().is_empty());
                reasons.push(format!(
                    "{}: {}",
                    tool.name(),
                    last.unwrap_or("no message").trim()
                ));
            }
        }
    }
    Err(format!(
        "Administrator rights could not be requested ({}).",
        reasons.join("; ")
    ))
}

pub(crate) fn wait(mut command: Command) -> Result<Attempt, String> {
    let out = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    Ok(Attempt::Ended(
        out.status,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
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
    fn tells_refusal_from_failure_to_ask() {
        assert_eq!(Tool::Pkexec.verdict(Some(126), ""), Verdict::Denied);
        assert_eq!(Tool::Pkexec.verdict(Some(127), ""), Verdict::NotAsked);
        assert_eq!(Tool::Pkexec.verdict(Some(1), ""), Verdict::Failed);
        let denied = "Failed to start transient service unit: Access denied";
        assert_eq!(Tool::Run0.verdict(Some(1), denied), Verdict::Denied);
        let bus = "Failed to start transient service unit: Connection refused";
        assert_eq!(Tool::Run0.verdict(Some(1), bus), Verdict::NotAsked);
        assert_eq!(
            Tool::Run0.verdict(Some(1), "renice: failed"),
            Verdict::Failed
        );
        assert_eq!(Tool::Run0.verdict(Some(0), ""), Verdict::Done);
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

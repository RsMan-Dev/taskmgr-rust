//! Actions on processes: signals and priority.

use rustix::io::Errno;
use rustix::process::{
    Pid, PidfdFlags, kill_process, pidfd_open, pidfd_send_signal, setpriority_process,
};

use super::procfs::parse_stat;
#[cfg(test)]
use super::procfs::status_kb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Terminate,
    Kill,
    Suspend,
    Resume,
    Hangup,
    Interrupt,
    User1,
    User2,
}

impl Signal {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "term" => Self::Terminate,
            "kill" => Self::Kill,
            "stop" => Self::Suspend,
            "cont" => Self::Resume,
            "hup" => Self::Hangup,
            "int" => Self::Interrupt,
            "usr1" => Self::User1,
            "usr2" => Self::User2,
            _ => return None,
        })
    }

    fn raw(self) -> rustix::process::Signal {
        use rustix::process::Signal as S;
        match self {
            Self::Terminate => S::TERM,
            Self::Kill => S::KILL,
            Self::Suspend => S::STOP,
            Self::Resume => S::CONT,
            Self::Hangup => S::HUP,
            Self::Interrupt => S::INT,
            Self::User1 => S::USR1,
            Self::User2 => S::USR2,
        }
    }
}

/// A process as the interface saw it: the start time tells a recycled pid
/// apart, so an action never hits a process that took over the number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub pid: i32,
    /// Start time in clock ticks since boot; 0 skips the check.
    pub start: u64,
}

impl Target {
    /// `"pid:start,pid:start"`, as sent by the interface.
    pub fn parse_list(s: &str) -> Vec<Target> {
        s.split(',')
            .filter_map(|t| {
                let (pid, start) = t.trim().split_once(':').unwrap_or((t.trim(), "0"));
                Some(Target {
                    pid: pid.parse().ok().filter(|p| *p > 0)?,
                    start: start.parse().unwrap_or(0),
                })
            })
            .collect()
    }

    fn still_same(&self) -> bool {
        self.start == 0
            || std::fs::read_to_string(format!("/proc/{}/stat", self.pid))
                .ok()
                .and_then(|s| parse_stat(&s))
                .is_some_and(|s| s.start == self.start)
    }
}

/// Signals every target. Returns false if any was denied; processes that
/// exited (or whose pid was reused) are skipped, not an error. A pidfd pins
/// the process between the identity check and the signal.
pub fn send(targets: &[Target], signal: Signal) -> bool {
    let mut ok = true;
    for t in targets {
        if let Err(e) = send_one(*t, signal.raw())
            && e != Errno::SRCH
        {
            ok = false;
        }
    }
    ok
}

fn send_one(t: Target, signal: rustix::process::Signal) -> Result<(), Errno> {
    let pid = Pid::from_raw(t.pid).ok_or(Errno::SRCH)?;
    match pidfd_open(pid, PidfdFlags::empty()) {
        Ok(fd) if t.still_same() => pidfd_send_signal(&fd, signal),
        Ok(_) => Ok(()),
        Err(Errno::SRCH) => Ok(()),
        // Kernels before 5.3 have no pidfd.
        Err(_) if t.still_same() => kill_process(pid, signal),
        Err(_) => Ok(()),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PriorityError {
    /// Raising priority (lower nice) needs root.
    Denied,
    Gone,
    Failed(String),
}

/// Sets the nice value (-20 highest to 19 lowest).
pub fn set_priority(target: Target, nice: i32) -> Result<(), PriorityError> {
    if !target.still_same() {
        return Err(PriorityError::Gone);
    }
    let pid = Pid::from_raw(target.pid).ok_or(PriorityError::Gone)?;
    setpriority_process(Some(pid), nice).map_err(|e| match e {
        Errno::PERM | Errno::ACCESS => PriorityError::Denied,
        Errno::SRCH => PriorityError::Gone,
        e => PriorityError::Failed(e.to_string()),
    })
}

/// Same through pkexec and renice, for priorities only root may set.
/// Blocks until the password dialog is answered.
pub fn set_priority_as_admin(pid: i32, nice: i32) -> Result<(), String> {
    let pkexec =
        crate::energy::find_program("pkexec", &["/run/wrappers/bin/pkexec", "/usr/bin/pkexec"])
            .ok_or("pkexec not found (polkit package).")?;
    let renice = crate::energy::find_program(
        "renice",
        &[
            "/run/current-system/sw/bin/renice",
            "/usr/bin/renice",
            "/bin/renice",
        ],
    )
    .ok_or("renice not found.")?;
    let status = std::process::Command::new(pkexec)
        .args([
            renice.as_str(),
            "-n",
            &nice.to_string(),
            "-p",
            &pid.to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    match status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) => Err("Authorization denied or cancelled.".to_owned()),
        _ => Err(format!("The command failed ({status}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspend_resume_and_priority_on_own_process() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        let state = || {
            let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
            parse_stat(&s).unwrap()
        };
        let me = Target {
            pid,
            start: state().start,
        };
        assert!(send(&[me], Signal::Suspend));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(state().state, 'T');
        assert!(send(&[me], Signal::Resume));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_ne!(state().state, 'T');
        assert_eq!(set_priority(me, 5), Ok(()));
        let reused = Target {
            pid,
            start: me.start + 1,
        };
        assert_eq!(set_priority(reused, 0), Err(PriorityError::Gone));
        assert!(send(&[reused], Signal::Kill));
        assert_ne!(state().state, 'Z');
        assert_eq!(state().nice, 5);
        assert!(send(&[me], Signal::Kill));
        let _ = child.wait();
    }

    #[test]
    fn terminate_kills_own_process_and_reports_denial() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let me = Target {
            pid: child.id() as i32,
            start: 0,
        };
        assert!(send(&[me], Signal::Terminate));
        assert!(!child.wait().unwrap().success());
        // In a build sandbox pid 1 may be ours: only check denial when it is not.
        let uid = |pid: &str| {
            std::fs::read_to_string(format!("/proc/{pid}/status"))
                .ok()
                .and_then(|s| status_kb(&s, "Uid"))
        };
        if uid("1").is_some() && uid("1") != uid("self") {
            assert!(!send(&[Target { pid: 1, start: 0 }], Signal::Terminate));
        }
        assert!(send(&[me], Signal::Kill));
    }
}

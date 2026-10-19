//! Which desktop entry launched a program, from its `Exec=` line: for the
//! processes that systemd did not put in a unit of their own (desktops
//! without per-application scopes, OpenRC, a login session).

/// Programs that run something else: their entries say nothing about the
/// process.
const RUNNERS: [&str; 14] = [
    "sh",
    "bash",
    "zsh",
    "env",
    "python",
    "python3",
    "perl",
    "node",
    "java",
    "flatpak",
    "snap",
    "pkexec",
    "sudo",
    "gtk-launch",
];

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Launch {
    pub id: String,
    pub program: String,
    /// Arguments the command line must contain, field codes removed.
    pub args: Vec<String>,
}

/// Desktop entry quoting: double quotes, backslash escapes inside them.
pub(super) fn split(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut quoted, mut escaped, mut started) = (false, false, false);
    for c in exec.chars() {
        if escaped {
            cur.push(c);
            escaped = false;
        } else if quoted && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
            started = true;
        } else if c.is_whitespace() && !quoted {
            if started {
                out.push(std::mem::take(&mut cur));
                started = false;
            }
        } else {
            cur.push(c);
            started = true;
        }
    }
    if started {
        out.push(cur);
    }
    out
}

/// File name without the NixOS wrapper decoration (`.firefox-wrapped`).
pub(super) fn program_name(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    let base = base.strip_prefix('.').unwrap_or(base);
    base.strip_suffix("-wrapped").unwrap_or(base).to_owned()
}

pub(super) fn launch(id: &str, exec: &str) -> Option<Launch> {
    let mut argv = split(exec)
        .into_iter()
        .skip_while(|a| a == "env" || a.contains('='));
    let program = program_name(&argv.next()?);
    if RUNNERS.contains(&program.as_str()) {
        return None;
    }
    let args = argv
        .filter(|a| !(a.len() == 2 && a.starts_with('%')))
        .collect();
    Some(Launch {
        id: id.to_owned(),
        program,
        args,
    })
}

/// The entry whose arguments all appear on the command line, the most
/// specific one winning; a tie means the process cannot be told apart.
pub(super) fn best<'a>(candidates: &'a [Launch], cmdline: &[String]) -> Option<&'a str> {
    let mut fitting: Vec<&Launch> = candidates
        .iter()
        .filter(|l| {
            l.args
                .iter()
                .all(|a| cmdline.get(1..).unwrap_or_default().contains(a))
        })
        .collect();
    fitting.sort_by_key(|l| std::cmp::Reverse(l.args.len()));
    match fitting.as_slice() {
        [one] => Some(&one.id),
        [a, b, ..] if a.args.len() > b.args.len() => Some(&a.id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn splits_quotes_and_drops_field_codes() {
        assert_eq!(
            split(r#"env A=1 "/opt/My App/app" --name="x y" %U"#),
            argv(&["env", "A=1", "/opt/My App/app", "--name=x y", "%U"])
        );
        let l = launch("app", r#"env A=1 "/opt/My App/app" --flag %U"#).unwrap();
        assert_eq!((l.program.as_str(), l.args), ("app", argv(&["--flag"])));
        assert_eq!(launch("x", "sh -c 'echo'"), None);
        assert_eq!(program_name("/nix/store/x/bin/.firefox-wrapped"), "firefox");
    }

    #[test]
    fn web_apps_share_a_browser_binary() {
        let all = [
            launch("google-chrome", "/opt/google/chrome/chrome %U").unwrap(),
            launch(
                "chrome-abc-Default",
                "/opt/google/chrome/chrome --profile-directory=Default --app-id=abc",
            )
            .unwrap(),
            launch(
                "chrome-def-Default",
                "/opt/google/chrome/chrome --profile-directory=Default --app-id=def",
            )
            .unwrap(),
        ];
        let app = argv(&["chrome", "--profile-directory=Default", "--app-id=def"]);
        assert_eq!(best(&all, &app), Some("chrome-def-Default"));
        assert_eq!(
            best(&all, &argv(&["chrome", "--type=renderer"])),
            Some("google-chrome")
        );
        let twins = [launch("a", "app").unwrap(), launch("b", "app").unwrap()];
        assert_eq!(best(&twins, &argv(&["app"])), None);
    }
}

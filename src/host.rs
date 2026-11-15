//! Running host programs (`gh`, `flatpak`, `xdg-open`, ...).
//!
//! Floe runs either natively or as a flatpak. Inside the flatpak sandbox the host's
//! `gh` login and flatpak installation aren't visible, so commands are sent to the host
//! with `flatpak-spawn --host` (the manifest grants `--talk-name=org.freedesktop.Flatpak`).

use std::path::Path;
use std::process::{Command, Stdio};

/// True when running inside a flatpak sandbox.
pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// Program name, overridable for testing: `FLOE_GH=/path/to/fake-gh` etc.
fn program(name: &str) -> String {
    let var = format!("FLOE_{}", name.to_ascii_uppercase().replace('-', "_"));
    std::env::var(var).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| name.to_string())
}

/// Environment for every command: plain, English, non-interactive output.
const ENV: &[(&str, &str)] = &[
    ("LC_ALL", "C"),
    ("NO_COLOR", "1"),
    ("GH_PROMPT_DISABLED", "1"),
    ("GH_NO_UPDATE_NOTIFIER", "1"),
    ("GH_SPINNER_DISABLED", "1"),
];

/// A `Command` for `name args...`, routed to the host when sandboxed.
pub fn command(name: &str, args: &[&str]) -> Command {
    let prog = program(name);
    let mut cmd = if in_flatpak() {
        let mut c = Command::new("flatpak-spawn");
        c.arg("--host");
        for (k, v) in ENV {
            c.arg(format!("--env={k}={v}"));
        }
        c.arg(prog);
        c
    } else {
        let mut c = Command::new(prog);
        c.envs(ENV.iter().copied());
        c
    };
    cmd.args(args).stdin(Stdio::null());
    cmd
}

#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub ok: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// stderr and stdout combined, trimmed (for error messages and the activity log).
    pub fn text(&self) -> String {
        let mut s = self.stderr.trim().to_string();
        let out = self.stdout.trim();
        if !out.is_empty() {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(out);
        }
        s
    }

    /// The most useful single line for a short error message.
    pub fn summary(&self) -> String {
        let text = self.text();
        let line = text
            .lines()
            .map(str::trim)
            .rfind(|l| l.to_ascii_lowercase().starts_with("error") || l.contains("failed"))
            .or_else(|| text.lines().map(str::trim).rfind(|l| !l.is_empty()))
            .unwrap_or("")
            .to_string();
        if line.is_empty() {
            format!("exited with code {}", self.code.map_or("?".into(), |c| c.to_string()))
        } else {
            line
        }
    }
}

/// Run a program to completion. `Err` only when it could not be started at all
/// (usually: not installed).
pub fn run(name: &str, args: &[&str]) -> Result<Output, String> {
    let out = command(name, args).output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound if in_flatpak() => {
            "flatpak-spawn is missing; Floe can't reach the host system".to_string()
        }
        std::io::ErrorKind::NotFound => format!("`{name}` is not installed (not found in PATH)"),
        _ => format!("could not run `{name}`: {e}"),
    })?;
    let result = Output {
        ok: out.status.success(),
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    };
    // flatpak-spawn reports a missing host program as exit code 127 / "No such file".
    if in_flatpak() && result.code == Some(127) {
        return Err(format!("`{name}` is not installed on this computer"));
    }
    Ok(result)
}

/// Start a program and don't wait for it (launching an app, opening a link).
pub fn spawn_detached(name: &str, args: &[&str]) -> Result<(), String> {
    command(name, args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut child| {
            // Reap it in the background so it doesn't linger as a zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        })
        .map_err(|e| format!("could not start `{name}`: {e}"))
}

/// Open a web page in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    spawn_detached("xdg-open", &[url])
}

/// Show a desktop notification (best effort; silently does nothing without notify-send).
pub fn notify(title: &str, body: &str) {
    let _ = spawn_detached("notify-send", &["--app-name=Floe", "--icon=io.github.tobeoffline64.Floe", title, body]);
}

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
        // --watch-bus: the host process ends if Floe exits, so nothing keeps running unseen.
        c.arg("--host").arg("--watch-bus");
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

/// Run a program and pass each line of its output to `job` as it arrives (stdout and stderr,
/// split on newlines and on the carriage returns progress bars use). Stops the program when
/// the job is cancelled: SIGTERM first (flatpak-spawn forwards it to the host process), then
/// SIGKILL if it doesn't exit within a few seconds.
pub fn run_streaming(name: &str, args: &[&str], job: &crate::console::Handle) -> Result<Output, String> {
    use std::io::Read;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    job.line(format!("$ {name} {}", args.join(" ")));
    let mut cmd = command(name, args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound if in_flatpak() => "flatpak-spawn is missing; Floe can't reach the host system".to_string(),
        std::io::ErrorKind::NotFound => format!("`{name}` is not installed (not found in PATH)"),
        _ => format!("could not run `{name}`: {e}"),
    })?;

    // (is_stderr, line, ended with \r = a progress line the next one redraws)
    let (tx, rx) = mpsc::channel::<(bool, String, bool)>();
    let readers: Vec<_> = [(false, child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>)),
        (true, child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>))]
    .into_iter()
    .filter_map(|(is_err, pipe)| pipe.map(|p| (is_err, p)))
    .map(|(is_err, mut pipe)| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut pending: Vec<u8> = Vec::new();
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        for &b in &buf[..n] {
                            if b == b'\n' || b == b'\r' {
                                if !pending.is_empty() {
                                    let _ = tx.send((is_err, String::from_utf8_lossy(&pending).into_owned(), b == b'\r'));
                                    pending.clear();
                                }
                            } else {
                                pending.push(b);
                            }
                        }
                    }
                }
            }
            if !pending.is_empty() {
                let _ = tx.send((is_err, String::from_utf8_lossy(&pending).into_owned(), false));
            }
        })
    })
    .collect();
    drop(tx);

    let (mut stdout, mut stderr) = (String::new(), String::new());
    let mut term_sent: Option<Instant> = None;
    // The console line a progress bar keeps redrawing (like a terminal does with \r).
    let mut redraw = [false, false];
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok((is_err, line, cr)) => {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    continue;
                }
                let stream = usize::from(is_err);
                if redraw[stream] { job.replace_last(trimmed) } else { job.line(trimmed) }
                redraw[stream] = cr;
                // Keep only final states of progress lines in the captured output.
                if !cr {
                    let out = if is_err { &mut stderr } else { &mut stdout };
                    out.push_str(trimmed);
                    out.push('\n');
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if job.cancelled() {
            match term_sent {
                None => {
                    job.line("Cancelling…");
                    // SAFETY: plain kill(2) on our own child's pid.
                    unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
                    term_sent = Some(Instant::now());
                }
                Some(t) if t.elapsed() > Duration::from_secs(5) => {
                    let _ = child.kill();
                    term_sent = Some(Instant::now() + Duration::from_secs(3600));
                }
                _ => {}
            }
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.wait().map_err(|e| format!("`{name}` did not finish: {e}"))?;
    if job.cancelled() {
        job.line("Cancelled.");
        return Err("Cancelled.".into());
    }
    let out = Output { ok: status.success(), code: status.code(), stdout, stderr };
    if in_flatpak() && out.code == Some(127) {
        return Err(format!("`{name}` is not installed on this computer"));
    }
    job.line(match out.code {
        Some(0) => "Finished.".to_string(),
        Some(c) => format!("Exited with code {c}."),
        None => "Stopped by a signal.".to_string(),
    });
    Ok(out)
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

/// Floe's own flatpak application ID and GitHub repository (for self-updates).
pub const FLOE_APP_ID: &str = "io.github.tobeoffline64.Floe";
pub const FLOE_REPO: &str = "tobeoffline64/Floe";

/// When running as a flatpak: whether Floe itself is installed per-user or system-wide
/// (read from /.flatpak-info, whose app-path is under /var/lib/flatpak for system installs).
pub fn own_scope() -> Option<crate::flatpak::Scope> {
    let info = std::fs::read_to_string("/.flatpak-info").ok()?;
    Some(parse_own_scope(&info))
}

fn parse_own_scope(info: &str) -> crate::flatpak::Scope {
    let path = info.lines().find_map(|l| l.trim().strip_prefix("app-path=")).unwrap_or("");
    if path.starts_with("/var/lib/flatpak") || path.starts_with("/usr/") {
        crate::flatpak::Scope::System
    } else {
        crate::flatpak::Scope::User
    }
}

/// Start a fresh copy of Floe that outlives this one (used after a self-update). Unlike
/// [`spawn_detached`] this doesn't tie the host process to Floe's lifetime.
pub fn relaunch_self() -> Result<(), String> {
    let mut cmd = if in_flatpak() {
        let mut c = Command::new("flatpak-spawn");
        c.args(["--host", "flatpak", "run", FLOE_APP_ID]);
        c
    } else {
        Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map(|_| ()).map_err(|e| format!("could not restart Floe: {e}"))
}

/// Open a web page in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    spawn_detached("xdg-open", &[url])
}

/// Show a desktop notification (best effort; silently does nothing without notify-send).
pub fn notify(title: &str, body: &str) {
    let _ = spawn_detached("notify-send", &["--app-name=Floe", "--icon=io.github.tobeoffline64.Floe", title, body]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatpak::Scope;

    #[test]
    fn reads_own_scope() {
        let user = "[Application]\nname=io.github.tobeoffline64.Floe\n\n[Instance]\napp-path=/home/me/.local/share/flatpak/app/io.github.tobeoffline64.Floe/x86_64/master/abc/files\n";
        assert_eq!(parse_own_scope(user), Scope::User);
        let system = "[Instance]\napp-path=/var/lib/flatpak/app/io.github.tobeoffline64.Floe/x86_64/master/abc/files\n";
        assert_eq!(parse_own_scope(system), Scope::System);
    }
}

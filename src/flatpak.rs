//! Installing flatpak bundles and reading what's installed, through the `flatpak` command.

use crate::host;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Per-user installation (no password needed). The default.
    #[default]
    User,
    /// System-wide installation (flatpak asks for an admin password through polkit).
    System,
}

impl Scope {
    pub const ALL: [Scope; 2] = [Scope::User, Scope::System];
    pub fn flag(self) -> &'static str {
        match self {
            Scope::User => "--user",
            Scope::System => "--system",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::System => "system",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Scope::User => "Just me (--user)",
            Scope::System => "All users (--system, asks for a password)",
        }
    }
    pub fn from_key(k: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|s| s.key() == k)
    }
}

pub fn available() -> Result<String, String> {
    let out = host::run("flatpak", &["--version"])?;
    if out.ok { Ok(out.stdout.trim().to_string()) } else { Err(out.summary()) }
}

/// Installed application IDs in both installations.
pub fn installed_apps() -> Vec<String> {
    let mut ids = Vec::new();
    for scope in Scope::ALL {
        if let Ok(o) = host::run("flatpak", &["list", "--app", scope.flag(), "--columns=application"]) {
            ids.extend(o.stdout.lines().map(str::trim).filter(|l| is_app_id(l)).map(String::from));
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

/// The version of an installed app (from its AppStream metadata), if it is installed.
pub fn installed_version(app_id: &str) -> Option<String> {
    if !is_app_id(app_id) {
        return None;
    }
    for scope in Scope::ALL {
        if let Ok(o) = host::run("flatpak", &["info", scope.flag(), app_id])
            && o.ok {
                return Some(parse_info_version(&o.stdout).unwrap_or_default());
            }
    }
    None
}

/// Whether the app is installed at all (even without a version in its metadata).
pub fn is_installed(app_id: &str) -> bool {
    installed_version(app_id).is_some()
}

fn parse_info_version(info: &str) -> Option<String> {
    info.lines()
        .filter_map(|l| l.trim().strip_prefix("Version:"))
        .map(|v| v.trim().to_string())
        .find(|v| !v.is_empty())
}

/// Install (or reinstall over an existing copy) a `.flatpak` bundle. App data in
/// `~/.var/app/<id>` is kept. Returns the command output for the activity log.
pub fn install_bundle(path: &Path, scope: Scope) -> Result<String, String> {
    let p = path.to_string_lossy().into_owned();
    let out = host::run("flatpak", &["install", scope.flag(), "-y", "--noninteractive", "--reinstall", &p])?;
    if out.ok {
        return Ok(out.text());
    }
    let text = out.text();
    let lower = text.to_lowercase();
    if lower.contains("already installed") && !lower.contains("error: failed") {
        // Same commit as what's installed: nothing to do, which counts as success.
        return Ok(text);
    }
    Err(if lower.contains("runtime") && (lower.contains("not installed") || lower.contains("no remote")) {
        format!(
            "{}\nThe app's runtime is missing. Add Flathub once with:\nflatpak remote-add {} --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo",
            out.summary(),
            scope.flag()
        )
    } else {
        out.summary()
    })
}

/// Start an installed app.
pub fn launch(app_id: &str) -> Result<(), String> {
    if !is_app_id(app_id) {
        return Err(format!("\"{app_id}\" is not a valid app ID"));
    }
    host::spawn_detached("flatpak", &["run", app_id])
}

/// Reverse-DNS application ID like "io.github.owner.App".
pub fn is_app_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && !p.starts_with(|c: char| c.is_ascii_digit())
                && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

/// After installing a bundle whose app ID wasn't known: the newly listed app, or an app ID
/// mentioned in flatpak's output (on a reinstall the list doesn't change).
pub fn detect_app_id(before: &[String], after: &[String], install_output: &str, repo: &str) -> Option<String> {
    let new: Vec<&String> = after.iter().filter(|a| !before.contains(a)).collect();
    if new.len() == 1 {
        return Some(new[0].clone());
    }
    let mentioned: Vec<String> = install_output
        .split(|c: char| c.is_whitespace() || c == '/' || c == '\'' || c == '"' || c == ',')
        .filter(|w| is_app_id(w) && after.iter().any(|a| a == w))
        .map(String::from)
        .collect();
    // Prefer one that contains the repository's name ("owner/PenguinFin" -> "...PenguinFin").
    let name = repo.rsplit('/').next().unwrap_or("").to_lowercase();
    mentioned
        .iter()
        .find(|id| !name.is_empty() && id.to_lowercase().contains(&name))
        .or(mentioned.first())
        .cloned()
        .or_else(|| new.first().map(|s| s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids() {
        assert!(is_app_id("io.github.tobeoffline64.PenguinFin"));
        assert!(is_app_id("org.gnome.Platform"));
        assert!(!is_app_id("penguinfin"));
        assert!(!is_app_id("1.3.0.1"));
        assert!(!is_app_id("io..x"));
    }

    #[test]
    fn info_version() {
        let info = "\nPenguinFin - Offline planner\n\n          ID: io.github.tobeoffline64.PenguinFin\n         Ref: app/x\n     Version: 1.3.0\n     License: MIT\n";
        assert_eq!(parse_info_version(info).as_deref(), Some("1.3.0"));
        assert_eq!(parse_info_version("ID: x"), None);
    }

    #[test]
    fn detects_app_ids() {
        let before = vec!["org.a.One".to_string()];
        let after = vec!["org.a.One".to_string(), "io.github.me.Two".to_string()];
        assert_eq!(detect_app_id(&before, &after, "", "me/two").as_deref(), Some("io.github.me.Two"));
        // Reinstall: nothing new, but the output names it.
        let out = "Installing app/io.github.me.Two/x86_64/master\n 1. io.github.me.Two  master  i  user  < 5 MB";
        assert_eq!(detect_app_id(&after, &after, out, "me/two").as_deref(), Some("io.github.me.Two"));
        assert_eq!(detect_app_id(&after, &after, "nothing", "me/two"), None);
    }
}

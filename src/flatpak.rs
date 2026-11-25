//! Installing flatpak bundles and reading what's installed, through the `flatpak` command.

use crate::console::Handle;
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

/// An app found in `flatpak list`.
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The remote it came from; a bundle installed from a file gets "<name>-origin".
    pub origin: String,
    pub scope: Scope,
}

/// Every installed application in both installations.
pub fn installed_list() -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    for scope in Scope::ALL {
        if let Ok(o) = host::run("flatpak", &["list", "--app", scope.flag(), "--columns=application,name,version,origin"]) {
            apps.extend(parse_list(&o.stdout, scope));
        }
    }
    apps
}

fn parse_list(stdout: &str, scope: Scope) -> Vec<InstalledApp> {
    stdout
        .lines()
        .filter_map(|l| {
            let mut cols = l.split('\t').map(str::trim);
            let id = cols.next()?.to_string();
            if !is_app_id(&id) {
                return None;
            }
            Some(InstalledApp {
                name: cols.next().unwrap_or("").to_string(),
                version: cols.next().unwrap_or("").to_string(),
                origin: cols.next().unwrap_or("").to_string(),
                id,
                scope,
            })
        })
        .collect()
}

/// Installed application IDs in both installations.
pub fn installed_apps() -> Vec<String> {
    let mut ids: Vec<String> = installed_list().into_iter().map(|a| a.id).collect();
    ids.sort();
    ids.dedup();
    ids
}

/// How sure a match between a repository and an installed app is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// The ID is `io.github.<owner>.<name>`, or a bundle (origin `<name>-origin`) with that
    /// name: linked automatically.
    Sure,
    /// Only the name matches (for example an app from Flathub): suggested, never linked silently.
    Possible,
}

/// The installed app that belongs to a GitHub repository ("owner/Name"), if there is exactly
/// one candidate.
pub fn match_installed<'a>(repo: &str, apps: &'a [InstalledApp]) -> Option<(&'a InstalledApp, Match)> {
    let (owner, name) = repo.split_once('/')?;
    let exact = format!("io.github.{}.{}", owner.to_lowercase(), name.to_lowercase());
    if let Some(a) = apps.iter().find(|a| a.id.to_lowercase() == exact) {
        return Some((a, Match::Sure));
    }
    let squash = |s: &str| s.to_lowercase().replace(['-', '_'], "");
    let want = squash(name);
    let candidates: Vec<&InstalledApp> = apps
        .iter()
        .filter(|a| squash(a.id.rsplit('.').next().unwrap_or("")) == want || a.is_bundle_of(&want))
        .collect();
    let mut ids: Vec<&str> = candidates.iter().map(|a| a.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    if ids.len() != 1 {
        return None;
    }
    let app = candidates.iter().find(|a| a.is_bundle_of(&want)).or(candidates.first()).copied()?;
    Some((app, if app.is_bundle_of(&want) { Match::Sure } else { Match::Possible }))
}

impl InstalledApp {
    /// Installed from a bundle file (not a remote such as Flathub).
    pub fn from_bundle(&self) -> bool {
        self.origin.ends_with("-origin")
    }

    fn is_bundle_of(&self, squashed_name: &str) -> bool {
        self.from_bundle()
            && self.origin.trim_end_matches("-origin").to_lowercase().replace(['-', '_'], "") == squashed_name
    }
}

/// For an untracked installed app with a GitHub-style ID (`io.github.owner.Name`), the
/// repository it most likely comes from.
pub fn guess_repo(app_id: &str) -> Option<String> {
    let parts: Vec<&str> = app_id.split('.').collect();
    match parts.as_slice() {
        ["io", "github", owner, name] => Some(format!("{owner}/{name}")),
        _ => None,
    }
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

/// Install (or reinstall over an existing copy) a `.flatpak` bundle, streaming flatpak's
/// output to `job`. App data in `~/.var/app/<id>` is kept. Returns the output for the log.
pub fn install_bundle(path: &Path, scope: Scope, job: &Handle) -> Result<String, String> {
    let p = path.to_string_lossy().into_owned();
    let out = host::run_streaming("flatpak", &install_args(scope, &p), job)?;
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
    } else if lower.contains("not allowed for user") || lower.contains("polkit") || lower.contains("authoriz") {
        format!(
            "{}\nInstalling a bundle for all users needs an administrator password, and no password prompt was shown or it was cancelled. \
             Try again and enter your password when asked, or install it from a terminal:\nflatpak install --system --reinstall {}",
            out.summary(),
            shell_quote(&p)
        )
    } else {
        out.summary()
    })
}

/// `flatpak install` arguments. A system-wide bundle install always needs an administrator
/// password (polkit `install-bundle`), and `--noninteractive` makes flatpak refuse instead of
/// asking ("not allowed for user"), so it's only used for per-user installs. `-y` still
/// answers flatpak's own questions either way.
pub fn install_args(scope: Scope, path: &str) -> Vec<&str> {
    match scope {
        Scope::User => vec!["install", "--user", "-y", "--noninteractive", "--reinstall", path],
        Scope::System => vec!["install", "--system", "-y", "--reinstall", path],
    }
}

/// Quote a path for a command the user can paste into a terminal.
pub fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+:@".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
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
    fn install_arguments() {
        assert_eq!(install_args(Scope::User, "/a.flatpak"), ["install", "--user", "-y", "--noninteractive", "--reinstall", "/a.flatpak"]);
        // System installs must be allowed to show the polkit password prompt.
        assert!(!install_args(Scope::System, "/a.flatpak").contains(&"--noninteractive"));
        assert_eq!(shell_quote("/home/me/x.flatpak"), "/home/me/x.flatpak");
        assert_eq!(shell_quote("/home/me/my app's.flatpak"), "'/home/me/my app'\\''s.flatpak'");
    }

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
    fn lists_and_matches_installed() {
        let out = "io.github.tobeoffline64.PenguinFin\tPenguinFin\t1.3.0\tpenguinfin-origin\norg.gnome.Calculator\tCalculator\t47.1\tflathub\ncom.example.Notes-App\tNotes\t\tnotes-app-origin\n";
        let apps = parse_list(out, Scope::User);
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0].version, "1.3.0");
        assert_eq!(apps[0].origin, "penguinfin-origin");
        let m = |r: &str| match_installed(r, &apps).map(|(a, m)| (a.id.clone(), m));
        let pf = Some(("io.github.tobeoffline64.PenguinFin".to_string(), Match::Sure));
        assert_eq!(m("tobeoffline64/PenguinFin"), pf);
        assert_eq!(m("TobeOffline64/penguinfin"), pf);
        // Someone else's fork with the same name: still the installed bundle named penguinfin.
        assert_eq!(m("fork/PenguinFin"), pf);
        assert_eq!(m("someone/notes_app"), Some(("com.example.Notes-App".to_string(), Match::Sure)));
        // From Flathub, name only: just a suggestion.
        assert_eq!(m("gnome/calculator"), Some(("org.gnome.Calculator".to_string(), Match::Possible)));
        assert_eq!(m("me/unknown"), None);
        assert_eq!(guess_repo("io.github.tobeoffline64.PenguinFin").as_deref(), Some("tobeoffline64/PenguinFin"));
        assert_eq!(guess_repo("org.gnome.Calculator"), None);
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

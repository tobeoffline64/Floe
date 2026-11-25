//! Check and update steps for one tracked repository. Each function is blocking; the app
//! runs them on worker threads and applies the results to its state.

use crate::config::{Settings, TrackedRepo};
use crate::console::Handle;
use crate::flatpak::{InstalledApp, Match};
use crate::{config, flatpak, gh};
use std::path::PathBuf;

/// The outcome of a check, applied with [`apply_check`].
#[derive(Clone, Debug)]
pub struct Check {
    /// The newest stable release and newer beta, if any.
    pub channels: Result<gh::Channels, String>,
    /// What `flatpak list` shows on this computer.
    pub installed: Vec<InstalledApp>,
}

pub fn check(repo: &TrackedRepo) -> Check {
    Check { channels: gh::channels(&repo.repo), installed: flatpak::installed_list() }
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// Copy what flatpak reports for the repository's app into it.
fn sync_installed(repo: &mut TrackedRepo, app: Option<&InstalledApp>) {
    repo.app_present = app.is_some();
    repo.installed_version = app.map(|a| a.version.clone()).filter(|v| !v.is_empty());
    repo.installed_origin = app.map(|a| a.origin.clone()).unwrap_or_default();
    // Uninstalled outside Floe: forget the installed tag so it offers a fresh install.
    if app.is_none() {
        repo.installed_tag = None;
    }
}

/// Link the repository to an app that's already installed, when the app ID isn't known yet:
/// automatically for a sure match, as a suggestion otherwise. Returns a message for the log.
pub fn link_installed(repo: &mut TrackedRepo, installed: &[InstalledApp]) -> Option<String> {
    if !repo.app_id.is_empty() {
        repo.suggested_app_id = None;
        return None;
    }
    match flatpak::match_installed(&repo.repo, installed) {
        Some((app, Match::Sure)) => {
            repo.app_id = app.id.clone();
            repo.suggested_app_id = None;
            sync_installed(repo, Some(app));
            Some(format!(
                "Found it already installed: {} {}{} — linked",
                app.id,
                app.version,
                if app.from_bundle() { "" } else { " (from Flathub or another remote)" }
            ))
        }
        Some((app, Match::Possible)) => {
            let new = repo.suggested_app_id.as_deref() != Some(app.id.as_str());
            repo.suggested_app_id = Some(app.id.clone());
            new.then(|| format!("Installed app {} (from {}) may belong to this repository", app.id, app.origin))
        }
        None => {
            repo.suggested_app_id = None;
            None
        }
    }
}

/// Apply a check. Returns log messages about installed apps that were found.
pub fn apply_check(repo: &mut TrackedRepo, c: Check) -> Option<String> {
    repo.last_checked = Some(now());
    let note = link_installed(repo, &c.installed);
    if !repo.app_id.is_empty() {
        let app = c.installed.iter().find(|a| a.id == repo.app_id);
        sync_installed(repo, app);
    }
    match c.channels {
        Ok(ch) => {
            repo.last_error = ch.pick_or_err(repo.include_prereleases).err();
            repo.stable = ch.stable;
            repo.beta = ch.beta;
            repo.latest = None;
            repo.retarget();
        }
        Err(e) => repo.last_error = Some(e),
    }
    note
}

/// Download the latest release's bundle. Returns the release it came from and the file.
pub fn download(repo: &TrackedRepo, job: &Handle) -> Result<(gh::Release, PathBuf), String> {
    let release = match &repo.latest {
        Some(r) => r.clone(),
        None => {
            job.line(format!("Looking up the latest release of {}…", repo.repo));
            gh::latest_release(&repo.repo, repo.include_prereleases)?
        }
    };
    let pattern = repo.pattern_for(&release).to_string();
    let asset = release.pick_asset(&pattern).ok_or_else(|| {
        let names: Vec<&str> = release.assets.iter().map(|a| a.name.as_str()).collect();
        format!(
            "Release {} has no file matching \"{}\" (files: {}).",
            release.tag,
            pattern,
            if names.is_empty() { "none".to_string() } else { names.join(", ") }
        )
    })?;
    let dir = config::download_dir().join(repo.repo.replace('/', "_"));
    let path = gh::download(&repo.repo, &release.tag, &asset.name, asset.size, &dir, job)?;
    Ok((release, path))
}

#[derive(Clone, Debug)]
pub struct Installed {
    pub app_id: Option<String>,
    pub app: Option<InstalledApp>,
    pub log: String,
}

/// Install a downloaded bundle, detect the app ID if it isn't known, and clean up.
pub fn install(repo: &TrackedRepo, bundle: &PathBuf, settings: &Settings, job: &Handle) -> Result<Installed, String> {
    let before = if repo.app_id.is_empty() { flatpak::installed_apps() } else { Vec::new() };
    job.line(format!("Installing {} ({})…", bundle.file_name().unwrap_or_default().to_string_lossy(), settings.scope.flag()));
    let result = flatpak::install_bundle(bundle, settings.scope, job);
    // Keep the file after a failure, so the install can be retried by hand from a terminal.
    if result.is_ok() && !settings.keep_downloads {
        let _ = std::fs::remove_file(bundle);
    }
    let log = result?;
    let after = flatpak::installed_list();
    let app_id = if repo.app_id.is_empty() {
        let ids: Vec<String> = after.iter().map(|a| a.id.clone()).collect();
        flatpak::detect_app_id(&before, &ids, &log, &repo.repo)
    } else {
        Some(repo.app_id.clone())
    };
    let app = app_id.as_deref().and_then(|id| after.iter().find(|a| a.id == id)).cloned();
    if let Some(a) = &app {
        job.line(format!("Installed {} {}", a.id, a.version));
    }
    Ok(Installed { app_id, app, log })
}

pub fn apply_install(repo: &mut TrackedRepo, release: &gh::Release, done: Installed) {
    if repo.app_id.is_empty()
        && let Some(id) = done.app_id
    {
        repo.app_id = id;
    }
    repo.suggested_app_id = None;
    sync_installed(repo, done.app.as_ref());
    // Even if `flatpak list` didn't show it (unusual), the install itself succeeded.
    repo.app_present = true;
    repo.installed_tag = Some(release.tag.clone());
    repo.installed_at = Some(now());
    repo.latest = Some(release.clone());
    repo.last_error = None;
}

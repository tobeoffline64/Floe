//! Check and update steps for one tracked repository. Each function is blocking; the app
//! runs them on worker threads and applies the results to its state.

use crate::config::{Settings, TrackedRepo};
use crate::{config, flatpak, gh};
use std::path::PathBuf;

/// The outcome of a check, applied with [`apply_check`].
#[derive(Clone, Debug)]
pub struct Check {
    pub latest: Result<gh::Release, String>,
    pub installed_version: Option<String>,
    pub app_present: bool,
}

pub fn check(repo: &TrackedRepo) -> Check {
    let latest = gh::latest_release(&repo.repo, repo.include_prereleases);
    let installed = if repo.app_id.is_empty() { None } else { flatpak::installed_version(&repo.app_id) };
    Check { latest, app_present: installed.is_some(), installed_version: installed.filter(|v| !v.is_empty()) }
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

pub fn apply_check(repo: &mut TrackedRepo, c: Check) {
    repo.last_checked = Some(now());
    if !repo.app_id.is_empty() {
        repo.app_present = c.app_present;
        repo.installed_version = c.installed_version;
        // Uninstalled outside Floe: forget the installed tag so it offers a fresh install.
        if !repo.app_present {
            repo.installed_tag = None;
        }
    }
    match c.latest {
        Ok(r) => {
            repo.latest = Some(r);
            repo.last_error = None;
        }
        Err(e) => repo.last_error = Some(e),
    }
}

/// Download the latest release's bundle. Returns the release it came from and the file.
pub fn download(repo: &TrackedRepo) -> Result<(gh::Release, PathBuf), String> {
    let release = match &repo.latest {
        Some(r) => r.clone(),
        None => gh::latest_release(&repo.repo, repo.include_prereleases)?,
    };
    let asset = release.pick_asset(&repo.asset_pattern).ok_or_else(|| {
        let names: Vec<&str> = release.assets.iter().map(|a| a.name.as_str()).collect();
        format!(
            "Release {} has no file matching \"{}\" (files: {}).",
            release.tag,
            repo.pattern(),
            if names.is_empty() { "none".to_string() } else { names.join(", ") }
        )
    })?;
    let dir = config::download_dir().join(repo.repo.replace('/', "_"));
    let path = gh::download(&repo.repo, &release.tag, &asset.name, &dir)?;
    Ok((release, path))
}

#[derive(Clone, Debug)]
pub struct Installed {
    pub app_id: Option<String>,
    pub installed_version: Option<String>,
    pub log: String,
}

/// Install a downloaded bundle, detect the app ID if it isn't known, and clean up.
pub fn install(repo: &TrackedRepo, bundle: &PathBuf, settings: &Settings) -> Result<Installed, String> {
    let before = if repo.app_id.is_empty() { flatpak::installed_apps() } else { Vec::new() };
    let result = flatpak::install_bundle(bundle, settings.scope);
    if !settings.keep_downloads {
        let _ = std::fs::remove_file(bundle);
    }
    let log = result?;
    let app_id = if repo.app_id.is_empty() {
        flatpak::detect_app_id(&before, &flatpak::installed_apps(), &log, &repo.repo)
    } else {
        Some(repo.app_id.clone())
    };
    let installed_version = app_id.as_deref().and_then(flatpak::installed_version).filter(|v| !v.is_empty());
    Ok(Installed { app_id, installed_version, log })
}

pub fn apply_install(repo: &mut TrackedRepo, release: &gh::Release, done: Installed) {
    if repo.app_id.is_empty()
        && let Some(id) = done.app_id {
            repo.app_id = id;
        }
    repo.app_present = true;
    repo.installed_version = done.installed_version;
    repo.installed_tag = Some(release.tag.clone());
    repo.installed_at = Some(now());
    repo.latest = Some(release.clone());
    repo.last_error = None;
}

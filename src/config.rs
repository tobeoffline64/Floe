//! The tracked repositories and app settings, stored as one JSON file
//! (`~/.config/floe/config.json`, or `$FLOE_CONFIG`).

use crate::flatpak::Scope;
use crate::gh::{self, Release};
use crate::version;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiScale {
    #[default]
    Auto,
    Small,
    Medium,
    Large,
    ExtraLarge,
}

impl UiScale {
    pub const ALL: [UiScale; 5] = [UiScale::Auto, UiScale::Small, UiScale::Medium, UiScale::Large, UiScale::ExtraLarge];
    const STEPS: [UiScale; 4] = [UiScale::Small, UiScale::Medium, UiScale::Large, UiScale::ExtraLarge];

    pub fn key(self) -> &'static str {
        match self {
            UiScale::Auto => "auto",
            UiScale::Small => "s",
            UiScale::Medium => "m",
            UiScale::Large => "l",
            UiScale::ExtraLarge => "xl",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            UiScale::Auto => "Auto",
            UiScale::Small => "S",
            UiScale::Medium => "M",
            UiScale::Large => "L",
            UiScale::ExtraLarge => "XL",
        }
    }
    pub fn long_label(self) -> &'static str {
        match self {
            UiScale::Auto => "Auto (fit window)",
            UiScale::Small => "Small",
            UiScale::Medium => "Medium",
            UiScale::Large => "Large",
            UiScale::ExtraLarge => "Extra large",
        }
    }
    /// CSS root font size; everything in the UI is sized in rem.
    pub fn root_font_size(self) -> &'static str {
        match self {
            UiScale::Auto => "clamp(13px, min(calc(8px + 0.55vw), calc(6px + 1.1vh)), 20px)",
            UiScale::Small => "14px",
            UiScale::Medium => "16px",
            UiScale::Large => "18px",
            UiScale::ExtraLarge => "20px",
        }
    }
    pub fn bigger(self) -> UiScale {
        match self {
            UiScale::Auto => UiScale::Large,
            s => {
                let i = Self::STEPS.iter().position(|x| *x == s).unwrap_or(1);
                Self::STEPS[(i + 1).min(Self::STEPS.len() - 1)]
            }
        }
    }
    pub fn smaller(self) -> UiScale {
        match self {
            UiScale::Auto => UiScale::Small,
            s => {
                let i = Self::STEPS.iter().position(|x| *x == s).unwrap_or(1);
                Self::STEPS[i.saturating_sub(1)]
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dark_mode: bool,
    pub ui_scale: UiScale,
    /// Check every repository a moment after the window opens.
    pub check_on_launch: bool,
    /// While Floe is open, check again every N hours (0 = never).
    pub check_interval_hours: u32,
    /// Desktop notification when a background check finds updates.
    pub notify: bool,
    /// Where bundles are installed.
    pub scope: Scope,
    /// Keep downloaded .flatpak files instead of deleting them after installing.
    pub keep_downloads: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dark_mode: false,
            ui_scale: UiScale::Auto,
            check_on_launch: true,
            check_interval_hours: 6,
            notify: true,
            scope: Scope::User,
            keep_downloads: false,
        }
    }
}

/// One GitHub repository whose releases Floe follows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackedRepo {
    pub id: u64,
    /// "owner/name"
    pub repo: String,
    /// Which release asset to install; empty means `*.flatpak`.
    pub asset_pattern: String,
    /// Flatpak application ID; filled in automatically after the first install if empty.
    pub app_id: String,
    pub include_prereleases: bool,
    /// The release tag Floe last installed.
    pub installed_tag: Option<String>,
    pub installed_at: Option<String>,
    /// Version flatpak reports for the installed app (refreshed on every check).
    pub installed_version: Option<String>,
    /// Whether the app ID is currently installed (refreshed on every check).
    pub app_present: bool,
    /// Where flatpak says the installed app came from ("<name>-origin" for a bundle file,
    /// "flathub", ...).
    pub installed_origin: String,
    /// An installed app whose name matches but that wasn't linked automatically.
    pub suggested_app_id: Option<String>,
    pub latest: Option<Release>,
    pub last_checked: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Never checked.
    Unknown,
    /// The last check failed and no earlier result is known.
    Error,
    /// The latest release has no asset matching the pattern.
    NoAsset,
    NotInstalled,
    UpdateAvailable,
    UpToDate,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Unknown => "Not checked",
            Status::Error => "Check failed",
            Status::NoAsset => "No matching file",
            Status::NotInstalled => "Not installed",
            Status::UpdateAvailable => "Update available",
            Status::UpToDate => "Up to date",
        }
    }
}

impl TrackedRepo {
    pub fn name(&self) -> &str {
        self.repo.rsplit('/').next().unwrap_or(&self.repo)
    }
    pub fn owner(&self) -> &str {
        self.repo.split('/').next().unwrap_or("")
    }
    pub fn url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }
    pub fn pattern(&self) -> &str {
        gh::effective_pattern(&self.asset_pattern)
    }
    pub fn latest_asset(&self) -> Option<&gh::Asset> {
        self.latest.as_ref()?.pick_asset(&self.asset_pattern)
    }
    /// Linked to an app that came from a remote like Flathub: updating replaces it with the
    /// GitHub build.
    pub fn installed_from_remote(&self) -> bool {
        self.app_present && !self.installed_origin.is_empty() && !self.installed_origin.ends_with("-origin")
    }

    pub fn is_installed(&self) -> bool {
        if self.app_id.is_empty() { self.installed_tag.is_some() } else { self.app_present }
    }

    /// What's installed, best source first: the flatpak's own version, else the tag Floe installed.
    pub fn current_version(&self) -> Option<&str> {
        self.installed_version.as_deref().filter(|v| !v.is_empty()).or(self.installed_tag.as_deref())
    }

    pub fn status(&self) -> Status {
        let Some(latest) = &self.latest else {
            return if self.last_error.is_some() { Status::Error } else { Status::Unknown };
        };
        if !self.is_installed() {
            return if self.latest_asset().is_some() { Status::NotInstalled } else { Status::NoAsset };
        }
        // Floe installed exactly this release (trust the tag over metadata that may lag behind).
        if self.installed_tag.as_deref() == Some(latest.tag.as_str()) {
            return Status::UpToDate;
        }
        let behind = match self.current_version().and_then(|cur| version::compare(cur, &latest.tag)) {
            Some(o) => o == std::cmp::Ordering::Less,
            // Not comparable (e.g. "nightly"): anything other than the installed tag is new.
            None => self.installed_tag.as_deref().is_some_and(|t| t != latest.tag),
        };
        if !behind {
            Status::UpToDate
        } else if self.latest_asset().is_none() {
            Status::NoAsset
        } else {
            Status::UpdateAvailable
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    pub repos: Vec<TrackedRepo>,
    pub next_id: u64,
}

impl Config {
    pub fn repo(&self, id: u64) -> Option<&TrackedRepo> {
        self.repos.iter().find(|r| r.id == id)
    }
    pub fn repo_mut(&mut self, id: u64) -> Option<&mut TrackedRepo> {
        self.repos.iter_mut().find(|r| r.id == id)
    }

    /// Add a repository (input as owner/name or a link). Returns its id.
    pub fn add(&mut self, input: &str, asset_pattern: &str, app_id: &str, prereleases: bool) -> Result<u64, String> {
        let repo = gh::parse_repo(input)?;
        let app_id = app_id.trim();
        if !app_id.is_empty() && !crate::flatpak::is_app_id(app_id) {
            return Err(format!("\"{app_id}\" isn't a flatpak app ID (like io.github.owner.App)."));
        }
        let pattern = asset_pattern.trim();
        if self.repos.iter().any(|r| r.repo.eq_ignore_ascii_case(&repo) && r.pattern() == gh::effective_pattern(pattern)) {
            return Err(format!("{repo} is already in your list."));
        }
        self.next_id = self.next_id.max(self.repos.iter().map(|r| r.id).max().unwrap_or(0)) + 1;
        let id = self.next_id;
        self.repos.push(TrackedRepo {
            id,
            repo,
            asset_pattern: if pattern == gh::DEFAULT_PATTERN { String::new() } else { pattern.to_string() },
            app_id: app_id.to_string(),
            include_prereleases: prereleases,
            ..Default::default()
        });
        Ok(id)
    }

    pub fn remove(&mut self, id: u64) {
        self.repos.retain(|r| r.id != id);
    }

    pub fn updates(&self) -> usize {
        self.repos.iter().filter(|r| r.status() == Status::UpdateAvailable).count()
    }

    pub fn load(path: &Path) -> Result<Config, String> {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Write atomically (temp file + rename), so a crash never leaves a half-written list.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The list without check results, for sharing or moving to another computer.
    pub fn export_list(&self) -> String {
        let list: Vec<ExportedRepo> = self
            .repos
            .iter()
            .map(|r| ExportedRepo {
                repo: r.repo.clone(),
                asset_pattern: r.asset_pattern.clone(),
                app_id: r.app_id.clone(),
                include_prereleases: r.include_prereleases,
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::json!({ "floe": 1, "repos": list })).unwrap_or_default()
    }

    /// Merge an exported list; skips entries that are already present. Returns how many were added.
    pub fn import_list(&mut self, json: &str) -> Result<usize, String> {
        #[derive(Deserialize)]
        struct File {
            repos: Vec<ExportedRepo>,
        }
        let file: File = serde_json::from_str(json).map_err(|e| format!("Not a Floe list: {e}"))?;
        let mut added = 0;
        for r in file.repos {
            if self.add(&r.repo, &r.asset_pattern, &r.app_id, r.include_prereleases).is_ok() {
                added += 1;
            }
        }
        Ok(added)
    }
}

#[derive(Serialize, Deserialize)]
struct ExportedRepo {
    repo: String,
    #[serde(default)]
    asset_pattern: String,
    #[serde(default)]
    app_id: String,
    #[serde(default)]
    include_prereleases: bool,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

pub fn config_path() -> PathBuf {
    std::env::var_os("FLOE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg("XDG_CONFIG_HOME", ".config").join("floe").join("config.json"))
}

pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("floe")
}

/// Where bundles are downloaded. A real path on the host even inside the flatpak sandbox
/// (`~/.var/app/<id>/cache/...`), so the host's `gh` and `flatpak` can both use it.
pub fn download_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("floe").join("downloads")
}

/// The user's Downloads folder, for exporting/importing the list.
pub fn user_downloads() -> PathBuf {
    std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join("Downloads"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gh::Asset;

    fn release(tag: &str) -> Release {
        Release { tag: tag.into(), assets: vec![Asset { name: "app.flatpak".into(), size: 10 }], ..Default::default() }
    }

    #[test]
    fn statuses() {
        let mut r = TrackedRepo { repo: "o/app".into(), ..Default::default() };
        assert_eq!(r.status(), Status::Unknown);
        r.last_error = Some("boom".into());
        assert_eq!(r.status(), Status::Error);
        r.latest = Some(release("v1.1.0"));
        assert_eq!(r.status(), Status::NotInstalled);
        r.installed_tag = Some("v1.0.0".into());
        assert_eq!(r.status(), Status::UpdateAvailable);
        r.installed_tag = Some("v1.1.0".into());
        assert_eq!(r.status(), Status::UpToDate);

        // With an app ID, flatpak's version is used; the app must be present.
        r.app_id = "io.github.o.App".into();
        r.installed_tag = None;
        assert_eq!(r.status(), Status::NotInstalled);
        r.app_present = true;
        r.installed_version = Some("1.0.0".into());
        assert_eq!(r.status(), Status::UpdateAvailable);
        r.installed_version = Some("1.1.0".into());
        assert_eq!(r.status(), Status::UpToDate);
        // Metadata lagging behind, but Floe installed this tag: up to date.
        r.installed_version = Some("1.0.0".into());
        r.installed_tag = Some("v1.1.0".into());
        assert_eq!(r.status(), Status::UpToDate);
        // Non-numeric tags: compare by tag.
        r.installed_version = None;
        r.installed_tag = Some("nightly-a".into());
        r.latest = Some(release("nightly-b"));
        assert_eq!(r.status(), Status::UpdateAvailable);
        // Newer release without a bundle.
        r.latest.as_mut().unwrap().assets.clear();
        assert_eq!(r.status(), Status::NoAsset);
    }

    #[test]
    fn add_save_load_export() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/config.json");
        let mut c = Config::default();
        let id = c.add("https://github.com/tobeoffline64/PenguinFin", "", "", false).unwrap();
        assert_eq!(c.repo(id).unwrap().repo, "tobeoffline64/PenguinFin");
        assert!(c.add("tobeoffline64/penguinfin", "*.flatpak", "", false).is_err(), "duplicate");
        assert!(c.add("me/other", "", "not-an-id", false).is_err());
        let id2 = c.add("me/other", "*-x86_64.flatpak", "io.github.me.Other", true).unwrap();
        assert_ne!(id, id2);
        c.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded, c);
        assert_eq!(Config::load(&dir.path().join("missing.json")).unwrap(), Config::default());

        let exported = c.export_list();
        let mut fresh = Config::default();
        assert_eq!(fresh.import_list(&exported).unwrap(), 2);
        assert_eq!(fresh.import_list(&exported).unwrap(), 0);
        assert_eq!(fresh.repos[1].app_id, "io.github.me.Other");
        c.remove(id);
        assert_eq!(c.repos.len(), 1);
    }

    #[test]
    fn ui_scale_steps() {
        assert_eq!(UiScale::Auto.bigger(), UiScale::Large);
        assert_eq!(UiScale::Small.smaller(), UiScale::Small);
        assert_eq!(UiScale::ExtraLarge.bigger(), UiScale::ExtraLarge);
    }
}

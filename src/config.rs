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
    /// Look for a newer Floe release when Floe opens (and with the periodic checks).
    pub check_self: bool,
    /// Offer Floe's own pre-releases too.
    pub self_beta: bool,
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
            check_self: true,
            self_beta: false,
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
    /// Beta channel: also offer pre-releases (betas, release candidates) when they're newer
    /// than the latest stable release. (Field name kept for existing config files.)
    pub include_prereleases: bool,
    /// Which asset to install from a pre-release, when betas name their bundle differently;
    /// empty means the same pattern as stable releases.
    pub beta_asset_pattern: String,
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
    /// The release to install on this repository's channel (stable, or beta when that's on).
    pub latest: Option<Release>,
    /// The newest stable release, and the newest pre-release if it's newer than that.
    pub stable: Option<Release>,
    pub beta: Option<Release>,
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
    /// A beta is installed but the app is on the stable channel (stable is older).
    OnBeta,
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
            Status::OnBeta => "On beta",
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
    /// The asset pattern for a release: the beta pattern for pre-releases, if one is set.
    pub fn pattern_for(&self, release: &Release) -> &str {
        if release.prerelease && !self.beta_asset_pattern.trim().is_empty() {
            self.beta_asset_pattern.trim()
        } else {
            gh::effective_pattern(&self.asset_pattern)
        }
    }
    pub fn latest_asset(&self) -> Option<&gh::Asset> {
        let latest = self.latest.as_ref()?;
        latest.pick_asset(self.pattern_for(latest))
    }
    pub fn beta_channel(&self) -> bool {
        self.include_prereleases
    }
    /// A beta newer than both stable and what's installed, offered while on the stable channel.
    pub fn beta_on_offer(&self) -> Option<&Release> {
        let beta = self.beta.as_ref().filter(|_| !self.include_prereleases)?;
        match self.compare_installed(&beta.tag) {
            Some(std::cmp::Ordering::Less) | None => Some(beta),
            _ => None,
        }
    }
    /// Switch between the stable and beta channel, using the releases from the last check.
    pub fn set_beta_channel(&mut self, on: bool) {
        self.include_prereleases = on;
        self.retarget();
    }
    /// Recompute `latest` from `stable`/`beta` for the current channel.
    pub fn retarget(&mut self) {
        if self.stable.is_some() || self.beta.is_some() {
            let ch = gh::Channels { stable: self.stable.clone(), beta: self.beta.clone() };
            self.latest = ch.pick(self.include_prereleases).cloned();
        }
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
        let meta = self.installed_version.as_deref().filter(|v| !v.is_empty());
        match (meta, self.installed_tag.as_deref()) {
            // An app may write "2.0.0" in its metadata for "v2.0.0-beta.1": when the numbers
            // agree, the tag Floe installed is the more precise of the two.
            (Some(m), Some(t)) if version::same_core(m, t) => Some(t),
            (Some(m), _) => Some(m),
            (None, t) => t,
        }
    }

    /// The installed version compared with a tag (`Less` = the tag is newer).
    fn compare_installed(&self, tag: &str) -> Option<std::cmp::Ordering> {
        if self.installed_tag.as_deref() == Some(tag) {
            return Some(std::cmp::Ordering::Equal);
        }
        version::compare(self.current_version()?, tag)
    }

    /// Installed a pre-release that's newer than the stable release, while on the stable channel.
    pub fn on_beta(&self) -> bool {
        !self.include_prereleases
            && self.is_installed()
            && self.current_version().is_some_and(version::is_prerelease)
            && self.stable.as_ref().is_some_and(|s| self.compare_installed(&s.tag) == Some(std::cmp::Ordering::Greater))
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
        if self.on_beta() {
            return Status::OnBeta;
        }
        let behind = match self.compare_installed(&latest.tag) {
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

    fn pre(tag: &str, asset: &str) -> Release {
        Release { tag: tag.into(), prerelease: true, assets: vec![Asset { name: asset.into(), size: 10 }], ..Default::default() }
    }

    /// PenguinFin's releases: stable v1.10.0 (penguinfin.flatpak) and beta v2.0.0-beta.1
    /// (penguinfin-2.flatpak), same app ID.
    #[test]
    fn beta_channel() {
        let mut r = TrackedRepo {
            repo: "tobeoffline64/PenguinFin".into(),
            app_id: "io.github.tobeoffline64.PenguinFin".into(),
            app_present: true,
            installed_version: Some("1.10.0".into()),
            stable: Some(Release { tag: "v1.10.0".into(), assets: vec![Asset { name: "penguinfin.flatpak".into(), size: 1 }], ..Default::default() }),
            beta: Some(pre("v2.0.0-beta.1", "penguinfin-2.flatpak")),
            ..Default::default()
        };
        r.retarget();
        // Stable channel: up to date, with the beta on offer.
        assert_eq!(r.latest.as_ref().unwrap().tag, "v1.10.0");
        assert_eq!(r.status(), Status::UpToDate);
        assert_eq!(r.beta_on_offer().unwrap().tag, "v2.0.0-beta.1");

        // Switch to beta: the beta is the update.
        r.set_beta_channel(true);
        assert_eq!(r.status(), Status::UpdateAvailable);
        assert_eq!(r.latest_asset().unwrap().name, "penguinfin-2.flatpak");
        assert!(r.beta_on_offer().is_none());

        // Installed the beta (metadata says "2.0.0~beta.1"): up to date on beta.
        r.installed_tag = Some("v2.0.0-beta.1".into());
        r.installed_version = Some("2.0.0~beta.1".into());
        assert_eq!(r.status(), Status::UpToDate);
        // Beta 2 arrives.
        r.beta = Some(pre("v2.0.0-beta.2", "penguinfin-2.flatpak"));
        r.retarget();
        assert_eq!(r.status(), Status::UpdateAvailable);
        // Metadata without the beta suffix still compares by the installed tag.
        r.installed_version = Some("2.0.0".into());
        assert_eq!(r.status(), Status::UpdateAvailable);
        r.installed_version = Some("2.0.0~beta.1".into());

        // Back to stable while the beta is installed: "On beta", not "up to date".
        r.set_beta_channel(false);
        assert_eq!(r.status(), Status::OnBeta);
        // A newer beta is still mentioned (not installed automatically) on the stable channel.
        assert_eq!(r.beta_on_offer().unwrap().tag, "v2.0.0-beta.2");

        // 2.0.0 ships: betas drop out, stable channel offers the update.
        r.stable = Some(Release { tag: "v2.0.0".into(), assets: vec![Asset { name: "penguinfin.flatpak".into(), size: 1 }], ..Default::default() });
        r.beta = None;
        r.retarget();
        assert_eq!(r.status(), Status::UpdateAvailable);

        // A beta pattern only applies to pre-releases.
        r.asset_pattern = "penguinfin.flatpak".into();
        r.beta_asset_pattern = "penguinfin-2*.flatpak".into();
        assert_eq!(r.pattern_for(r.stable.as_ref().unwrap()), "penguinfin.flatpak");
        assert_eq!(r.pattern_for(&pre("v2.1.0-beta.1", "x")), "penguinfin-2*.flatpak");
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

//! GitHub releases through the GitHub CLI (`gh`). Using `gh` means private repositories
//! and your existing `gh auth login` just work, and Floe never handles a token itself.

use crate::host;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
}

/// A published release, as returned by `gh release view --json ...`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Release {
    #[serde(rename = "tagName")]
    pub tag: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "publishedAt", default)]
    pub published_at: String,
    #[serde(rename = "isPrerelease", default)]
    pub prerelease: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    /// The publish date as YYYY-MM-DD.
    pub fn date(&self) -> &str {
        self.published_at.get(..10).unwrap_or(&self.published_at)
    }

    /// The asset to install: the first match for `pattern`, preferring one built for this
    /// machine's architecture when several match.
    pub fn pick_asset(&self, pattern: &str) -> Option<&Asset> {
        let pattern = effective_pattern(pattern);
        let matches: Vec<&Asset> = self.assets.iter().filter(|a| glob_match(pattern, &a.name)).collect();
        let arch = std::env::consts::ARCH;
        matches.iter().find(|a| a.name.contains(arch)).or(matches.first()).copied()
    }
}

pub const DEFAULT_PATTERN: &str = "*.flatpak";

pub fn effective_pattern(p: &str) -> &str {
    if p.trim().is_empty() { DEFAULT_PATTERN } else { p.trim() }
}

/// Case-insensitive glob with `*` and `?`.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Accepts "owner/name", "https://github.com/owner/name(.git)(/...)", "github.com/owner/name"
/// or "git@github.com:owner/name.git" and returns "owner/name".
pub fn parse_repo(input: &str) -> Result<String, String> {
    let mut s = input.trim();
    for prefix in ["https://", "http://", "ssh://", "git@", "www."] {
        s = s.strip_prefix(prefix).unwrap_or(s);
    }
    for prefix in ["github.com/", "github.com:", "git@github.com/"] {
        s = s.strip_prefix(prefix).unwrap_or(s);
    }
    let mut parts = s.split('/').filter(|p| !p.is_empty());
    let (Some(owner), Some(name)) = (parts.next(), parts.next()) else {
        return Err("Enter a repository as owner/name or a GitHub link.".into());
    };
    let name = name.strip_suffix(".git").unwrap_or(name);
    let valid = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if !valid(owner) || !valid(name) {
        return Err(format!("\"{owner}/{name}\" doesn't look like a GitHub repository."));
    }
    Ok(format!("{owner}/{name}"))
}

/// What `gh` reports about itself and the login.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub installed: bool,
    /// e.g. "2.62.0"
    pub version: String,
    pub logged_in: bool,
    /// The github.com account, when it could be read from `gh auth status`.
    pub account: String,
    /// Raw output (or the error) for the details view.
    pub details: String,
}

pub fn status() -> Status {
    let mut st = Status::default();
    match host::run("gh", &["--version"]) {
        Err(e) => {
            st.details = e;
            return st;
        }
        Ok(o) => {
            st.installed = o.ok;
            st.version = o.stdout.split_whitespace().nth(2).unwrap_or("").to_string();
        }
    }
    match host::run("gh", &["auth", "status", "--hostname", "github.com"]) {
        Err(e) => st.details = e,
        Ok(o) => {
            st.logged_in = o.ok;
            st.details = o.text();
            st.account = parse_account(&st.details).unwrap_or_default();
        }
    }
    st
}

/// "Logged in to github.com account octocat (keyring)" / older "Logged in to github.com as octocat".
fn parse_account(text: &str) -> Option<String> {
    let line = text.lines().find(|l| l.contains("Logged in to"))?;
    let rest = line.split(" account ").nth(1).or_else(|| line.split(" as ").nth(1))?;
    Some(rest.split_whitespace().next()?.trim_matches(|c| c == '(' || c == ')').to_string())
}

fn gh(args: &[&str]) -> Result<host::Output, String> {
    let out = host::run("gh", args)?;
    if out.ok { Ok(out) } else { Err(friendly_error(&out)) }
}

/// Turn common `gh` failures into something actionable.
fn friendly_error(out: &host::Output) -> String {
    let text = out.text();
    let lower = text.to_lowercase();
    if lower.contains("gh auth login") || lower.contains("not logged") || lower.contains("authentication") {
        "GitHub CLI isn't signed in. Run `gh auth login` in a terminal.".into()
    } else if lower.contains("could not resolve to a repository") || lower.contains("http 404") {
        "Repository not found (or your GitHub account can't see it).".into()
    } else if lower.contains("release not found") {
        "No release found.".into()
    } else if lower.contains("rate limit") {
        "GitHub rate limit reached; try again later.".into()
    } else {
        out.summary()
    }
}

const RELEASE_FIELDS: &str = "tagName,name,publishedAt,isPrerelease,url,body,assets";

/// The newest published (non-draft) release, optionally including pre-releases.
pub fn latest_release(repo: &str, include_prereleases: bool) -> Result<Release, String> {
    #[derive(Deserialize)]
    struct Item {
        #[serde(rename = "tagName")]
        tag: String,
    }
    let mut args = vec!["release", "list", "--repo", repo, "--limit", "1", "--exclude-drafts", "--json", "tagName"];
    if !include_prereleases {
        args.push("--exclude-pre-releases");
    }
    let out = gh(&args)?;
    let items: Vec<Item> =
        serde_json::from_str(out.stdout.trim()).map_err(|e| format!("Unexpected output from gh release list: {e}"))?;
    let Some(item) = items.into_iter().next() else {
        return Err(if include_prereleases {
            "This repository has no releases yet.".into()
        } else {
            "No stable releases yet (turn on pre-releases to include them).".into()
        });
    };
    release(repo, &item.tag)
}

pub fn release(repo: &str, tag: &str) -> Result<Release, String> {
    let out = gh(&["release", "view", tag, "--repo", repo, "--json", RELEASE_FIELDS])?;
    serde_json::from_str(out.stdout.trim()).map_err(|e| format!("Unexpected output from gh release view: {e}"))
}

/// `gh` treats `--pattern` as a glob; match this one asset name literally.
fn glob_escape(name: &str) -> String {
    name.chars()
        .map(|c| if matches!(c, '*' | '?' | '[' | ']' | '\\') { format!("[{c}]") } else { c.to_string() })
        .collect()
}

/// Download one release asset into `dir` and return the file's path.
pub fn download(repo: &str, tag: &str, asset: &str, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let dir_s = dir.to_string_lossy().into_owned();
    let pattern = glob_escape(asset);
    gh(&["release", "download", tag, "--repo", repo, "--pattern", &pattern, "--dir", &dir_s, "--clobber"])?;
    let path = dir.join(asset);
    if path.is_file() { Ok(path) } else { Err(format!("gh finished but {} is missing", path.display())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repos() {
        for input in [
            "tobeoffline64/PenguinFin",
            " https://github.com/tobeoffline64/PenguinFin ",
            "https://github.com/tobeoffline64/PenguinFin.git",
            "https://github.com/tobeoffline64/PenguinFin/releases/latest",
            "github.com/tobeoffline64/PenguinFin",
            "git@github.com:tobeoffline64/PenguinFin.git",
        ] {
            assert_eq!(parse_repo(input).as_deref(), Ok("tobeoffline64/PenguinFin"), "{input}");
        }
        assert!(parse_repo("PenguinFin").is_err());
        assert!(parse_repo("a b/c").is_err());
    }

    #[test]
    fn globs() {
        assert!(glob_match("*.flatpak", "penguinfin.flatpak"));
        assert!(glob_match("*.FLATPAK", "PenguinFin.flatpak"));
        assert!(glob_match("app-*-x86_64.flatpak", "app-1.2-x86_64.flatpak"));
        assert!(glob_match("app?.flatpak", "app1.flatpak"));
        assert!(!glob_match("*.flatpak", "penguinfin.flatpak.sha256"));
        assert!(!glob_match("*.flatpak", "source.tar.gz"));
    }

    #[test]
    fn picks_asset_for_arch() {
        let arch = std::env::consts::ARCH;
        let r = Release {
            assets: vec![
                Asset { name: "src.tar.gz".into(), size: 1 },
                Asset { name: "app-other.flatpak".into(), size: 2 },
                Asset { name: format!("app-{arch}.flatpak"), size: 3 },
            ],
            ..Default::default()
        };
        assert_eq!(r.pick_asset("").map(|a| a.size), Some(3));
        assert_eq!(r.pick_asset("*other*").map(|a| a.size), Some(2));
        assert!(r.pick_asset("*.deb").is_none());
    }

    #[test]
    fn parses_release_json() {
        let json = r#"{"assets":[{"apiUrl":"x","name":"penguinfin.flatpak","size":123,"downloadCount":4}],
            "body":"Notes","isPrerelease":false,"name":"PenguinFin 1.3.0",
            "publishedAt":"2026-11-15T20:00:00Z","tagName":"v1.3.0","url":"https://github.com/o/r/releases/tag/v1.3.0"}"#;
        let r: Release = serde_json::from_str(json).unwrap();
        assert_eq!(r.tag, "v1.3.0");
        assert_eq!(r.date(), "2026-11-15");
        assert_eq!(r.assets[0].size, 123);
    }

    #[test]
    fn reads_account() {
        let new = "github.com\n  ✓ Logged in to github.com account octocat (keyring)\n  - Active account: true";
        assert_eq!(parse_account(new).as_deref(), Some("octocat"));
        let old = "github.com\n  ✓ Logged in to github.com as octo-cat (/home/u/.config/gh/hosts.yml)";
        assert_eq!(parse_account(old).as_deref(), Some("octo-cat"));
    }

    #[test]
    fn escapes_globs() {
        assert_eq!(glob_escape("a*b[1].flatpak"), "a[*]b[[]1[]].flatpak");
    }
}

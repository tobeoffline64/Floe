//! End-to-end check → download → install → update, against fake `gh` and `flatpak` scripts
//! in tests/fake (selected with FLOE_GH / FLOE_FLATPAK).

use floe::config::{Config, Settings, Status};
use floe::updater;
use std::path::Path;

fn setup(dir: &Path) {
    let fake = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake");
    // SAFETY: the only test in this binary; set before any threads start.
    unsafe {
        std::env::set_var("FLOE_GH", fake.join("gh"));
        std::env::set_var("FLOE_FLATPAK", fake.join("flatpak"));
        std::env::set_var("FAKE_DIR", dir);
        std::env::set_var("XDG_CACHE_HOME", dir.join("cache"));
    }
}

#[test]
fn check_install_and_update() {
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    std::fs::write(dir.path().join("tag"), "v1.1.0").unwrap();

    let st = floe::gh::status();
    assert!(st.installed && st.logged_in, "{st:?}");
    assert_eq!(st.account, "tester");
    assert_eq!(st.version, "2.62.0");

    let mut cfg = Config::default();
    let id = cfg.add("https://github.com/o/demo", "", "", false).unwrap();
    let missing = cfg.add("missing/repo", "", "", false).unwrap();

    // Check: latest release found, nothing installed yet.
    let c = updater::check(cfg.repo(id).unwrap());
    updater::apply_check(cfg.repo_mut(id).unwrap(), c);
    let r = cfg.repo(id).unwrap();
    assert_eq!(r.latest.as_ref().unwrap().tag, "v1.1.0");
    assert_eq!(r.latest_asset().unwrap().name, "demo.flatpak");
    assert_eq!(r.status(), Status::NotInstalled);

    let c = updater::check(cfg.repo(missing).unwrap());
    updater::apply_check(cfg.repo_mut(missing).unwrap(), c);
    assert_eq!(cfg.repo(missing).unwrap().status(), Status::Error);
    assert!(cfg.repo(missing).unwrap().last_error.as_ref().unwrap().contains("not found"));

    // Install: downloads via gh, installs, detects the app ID, deletes the bundle.
    let settings = Settings::default();
    let (release, bundle) = updater::download(cfg.repo(id).unwrap()).unwrap();
    assert!(bundle.is_file());
    let done = updater::install(cfg.repo(id).unwrap(), &bundle, &settings).unwrap();
    assert!(!bundle.exists(), "bundle removed after install");
    updater::apply_install(cfg.repo_mut(id).unwrap(), &release, done);
    let r = cfg.repo(id).unwrap();
    assert_eq!(r.app_id, "io.github.o.Demo");
    assert_eq!(r.installed_version.as_deref(), Some("1.1.0"));
    assert_eq!(r.status(), Status::UpToDate);

    // A new release appears: update available, then installed.
    std::fs::write(dir.path().join("tag"), "v1.2.0").unwrap();
    let c = updater::check(cfg.repo(id).unwrap());
    updater::apply_check(cfg.repo_mut(id).unwrap(), c);
    assert_eq!(cfg.repo(id).unwrap().status(), Status::UpdateAvailable);
    assert_eq!(cfg.updates(), 1);
    let (release, bundle) = updater::download(cfg.repo(id).unwrap()).unwrap();
    let done = updater::install(cfg.repo(id).unwrap(), &bundle, &settings).unwrap();
    updater::apply_install(cfg.repo_mut(id).unwrap(), &release, done);
    assert_eq!(cfg.repo(id).unwrap().installed_version.as_deref(), Some("1.2.0"));
    assert_eq!(cfg.repo(id).unwrap().status(), Status::UpToDate);

    // Uninstalled outside Floe: the next check offers a fresh install.
    std::fs::remove_file(dir.path().join("installed")).unwrap();
    let c = updater::check(cfg.repo(id).unwrap());
    updater::apply_check(cfg.repo_mut(id).unwrap(), c);
    assert_eq!(cfg.repo(id).unwrap().status(), Status::NotInstalled);

    let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
    assert!(calls.contains("gh release download v1.2.0 --repo o/demo --pattern demo.flatpak"), "{calls}");
    assert!(calls.contains("flatpak install --user -y --noninteractive --reinstall"), "{calls}");
}

//! End-to-end check → download → install → update, against fake `gh` and `flatpak` scripts
//! in tests/fake (selected with FLOE_GH / FLOE_FLATPAK).

use floe::config::{Config, Settings, Status};
use floe::console::Handle;
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
    let (release, bundle) = updater::download(cfg.repo(id).unwrap(), &Handle::detached()).unwrap();
    assert!(bundle.is_file());
    let done = updater::install(cfg.repo(id).unwrap(), &bundle, &settings, &Handle::detached()).unwrap();
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
    let (release, bundle) = updater::download(cfg.repo(id).unwrap(), &Handle::detached()).unwrap();
    let done = updater::install(cfg.repo(id).unwrap(), &bundle, &settings, &Handle::detached()).unwrap();
    updater::apply_install(cfg.repo_mut(id).unwrap(), &release, done);
    assert_eq!(cfg.repo(id).unwrap().installed_version.as_deref(), Some("1.2.0"));
    assert_eq!(cfg.repo(id).unwrap().status(), Status::UpToDate);

    // Uninstalled outside Floe: the next check offers a fresh install.
    std::fs::remove_file(dir.path().join("installed")).unwrap();
    let c = updater::check(cfg.repo(id).unwrap());
    updater::apply_check(cfg.repo_mut(id).unwrap(), c);
    assert_eq!(cfg.repo(id).unwrap().status(), Status::NotInstalled);

    // Already installed before it's added: linked by app ID instead of offering an install.
    std::fs::write(dir.path().join("installed"), "1.2.0").unwrap();
    std::fs::write(dir.path().join("extra_apps"), "io.github.someone.Tool\tTool\t0.9\ttool-origin\norg.gnome.Calculator\tCalculator\t47\tflathub\n").unwrap();
    let mut cfg2 = Config::default();
    let a = cfg2.add("o/demo", "", "", false).unwrap();
    let b = cfg2.add("someone/tool", "", "", false).unwrap();
    let c3 = cfg2.add("gnome/calculator", "", "", false).unwrap();
    for rid in [a, b, c3] {
        let c = updater::check(cfg2.repo(rid).unwrap());
        updater::apply_check(cfg2.repo_mut(rid).unwrap(), c);
    }
    let r = cfg2.repo(a).unwrap();
    assert_eq!((r.app_id.as_str(), r.installed_version.as_deref()), ("io.github.o.Demo", Some("1.2.0")));
    assert_eq!(r.status(), Status::UpToDate);
    let r = cfg2.repo(b).unwrap();
    assert_eq!(r.app_id, "io.github.someone.Tool");
    assert_eq!(r.status(), Status::UpdateAvailable, "0.9 installed, v1.2.0 released");
    // Same name but from Flathub: only suggested.
    let r = cfg2.repo(c3).unwrap();
    assert_eq!(r.app_id, "");
    assert_eq!(r.suggested_app_id.as_deref(), Some("org.gnome.Calculator"));
    assert_eq!(r.status(), Status::NotInstalled);

    // Live output and cancel: a slow install streams its progress and stops when cancelled.
    std::fs::write(dir.path().join("slow"), "20").unwrap();
    let job = floe::console::start(42);
    let (r, b2) = (cfg.repo(id).unwrap().clone(), dir.path().join("cache/x.flatpak"));
    std::fs::create_dir_all(b2.parent().unwrap()).unwrap();
    std::fs::write(&b2, "bundle").unwrap();
    let j = job.clone();
    let worker = std::thread::spawn(move || updater::install(&r, &b2, &Settings::default(), &j));
    std::thread::sleep(std::time::Duration::from_millis(2500));
    let lines = floe::console::snapshot(42).unwrap().lines;
    assert!(lines[0].starts_with("Installing x.flatpak"), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("$ flatpak install --user")), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("Required runtime")), "{lines:?}");
    let progress: Vec<_> = lines.iter().filter(|l| l.starts_with("Installing 1/2")).collect();
    assert_eq!(progress.len(), 1, "a \\r progress bar redraws one line: {lines:?}");
    let started = std::time::Instant::now();
    floe::console::cancel(42);
    let res = worker.join().unwrap();
    assert_eq!(res.err().as_deref(), Some("Cancelled."));
    assert!(started.elapsed().as_secs() < 3, "cancel stops the process promptly");

    // Beta channel, modelled on PenguinFin: stable v1.10.0, beta v2.0.0-beta.1 with its own
    // bundle name, same app ID.
    std::fs::write(dir.path().join("slow"), "0").unwrap();
    std::fs::write(dir.path().join("tag"), "v1.10.0").unwrap();
    std::fs::write(dir.path().join("installed"), "1.10.0").unwrap();
    std::fs::write(dir.path().join("beta"), "v2.0.0-beta.1").unwrap();
    let mut cfg3 = Config::default();
    let p = cfg3.add("o/demo", "", "io.github.o.Demo", false).unwrap();
    let c = updater::check(cfg3.repo(p).unwrap());
    updater::apply_check(cfg3.repo_mut(p).unwrap(), c);
    let r = cfg3.repo(p).unwrap();
    assert_eq!(r.status(), Status::UpToDate, "stable channel ignores the beta");
    assert_eq!(r.beta_on_offer().map(|b| b.tag.as_str()), Some("v2.0.0-beta.1"));
    // Switch to beta and install it.
    cfg3.repo_mut(p).unwrap().set_beta_channel(true);
    assert_eq!(cfg3.repo(p).unwrap().status(), Status::UpdateAvailable);
    let (release, bundle) = updater::download(cfg3.repo(p).unwrap(), &Handle::detached()).unwrap();
    assert_eq!(bundle.file_name().unwrap(), "demo-2.flatpak");
    let done = updater::install(cfg3.repo(p).unwrap(), &bundle, &settings, &Handle::detached()).unwrap();
    updater::apply_install(cfg3.repo_mut(p).unwrap(), &release, done);
    assert_eq!(cfg3.repo(p).unwrap().installed_version.as_deref(), Some("2.0.0-beta.1"));
    assert_eq!(cfg3.repo(p).unwrap().status(), Status::UpToDate);
    // Back to stable: "On beta"; returning installs the stable bundle again.
    cfg3.repo_mut(p).unwrap().set_beta_channel(false);
    assert_eq!(cfg3.repo(p).unwrap().status(), Status::OnBeta);
    let (release, bundle) = updater::download(cfg3.repo(p).unwrap(), &Handle::detached()).unwrap();
    assert_eq!((release.tag.as_str(), bundle.file_name().unwrap().to_str().unwrap()), ("v1.10.0", "demo.flatpak"));
    std::fs::write(dir.path().join("beta"), "").unwrap();

    // System-wide installs: no --noninteractive, so polkit can ask for the password.
    std::fs::write(dir.path().join("beta"), "").unwrap();
    let system = Settings { scope: floe::flatpak::Scope::System, ..Settings::default() };
    let (_, bundle) = updater::download(cfg3.repo(p).unwrap(), &Handle::detached()).unwrap();
    updater::install(cfg3.repo(p).unwrap(), &bundle, &system, &Handle::detached()).unwrap();
    // Password refused or no prompt: a clear message, and the file is kept for a manual retry.
    std::fs::write(dir.path().join("deny"), "").unwrap();
    let (_, bundle) = updater::download(cfg3.repo(p).unwrap(), &Handle::detached()).unwrap();
    let err = updater::install(cfg3.repo(p).unwrap(), &bundle, &system, &Handle::detached()).unwrap_err();
    assert!(err.contains("administrator password") && err.contains("flatpak install --system --reinstall"), "{err}");
    assert!(bundle.is_file(), "bundle kept after a failed install");
    std::fs::remove_file(dir.path().join("deny")).unwrap();

    let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
    assert!(calls.contains("gh release download v1.2.0 --repo o/demo --pattern demo.flatpak"), "{calls}");
    assert!(calls.contains("flatpak install --user -y --noninteractive --reinstall"), "{calls}");
    assert!(calls.contains("flatpak install --system -y --reinstall"), "{calls}");
    assert!(!calls.contains("--system -y --noninteractive"), "{calls}");
}

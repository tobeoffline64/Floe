use crate::views;
use crate::widgets::{Icon, IconName};
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use floe::config::{Config, Status, UiScale};
use floe::flatpak::InstalledApp;
use floe::{console, gh, host, updater};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Apps,
    Installed,
    Activity,
    Settings,
}

#[derive(Clone, PartialEq)]
pub struct Flash {
    pub text: String,
    pub error: bool,
}

/// What's currently happening to a repository.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Busy {
    Checking,
    Downloading,
    Installing,
}

impl Busy {
    pub fn label(self) -> &'static str {
        match self {
            Busy::Checking => "Checking…",
            Busy::Downloading => "Downloading…",
            Busy::Installing => "Installing…",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct LogEntry {
    pub time: String,
    pub repo: String,
    pub text: String,
    pub error: bool,
    /// Full command output, shown on demand.
    pub details: String,
}

/// Shared app state.
#[derive(Clone, Copy)]
pub struct Store {
    pub cfg: Signal<Config>,
    path: Signal<PathBuf>,
    pub busy: Signal<HashMap<u64, Busy>>,
    pub log: Signal<Vec<LogEntry>>,
    pub flash: Signal<Option<Flash>>,
    /// `gh` installation/login, refreshed at launch and from Settings.
    pub gh: Signal<Option<gh::Status>>,
    /// True while "Update all" works through the list.
    pub updating_all: Signal<bool>,
    /// Installed flatpaks, loaded by the Installed page.
    pub installed: Signal<Option<Vec<InstalledApp>>>,
}

impl Store {
    /// Change the config and save it.
    pub fn update(&mut self, f: impl FnOnce(&mut Config)) {
        let res = {
            let mut cfg = self.cfg.write();
            f(&mut cfg);
            cfg.save(&self.path.peek())
        };
        if let Err(e) = res {
            self.notify(format!("Could not save your list: {e}"), true);
        }
    }

    pub fn config_path(&self) -> String {
        self.path.read().display().to_string()
    }

    pub fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.flash.set(Some(Flash { text: text.into(), error }));
    }

    pub fn log(&mut self, repo: &str, text: impl Into<String>, error: bool, details: impl Into<String>) {
        let mut log = self.log.write();
        log.insert(
            0,
            LogEntry {
                time: chrono::Local::now().format("%H:%M:%S").to_string(),
                repo: repo.to_string(),
                text: text.into(),
                error,
                details: details.into(),
            },
        );
        log.truncate(300);
    }

    pub fn busy(&self, id: u64) -> Option<Busy> {
        self.busy.read().get(&id).copied()
    }

    fn set_busy(&mut self, id: u64, b: Option<Busy>) {
        let mut m = self.busy.write();
        match b {
            Some(b) => m.insert(id, b),
            None => m.remove(&id),
        };
    }
}

pub fn use_store() -> Store {
    use_context::<Store>()
}

type Boot = Result<(PathBuf, Config), String>;

/// The config loaded in `main` before the window exists, handed to `App` once.
static BOOT: Mutex<Option<Boot>> = Mutex::new(None);

pub fn set_boot(boot: Boot) {
    *BOOT.lock().expect("boot lock") = Some(boot);
}

#[component]
pub fn App() -> Element {
    let opened: Result<Store, String> = use_hook(|| {
        let (path, cfg) = BOOT.lock().expect("boot lock").take().unwrap_or_else(|| Err("config was not loaded".into()))?;
        let store = Store {
            cfg: Signal::new(cfg),
            path: Signal::new(path),
            busy: Signal::new(HashMap::new()),
            log: Signal::new(Vec::new()),
            flash: Signal::new(None),
            gh: Signal::new(None),
            updating_all: Signal::new(false),
            installed: Signal::new(None),
        };
        provide_context(store);
        Ok(store)
    });

    match opened {
        Ok(_) => rsx! { Shell {} },
        Err(e) => rsx! {
            div { class: "p-10",
                h1 { class: "text-xl font-semibold", "Floe could not read its settings" }
                pre { class: "mt-4 text-sm text-destructive whitespace-pre-wrap", "{e}" }
                p { class: "mt-4 text-sm text-muted-foreground", "Fix or move the file, then start Floe again." }
            }
        },
    }
}

/// Browser-side helpers: removes the boot skeleton, and Ctrl + / Ctrl - / Ctrl 0 send
/// "bigger" / "smaller" / "auto" to Rust.
const BRIDGE_JS: &str = r#"
document.getElementById('boot')?.remove();
if (!window.__flBridge) {
  window.__flBridge = true;
  window.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
    let cmd = null;
    if (e.key === '=' || e.key === '+') cmd = 'bigger';
    else if (e.key === '-' || e.key === '_') cmd = 'smaller';
    else if (e.key === '0') cmd = 'auto';
    else if (e.key === 'r' || e.key === 'R') cmd = 'check';
    if (cmd) { e.preventDefault(); dioxus.send(cmd); }
  });
}
"#;

#[component]
fn Shell() -> Element {
    let mut store = use_store();
    let mut page = use_signal(|| Page::Apps);
    let settings = store.cfg.read().settings.clone();
    let dark = settings.dark_mode;
    let scale = settings.ui_scale;
    let updates = store.cfg.read().updates();
    let errors = store.log.read().iter().take_while(|e| e.error).count();

    let mut flash = store.flash;
    use_effect(move || {
        if flash.read().is_some() {
            spawn(async move {
                sleep_ms(4500).await;
                flash.set(None);
            });
        }
    });

    use_hook(move || {
        spawn(async move {
            let mut bridge = document::eval(BRIDGE_JS);
            while let Ok(cmd) = bridge.recv::<String>().await {
                let current = store.cfg.read().settings.ui_scale;
                match cmd.as_str() {
                    "bigger" => set_ui_scale(&mut store, current.bigger()),
                    "smaller" => set_ui_scale(&mut store, current.smaller()),
                    "check" => check_all(store, true),
                    _ => set_ui_scale(&mut store, UiScale::Auto),
                }
            }
        });
    });

    // At launch: read the gh status, then (optionally) check everything. While open, check
    // again every N hours.
    use_hook(move || {
        spawn_forever(async move {
            refresh_gh_status(store).await;
            let s = store.cfg.peek().settings.clone();
            if s.check_on_launch && !store.cfg.peek().repos.is_empty() {
                sleep_ms(600).await;
                check_all_and_notify(store).await;
            }
            let mut last = std::time::Instant::now();
            loop {
                sleep_ms(60_000).await;
                let hours = store.cfg.peek().settings.check_interval_hours;
                if hours > 0 && last.elapsed().as_secs() >= u64::from(hours) * 3600 {
                    last = std::time::Instant::now();
                    check_all_and_notify(store).await;
                }
            }
        });
    });

    use_effect(move || {
        let s = store.cfg.read().settings.clone();
        match_gtk_theme(s.dark_mode);
        document::eval(&format!("document.documentElement.classList.toggle('dark', {});", s.dark_mode));
    });

    let nav = [
        (Page::Apps, "Apps", IconName::Package, updates),
        (Page::Installed, "Installed", IconName::Disk, 0),
        (Page::Activity, "Activity", IconName::Activity, errors),
        (Page::Settings, "Settings", IconName::Settings, 0),
    ];

    rsx! {
        style { "html {{ font-size: {scale.root_font_size()}; }}" }
        div { class: if dark { "dark h-full" } else { "h-full" },
            div { class: "@container/app flex h-screen bg-background text-foreground",
                nav { class: "flex w-14 @4xl/app:w-56 shrink-0 flex-col gap-1 overflow-y-auto border-r bg-sidenav p-2 @4xl/app:p-3",
                    div { class: "flex items-center gap-2 px-1 pb-4 pt-2 @4xl/app:px-3",
                        img { class: "size-8 shrink-0", src: LOGO_DATA_URI, alt: "Floe" }
                        div { class: "hidden @4xl/app:block",
                            div { class: "text-lg font-bold leading-tight tracking-tight", "Floe" }
                            div { class: "text-xs text-muted-foreground", "Flatpaks from GitHub" }
                        }
                    }
                    for (p, label, icon, count) in nav {
                        button {
                            title: label,
                            class: if page() == p {
                                "relative flex items-center justify-center @4xl/app:justify-start gap-3 rounded-md px-2 py-2 @4xl/app:px-3 text-left text-sm font-medium bg-sidenav-accent text-sidenav-accent-foreground"
                            } else {
                                "relative flex items-center justify-center @4xl/app:justify-start gap-3 rounded-md px-2 py-2 @4xl/app:px-3 text-left text-sm text-muted-foreground hover:bg-sidenav-accent hover:text-sidenav-accent-foreground cursor-pointer"
                            },
                            onclick: move |_| page.set(p),
                            Icon { name: icon, class: "size-4 shrink-0" }
                            span { class: "hidden @4xl/app:inline flex-1 truncate", "{label}" }
                            if count > 0 {
                                span {
                                    class: if p == Page::Activity {
                                        "absolute right-0.5 top-0.5 @4xl/app:static rounded-full bg-destructive px-1.5 text-[0.625rem] font-semibold leading-4 text-white"
                                    } else {
                                        "absolute right-0.5 top-0.5 @4xl/app:static rounded-full bg-info px-1.5 text-[0.625rem] font-semibold leading-4 text-info-foreground"
                                    },
                                    "{count}"
                                }
                            }
                        }
                    }
                    div { class: "mt-auto flex flex-col gap-2 pt-4",
                        UiSizeControl {}
                        GhIndicator { on_open: move |_| page.set(Page::Settings) }
                    }
                }
                for p in [page()] {
                main { key: "{p:?}", class: "@container min-w-0 flex-1 overflow-y-auto",
                    div { class: "mx-auto flex max-w-5xl flex-col gap-6 p-4 @2xl:p-6 @3xl:p-8",
                        match p {
                            Page::Apps => rsx! { views::apps::AppsPage { on_settings: move |_| page.set(Page::Settings) } },
                            Page::Installed => rsx! { views::installed::InstalledPage { on_apps: move |_| page.set(Page::Apps) } },
                            Page::Activity => rsx! { views::activity::ActivityPage {} },
                            Page::Settings => rsx! { views::settings::SettingsPage {} },
                        }
                    }
                }
                }
            }
            if let Some(f) = flash() {
                div {
                    class: if f.error {
                        "fixed bottom-6 right-6 z-50 max-w-md whitespace-pre-line rounded-lg border border-destructive bg-card px-4 py-3 text-sm text-destructive shadow-lg cursor-pointer"
                    } else {
                        "fixed bottom-6 right-6 z-50 max-w-md whitespace-pre-line rounded-lg border bg-card px-4 py-3 text-sm text-card-foreground shadow-lg cursor-pointer"
                    },
                    onclick: move |_| flash.set(None),
                    "{f.text}"
                }
            }
        }
    }
}

/// Sidebar footer: whether `gh` is ready.
#[component]
fn GhIndicator(on_open: EventHandler<()>) -> Element {
    let store = use_store();
    let (dot, text) = match &*store.gh.read() {
        None => ("bg-muted-foreground", "Checking gh…".to_string()),
        Some(s) if !s.installed => ("bg-destructive", "gh not installed".to_string()),
        Some(s) if !s.logged_in => ("bg-warning", "gh not signed in".to_string()),
        Some(s) if s.account.is_empty() => ("bg-success", "gh signed in".to_string()),
        Some(s) => ("bg-success", format!("gh: {}", s.account)),
    };
    rsx! {
        button {
            class: "flex items-center justify-center @4xl/app:justify-start gap-2 rounded-md px-2 py-1.5 @4xl/app:px-3 text-xs text-muted-foreground hover:bg-sidenav-accent cursor-pointer",
            title: "{text}",
            onclick: move |_| on_open.call(()),
            span { class: "size-2 shrink-0 rounded-full {dot}" }
            span { class: "hidden @4xl/app:inline truncate", "{text}" }
        }
    }
}

pub fn set_ui_scale(store: &mut Store, scale: UiScale) {
    if store.cfg.read().settings.ui_scale != scale {
        store.update(|c| c.settings.ui_scale = scale);
    }
}

#[component]
fn UiSizeControl() -> Element {
    let mut store = use_store();
    let scale = store.cfg.read().settings.ui_scale;
    let btn = "flex size-7 items-center justify-center rounded-md border bg-background text-sm font-medium hover:bg-accent cursor-pointer disabled:opacity-40 disabled:cursor-default";
    rsx! {
        div { class: "flex flex-col items-center gap-1 @4xl/app:flex-row @4xl/app:justify-between @4xl/app:px-3",
            span { class: "hidden text-xs text-muted-foreground @4xl/app:inline", "Text size" }
            div { class: "flex flex-col items-center gap-1 @4xl/app:flex-row",
                button { class: btn, title: "Bigger (Ctrl +)", aria_label: "Bigger text",
                    disabled: scale == UiScale::ExtraLarge,
                    onclick: move |_| set_ui_scale(&mut store, scale.bigger()), "+" }
                button {
                    class: if scale == UiScale::Auto {
                        "h-7 min-w-9 rounded-md bg-sidenav-accent px-1.5 text-xs font-medium cursor-pointer"
                    } else {
                        "h-7 min-w-9 rounded-md px-1.5 text-xs font-medium text-muted-foreground hover:bg-accent cursor-pointer"
                    },
                    title: "Fit to window (Ctrl 0)",
                    onclick: move |_| set_ui_scale(&mut store, UiScale::Auto),
                    "{scale.label()}"
                }
                button { class: btn, title: "Smaller (Ctrl −)", aria_label: "Smaller text",
                    disabled: scale == UiScale::Small,
                    onclick: move |_| set_ui_scale(&mut store, scale.smaller()), "−" }
            }
        }
    }
}

/// Penguin on an ice floe, with a download arrow.
pub const LOGO_DATA_URI: &str = concat!(
    "data:image/svg+xml;utf8,",
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='8 8 112 112'>",
    "<rect x='8' y='8' width='112' height='112' rx='26' fill='%230e7490'/>",
    "<path d='M20 98 Q64 86 108 98 L102 108 Q64 114 26 108 Z' fill='%23e0f2fe'/>",
    "<ellipse cx='56' cy='70' rx='20' ry='24' fill='%2317202e'/>",
    "<ellipse cx='56' cy='75' rx='12.5' ry='17' fill='white'/>",
    "<circle cx='56' cy='48' r='14' fill='%2317202e'/>",
    "<ellipse cx='56' cy='51' rx='9' ry='7.5' fill='white'/>",
    "<circle cx='52' cy='47' r='2.2' fill='%2317202e'/><circle cx='60' cy='47' r='2.2' fill='%2317202e'/>",
    "<path d='M52.5 52 L59.5 52 L56 57 Z' fill='%23f5a623'/>",
    "<circle cx='92' cy='38' r='15' fill='%2322c55e'/>",
    "<path d='M92 29 V46 M85 40 L92 47 L99 40' stroke='white' stroke-width='4' fill='none' stroke-linecap='round' stroke-linejoin='round'/>",
    "</svg>"
);

/// Run `f` on a worker thread and await its result without blocking the UI.
pub async fn off_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = futures_channel::oneshot::channel::<T>();
    std::thread::spawn(move || {
        if cfg!(debug_assertions) && std::env::var_os("FLOE_SLOW").is_some() {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        let _ = tx.send(f());
    });
    rx.await.expect("worker thread finished")
}

pub async fn sleep_ms(ms: u64) {
    let (tx, rx) = futures_channel::oneshot::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        let _ = tx.send(());
    });
    let _ = rx.await;
}

fn match_gtk_theme(dark: bool) {
    #[cfg(target_os = "linux")]
    {
        use gtk::prelude::GtkSettingsExt;
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_theme_name(Some("Adwaita"));
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = dark;
}

// ---------- actions ----------

pub async fn refresh_gh_status(mut store: Store) {
    let st = off_thread(gh::status).await;
    store.gh.set(Some(st));
}

/// Check one repository for a new release. Does nothing if it's already busy.
pub async fn check_repo(mut store: Store, id: u64) {
    let Some(repo) = store.cfg.peek().repo(id).cloned() else { return };
    if store.busy.peek().contains_key(&id) {
        return;
    }
    store.set_busy(id, Some(Busy::Checking));
    let before = repo.status();
    let result = off_thread(move || updater::check(&repo)).await;
    let mut entry = None;
    let mut found = None;
    store.update(|c| {
        if let Some(r) = c.repo_mut(id) {
            found = updater::apply_check(r, result).map(|n| (r.repo.clone(), n));
            let latest = r.latest.as_ref().map(|l| l.tag.clone()).unwrap_or_default();
            entry = Some(match (&r.last_error, r.status()) {
                (Some(e), _) => (r.repo.clone(), format!("Check failed: {e}"), true),
                (None, Status::UpdateAvailable) if before != Status::UpdateAvailable => {
                    (r.repo.clone(), format!("Update available: {latest}"), false)
                }
                (None, s) => (r.repo.clone(), format!("Checked: latest is {latest} ({})", s.label().to_lowercase()), false),
            });
        }
    });
    if let Some((repo, note)) = found {
        store.log(&repo, note, false, "");
    }
    if let Some((repo, text, err)) = entry {
        store.log(&repo, text, err, "");
    }
    store.set_busy(id, None);
}

/// Link every tracked repository that has no app ID yet to a matching installed flatpak,
/// without contacting GitHub.
pub fn link_installed_apps(mut store: Store) {
    spawn_forever(async move {
        let installed = off_thread(floe::flatpak::installed_list).await;
        let mut notes = Vec::new();
        store.update(|c| {
            for r in c.repos.iter_mut() {
                if let Some(n) = updater::link_installed(r, &installed) {
                    notes.push((r.repo.clone(), n));
                }
                if !r.app_id.is_empty() && r.app_present {
                    // Refresh version/origin for linked apps too.
                    let app = installed.iter().find(|a| a.id == r.app_id).cloned();
                    r.app_present = app.is_some();
                    if let Some(a) = app {
                        r.installed_version = Some(a.version).filter(|v| !v.is_empty());
                        r.installed_origin = a.origin;
                    }
                }
            }
        });
        let linked = notes.iter().filter(|(_, n)| n.contains("linked")).count();
        for (repo, n) in notes {
            store.log(&repo, n, false, "");
        }
        store.installed.set(Some(installed));
        store.notify(
            match linked {
                0 => "No new matches among your installed flatpaks.".to_string(),
                1 => "Linked 1 repository to an installed flatpak.".to_string(),
                n => format!("Linked {n} repositories to installed flatpaks."),
            },
            false,
        );
    });
}

/// Refresh the list of installed flatpaks (Installed page).
pub async fn refresh_installed(mut store: Store) {
    let installed = off_thread(floe::flatpak::installed_list).await;
    store.installed.set(Some(installed));
}

/// Check every repository in parallel.
pub async fn check_all_inner(store: Store) {
    let ids: Vec<u64> = store.cfg.peek().repos.iter().map(|r| r.id).collect();
    futures_util::future::join_all(ids.into_iter().map(|id| check_repo(store, id))).await;
}

/// From the "Check all" button: check, then report.
pub fn check_all(mut store: Store, manual: bool) {
    if store.cfg.peek().repos.is_empty() {
        if manual {
            store.notify("Add a repository first.", false);
        }
        return;
    }
    spawn_forever(async move {
        check_all_inner(store).await;
        if manual {
            let cfg = store.cfg.peek();
            let failed = cfg.repos.iter().filter(|r| r.last_error.is_some()).count();
            let n = cfg.updates();
            drop(cfg);
            let text = match (n, failed) {
                (0, 0) => "Everything is up to date.".to_string(),
                (n, 0) => format!("{n} update{} available.", if n == 1 { "" } else { "s" }),
                (n, f) => format!("{n} update{} available; {f} check{} failed (see Activity).", if n == 1 { "" } else { "s" }, if f == 1 { "" } else { "s" }),
            };
            store.notify(text, failed > 0);
        }
    });
}

/// Background check; sends a desktop notification for newly found updates.
async fn check_all_and_notify(store: Store) {
    let before: Vec<u64> =
        store.cfg.peek().repos.iter().filter(|r| r.status() == Status::UpdateAvailable).map(|r| r.id).collect();
    check_all_inner(store).await;
    let cfg = store.cfg.peek();
    let new: Vec<String> = cfg
        .repos
        .iter()
        .filter(|r| r.status() == Status::UpdateAvailable && !before.contains(&r.id))
        .map(|r| format!("{} {}", r.name(), r.latest.as_ref().map(|l| l.tag.as_str()).unwrap_or("")))
        .collect();
    if cfg.settings.notify && !new.is_empty() {
        let title = if new.len() == 1 { "Update available".to_string() } else { format!("{} updates available", new.len()) };
        let body = new.join(", ");
        std::thread::spawn(move || host::notify(&title, &body));
    }
}

/// Download and install the latest release of one repository.
pub async fn update_repo(mut store: Store, id: u64) -> bool {
    let Some(repo) = store.cfg.peek().repo(id).cloned() else { return false };
    if store.busy.peek().contains_key(&id) {
        return false;
    }
    let name = repo.repo.clone();
    let settings = store.cfg.peek().settings.clone();

    store.set_busy(id, Some(Busy::Downloading));
    let job = console::start(id);
    let (r, j) = (repo.clone(), job.clone());
    let downloaded = off_thread(move || updater::download(&r, &j)).await;
    let (release, path) = match downloaded {
        Ok(v) => v,
        Err(e) => return fail(store, id, &name, format!("Download failed: {e}")),
    };
    store.log(&name, format!("Downloaded {} ({})", path.file_name().unwrap_or_default().to_string_lossy(), release.tag), false, path.display().to_string());

    store.set_busy(id, Some(Busy::Installing));
    let (r, p, j) = (repo.clone(), path.clone(), job.clone());
    let installed = off_thread(move || updater::install(&r, &p, &settings, &j)).await;
    console::finish(id);
    match installed {
        Ok(done) => {
            let detected = repo.app_id.is_empty().then(|| done.app_id.clone()).flatten();
            store.update(|c| {
                if let Some(r) = c.repo_mut(id) {
                    updater::apply_install(r, &release, done);
                }
            });
            let mut msg = format!("Installed {}", release.tag);
            if let Some(app) = detected {
                msg.push_str(&format!(" · app ID {app}"));
            }
            store.log(&name, msg, false, console::text(id));
            store.set_busy(id, None);
            store.notify(format!("{} {} installed.", repo.name(), release.tag), false);
            true
        }
        Err(e) => fail(store, id, &name, format!("Install failed: {e}")),
    }
}

fn fail(mut store: Store, id: u64, name: &str, msg: String) -> bool {
    console::finish(id);
    let cancelled = msg.ends_with("Cancelled.");
    if cancelled {
        store.log(name, "Cancelled", false, console::text(id));
        store.set_busy(id, None);
        store.notify(format!("{name}: cancelled."), false);
        return false;
    }
    store.update(|c| {
        if let Some(r) = c.repo_mut(id) {
            r.last_error = Some(msg.clone());
        }
    });
    let details = match console::text(id) {
        t if t.is_empty() => msg.clone(),
        t => format!("{msg}\n\n--- console ---\n{t}"),
    };
    store.log(name, msg.lines().next().unwrap_or(&msg).to_string(), true, details);
    store.set_busy(id, None);
    store.notify(format!("{name}: {msg}"), true);
    false
}

/// Install every available update, one at a time (flatpak serializes installs anyway).
pub fn update_all(mut store: Store) {
    if *store.updating_all.peek() {
        return;
    }
    let ids: Vec<u64> =
        store.cfg.peek().repos.iter().filter(|r| r.status() == Status::UpdateAvailable).map(|r| r.id).collect();
    if ids.is_empty() {
        store.notify("No updates to install.", false);
        return;
    }
    store.updating_all.set(true);
    spawn_forever(async move {
        let total = ids.len();
        let mut ok = 0;
        for id in ids {
            if update_repo(store, id).await {
                ok += 1;
            }
        }
        store.updating_all.set(false);
        store.notify(
            if ok == total { format!("Updated {ok} app{}.", if ok == 1 { "" } else { "s" }) } else { format!("Updated {ok} of {total} apps; see Activity for errors.") },
            ok != total,
        );
    });
}

pub fn launch(mut store: Store, app_id: String) {
    if let Err(e) = floe::flatpak::launch(&app_id) {
        store.notify(e, true);
    }
}

pub fn open_url(mut store: Store, url: String) {
    if let Err(e) = host::open_url(&url) {
        store.notify(e, true);
    }
}

use crate::app::{self, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::separator::Separator;
use crate::ui::switch::Switch;
use crate::widgets::*;
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use floe::config::{self, UiScale};
use floe::flatpak::{self, Scope};
use floe::host;

const INTERVALS: [(u32, &str); 6] =
    [(0, "Never"), (1, "Every hour"), (3, "Every 3 hours"), (6, "Every 6 hours"), (12, "Every 12 hours"), (24, "Once a day")];

#[component]
pub fn SettingsPage() -> Element {
    let mut store = use_store();
    let s = store.cfg.read().settings.clone();
    let config_path = store.config_path();
    let mut import_path = use_signal(String::new);

    let interval_options: Vec<(String, String)> = INTERVALS.iter().map(|(h, l)| (h.to_string(), l.to_string())).collect();
    let scope_options: Vec<(String, String)> = Scope::ALL.iter().map(|s| (s.key().to_string(), s.label().to_string())).collect();

    let export = move |_| {
        let json = store.cfg.read().export_list();
        let path = config::user_downloads().join(format!("floe-list-{}.json", chrono::Local::now().format("%Y-%m-%d")));
        match std::fs::create_dir_all(config::user_downloads()).and_then(|_| std::fs::write(&path, json)) {
            Ok(()) => store.notify(format!("List saved to {}", path.display()), false),
            Err(e) => store.notify(format!("Could not save {}: {e}", path.display()), true),
        }
    };
    let import = move |_| {
        let p = import_path.read().trim().to_string();
        if p.is_empty() {
            return store.notify("Enter the path of an exported list.", true);
        }
        let text = match std::fs::read_to_string(&p) {
            Ok(t) => t,
            Err(e) => return store.notify(format!("Could not read {p}: {e}"), true),
        };
        let mut res = Ok(0);
        store.update(|c| res = c.import_list(&text));
        match res {
            Ok(n) => {
                store.notify(format!("Added {n} repositor{} from the list.", if n == 1 { "y" } else { "ies" }), false);
                if n > 0 {
                    app::check_all(store, false);
                }
            }
            Err(e) => store.notify(e, true),
        }
    };

    rsx! {
        PageHeader { title: "Settings" }

        SelfUpdateCard {}

        GhCard {}

        Card {
            CardHeader {
                CardTitle { "Updates" }
                CardDescription { "Floe only checks while it is open." }
            }
            CardContent { class: "flex flex-col gap-5",
                SettingRow { title: "Check when Floe opens",
                    Switch { checked: s.check_on_launch, aria_label: "Check when Floe opens",
                        onchange: move |v| store.update(|c| c.settings.check_on_launch = v) } }
                Separator {}
                SettingRow { title: "Check again while open", hint: "Runs in the background.",
                    div { class: "w-48",
                        NativeSelect { value: s.check_interval_hours.to_string(), options: interval_options,
                            onchange: move |v: String| store.update(|c| c.settings.check_interval_hours = v.parse().unwrap_or(0)) } } }
                Separator {}
                SettingRow { title: "Desktop notification", hint: "When a background check finds a new release.",
                    Switch { checked: s.notify, aria_label: "Desktop notification",
                        onchange: move |v| store.update(|c| c.settings.notify = v) } }
                Separator {}
                SettingRow { title: "Install for", hint: "Passed to flatpak install. Your app data in ~/.var/app is always kept.",
                    div { class: "w-72",
                        NativeSelect { value: s.scope.key().to_string(), options: scope_options,
                            onchange: move |v: String| store.update(|c| c.settings.scope = Scope::from_key(&v).unwrap_or_default()) } } }
                Separator {}
                SettingRow { title: "Keep downloaded bundles", hint: format!("In {}", config::download_dir().display()),
                    Switch { checked: s.keep_downloads, aria_label: "Keep downloaded bundles",
                        onchange: move |v| store.update(|c| c.settings.keep_downloads = v) } }
            }
        }

        Card {
            CardHeader { CardTitle { "Appearance" } }
            CardContent { class: "flex flex-col gap-5",
                SettingRow { title: "Dark mode",
                    Switch { checked: s.dark_mode, aria_label: "Dark mode",
                        onchange: move |v| store.update(|c| c.settings.dark_mode = v) } }
                Separator {}
                SettingRow { title: "Interface size", hint: "Ctrl + bigger, Ctrl − smaller, Ctrl 0 auto.",
                    div { class: "inline-flex rounded-lg bg-muted p-1", role: "radiogroup", aria_label: "Interface size",
                        for opt in UiScale::ALL {
                            button {
                                key: "{opt.key()}",
                                role: "radio",
                                aria_checked: "{s.ui_scale == opt}",
                                title: opt.long_label(),
                                class: if s.ui_scale == opt {
                                    "rounded-md bg-background px-3 py-1 text-sm font-medium shadow-sm"
                                } else {
                                    "rounded-md px-3 py-1 text-sm text-muted-foreground hover:text-foreground cursor-pointer"
                                },
                                onclick: move |_| app::set_ui_scale(&mut store, opt),
                                "{opt.label()}"
                            }
                        }
                    }
                }
            }
        }

        Card {
            CardHeader {
                CardTitle { "Your list" }
                CardDescription { "Move your tracked repositories to another computer." }
            }
            CardContent { class: "flex flex-col gap-5",
                Field { label: "Saved in",
                    code { class: "rounded bg-muted px-2 py-1 text-xs break-all", "{config_path}" } }
                div { class: "flex flex-col gap-2",
                    div { class: "text-sm font-medium", "Export" }
                    p { class: "text-xs text-muted-foreground", "Saves the list (without check results) to {config::user_downloads().display()}." }
                    div { Button { variant: ButtonVariant::Outline, onclick: export, "Export list" } }
                }
                div { class: "flex flex-col gap-2",
                    div { class: "text-sm font-medium", "Import" }
                    p { class: "text-xs text-muted-foreground", "Adds the repositories from an exported list; ones you already track are skipped." }
                    div { class: "flex gap-2",
                        Input { value: import_path(), placeholder: "/home/you/Downloads/floe-list-….json",
                            oninput: move |e: FormEvent| import_path.set(e.value()) }
                        Button { variant: ButtonVariant::Outline, onclick: import, "Import" }
                    }
                }
            }
        }

        Card {
            CardHeader { CardTitle { "About" } }
            CardContent { class: "text-sm text-muted-foreground flex flex-col gap-1",
                p { "Floe {env!(\"CARGO_PKG_VERSION\")} · Rust, Dioxus and Rust/UI components (the PenguinFin interface)." }
                p { "Releases are fetched with the GitHub CLI and installed with flatpak; Floe never stores a GitHub token." }
                if host::in_flatpak() {
                    p { "Running as a flatpak: gh and flatpak are run on your system through flatpak-spawn --host." }
                }
            }
        }
    }
}

/// GitHub CLI and flatpak status, with the commands to fix what's missing.
#[component]
fn GhCard() -> Element {
    let store = use_store();
    let st = store.gh.read().clone();
    let mut details = use_signal(|| false);
    let mut checking = use_signal(|| false);
    let flatpak_version = use_resource(|| async { app::off_thread(flatpak::available).await });

    let recheck = move |_| {
        checking.set(true);
        spawn_forever(async move {
            app::refresh_gh_status(store).await;
            checking.set(false);
        });
    };

    rsx! {
        Card {
            CardHeader {
                CardTitle { "GitHub CLI" }
                CardDescription { "Floe uses gh to list releases and download files, so private repositories work with your existing login." }
            }
            CardContent { class: "flex flex-col gap-4",
                div { class: "flex flex-wrap items-center gap-2",
                    match &st {
                        None => rsx! { Badge { variant: BadgeVariant::Muted, "Checking…" } },
                        Some(s) if !s.installed => rsx! { Badge { variant: BadgeVariant::Destructive, "gh not installed" } },
                        Some(s) if !s.logged_in => rsx! {
                            Badge { variant: BadgeVariant::Success, "gh {s.version}" }
                            Badge { variant: BadgeVariant::Warning, "not signed in" }
                        },
                        Some(s) => rsx! {
                            Badge { variant: BadgeVariant::Success, "gh {s.version}" }
                            Badge { variant: BadgeVariant::Success, if s.account.is_empty() { "signed in" } else { "signed in as {s.account}" } }
                        },
                    }
                    match &*flatpak_version.read() {
                        Some(Ok(v)) => rsx! { Badge { variant: BadgeVariant::Success, "{v}" } },
                        Some(Err(e)) => rsx! { span { title: "{e}", Badge { variant: BadgeVariant::Destructive, "flatpak not found" } } },
                        None => rsx! {},
                    }
                    div { class: "ml-auto flex gap-2",
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline, disabled: checking(), onclick: recheck,
                            if checking() { Spinner {} } else { Icon { name: IconName::Refresh } } "Re-check" }
                        if st.as_ref().is_some_and(|s| !s.details.is_empty()) {
                            Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| details.toggle(),
                                Icon { name: IconName::Terminal } if details() { "Hide output" } else { "Show output" } }
                        }
                    }
                }
                if details() {
                    if let Some(s) = &st {
                        pre { class: "max-h-60 overflow-auto whitespace-pre-wrap rounded-md bg-muted p-3 text-xs", "{s.details}" }
                    }
                }
                if st.as_ref().is_some_and(|s| !s.installed || !s.logged_in) {
                    div { class: "flex flex-col gap-1 text-sm",
                        p { class: "text-muted-foreground", "Set it up once in a terminal:" }
                        code { class: "rounded bg-muted px-2 py-1 text-xs", "sudo apt install gh    # or: sudo dnf install gh" }
                        code { class: "rounded bg-muted px-2 py-1 text-xs", "gh auth login" }
                    }
                }
            }
        }
    }
}

/// Floe's own version: check GitHub for a newer release, install it, restart.
#[component]
fn SelfUpdateCard() -> Element {
    let mut store = use_store();
    let su = store.self_update.read().clone();
    let check_self = store.cfg.read().settings.check_self;
    let self_beta = store.cfg.read().settings.self_beta;
    let mut notes = use_signal(|| false);
    let sandboxed = host::in_flatpak();
    let current = app::SelfUpdate::current();
    let newer = su.newer().cloned();
    let installing = matches!(su.busy, Some(app::Busy::Downloading | app::Busy::Installing));
    let has_console = floe::console::generation(app::SELF_ID).is_some();

    rsx! {
        Card {
            CardHeader {
                CardTitle { "Floe updates" }
                CardDescription { "Floe updates itself from its own GitHub releases ({host::FLOE_REPO}), the same way it updates your apps." }
            }
            CardContent { class: "flex flex-col gap-4",
                div { class: "flex flex-wrap items-center gap-2",
                    img { class: "size-8", src: app::LOGO_DATA_URI, alt: "" }
                    div { class: "mr-2",
                        div { class: "text-sm font-semibold", "Floe {current}" }
                        div { class: "text-xs text-muted-foreground",
                            if sandboxed { "Installed as a flatpak" } else { "Native build (not the flatpak)" }
                            if let Some(t) = &su.checked { " · checked {t}" }
                        }
                    }
                    match (&su.busy, &su.installed, &newer, &su.latest, &su.error) {
                        (Some(b), _, _, _, _) => rsx! { Badge { variant: BadgeVariant::Secondary, class: "gap-1.5", Spinner { class: "size-3" } "{b.label()}" } },
                        (None, Some(tag), _, _, _) => rsx! { Badge { variant: BadgeVariant::Success, "{tag} installed · restart to use it" } },
                        (None, None, Some(l), _, _) => rsx! { Badge { variant: BadgeVariant::Info, "{l.tag} available" } },
                        (None, None, None, _, Some(_)) => rsx! { Badge { variant: BadgeVariant::Warning, "Check failed" } },
                        (None, None, None, Some(_), None) => rsx! { Badge { variant: BadgeVariant::Success, "Up to date" } },
                        _ => rsx! { Badge { variant: BadgeVariant::Muted, "Not checked" } },
                    }
                }

                if let Some(e) = &su.error {
                    p { class: "whitespace-pre-line text-xs text-destructive", "{e}" }
                }

                if let Some(tag) = &su.installed {
                    div { class: "flex flex-wrap items-center gap-3 rounded-md bg-success-light px-3 py-2 text-sm text-success-dark",
                        Icon { name: IconName::Check, class: "size-4" }
                        span { class: "flex-1", "Floe {tag} is installed. Restart to switch to it; your list and settings are kept." }
                        Button { size: ButtonSize::Sm, onclick: move |_| app::restart_self(store), Icon { name: IconName::Refresh } "Restart Floe" }
                    }
                }

                if !sandboxed {
                    p { class: "text-xs text-muted-foreground",
                        "This copy of Floe was built from source, so installing an update adds the flatpak version alongside it (start it with flatpak run {host::FLOE_APP_ID}). To update this build instead: git pull && cargo install --path ." }
                }

                div { class: "flex flex-wrap items-center gap-2",
                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline, disabled: su.busy.is_some(),
                        onclick: move |_| { spawn_forever(app::check_self(store, true)); },
                        if su.busy == Some(app::Busy::Checking) { Spinner {} } else { Icon { name: IconName::Refresh } }
                        "Check for updates" }
                    if let Some(l) = &newer {
                        Button { size: ButtonSize::Sm, disabled: su.busy.is_some(),
                            onclick: move |_| { spawn_forever(app::update_self(store)); },
                            Icon { name: IconName::Download }
                            if sandboxed { "Update to {l.tag}" } else { "Install flatpak {l.tag}" } }
                    }
                    if installing {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive,
                            onclick: move |_| floe::console::cancel(app::SELF_ID), Icon { name: IconName::Stop } "Cancel" }
                    }
                    if su.latest.as_ref().is_some_and(|l| !l.body.trim().is_empty()) {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| notes.toggle(),
                            Icon { name: IconName::Notes } if notes() { "Hide notes" } else { "What's new" } }
                    }
                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost,
                        onclick: {
                            let url = su.latest.as_ref().map(|l| l.url.clone()).filter(|u| !u.is_empty())
                                .unwrap_or_else(|| format!("https://github.com/{}/releases", host::FLOE_REPO));
                            move |_| app::open_url(store, url.clone())
                        },
                        Icon { name: IconName::External } "Releases" }
                }

                if notes() {
                    if let Some(l) = &su.latest {
                        div { class: "max-h-72 overflow-y-auto rounded-md border bg-muted p-3",
                            div { class: "mb-2 text-sm font-semibold", if l.name.is_empty() { "{l.tag}" } else { "{l.name}" } }
                            pre { class: "whitespace-pre-wrap break-words font-sans text-sm", "{l.body.trim()}" }
                        }
                    }
                }

                if has_console && (installing || su.installed.is_some() || su.error.is_some()) {
                    crate::views::apps::ConsolePanel { id: app::SELF_ID, busy: su.busy }
                }

                Separator {}
                SettingRow { title: "Floe beta updates", hint: "Also offer Floe's own pre-releases. Turning this off doesn't downgrade an installed beta; you get the next stable release once it's newer.",
                    Switch { checked: self_beta, aria_label: "Floe beta updates",
                        onchange: move |v| {
                            store.update(|c| c.settings.self_beta = v);
                            spawn_forever(app::check_self(store, false));
                        } } }
                Separator {}
                SettingRow { title: "Check for Floe updates automatically", hint: "When Floe opens and with the scheduled checks; notifies you if a new version is out.",
                    Switch { checked: check_self, aria_label: "Check for Floe updates automatically",
                        onchange: move |v| store.update(|c| c.settings.check_self = v) } }
            }
        }
    }
}

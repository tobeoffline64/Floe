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

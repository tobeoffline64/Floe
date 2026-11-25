use crate::app::{self, Busy, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::switch::Switch;
use crate::widgets::*;
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use floe::config::{Status, TrackedRepo};
use floe::{console, gh};
use floe::flatpak::Scope;

#[component]
pub fn AppsPage(on_settings: EventHandler<()>) -> Element {
    let store = use_store();
    let cfg = store.cfg.read();
    let total = cfg.repos.len();
    let updates = cfg.updates();
    let ids: Vec<u64> = cfg.repos.iter().map(|r| r.id).collect();
    drop(cfg);
    let checking = store.busy.read().values().any(|b| *b == Busy::Checking);
    let updating_all = (store.updating_all)();
    let mut adding = use_signal(|| false);

    let subtitle = match (total, updates) {
        (0, _) => "Track GitHub repositories and install their flatpak releases.".to_string(),
        (t, 0) => format!("{t} tracked · all up to date"),
        (t, u) => format!("{t} tracked · {u} update{} available", if u == 1 { "" } else { "s" }),
    };

    rsx! {
        PageHeader { title: "Apps", subtitle,
            if total > 0 {
                Button { variant: ButtonVariant::Outline, disabled: checking, title: "Check every repository (Ctrl R)",
                    onclick: move |_| app::check_all(store, true),
                    if checking { Spinner {} } else { Icon { name: IconName::Refresh } }
                    "Check all"
                }
                if updates > 0 {
                    Button { disabled: updating_all, onclick: move |_| app::update_all(store),
                        if updating_all { Spinner {} } else { Icon { name: IconName::Download } }
                        "Update all ({updates})"
                    }
                }
            }
            Button { variant: if adding() || total == 0 { ButtonVariant::Secondary } else { ButtonVariant::Default },
                onclick: move |_| adding.toggle(),
                Icon { name: IconName::Plus } "Add repository"
            }
        }

        GhBanner { on_settings }

        if adding() || total == 0 {
            AddRepoCard { on_done: move |_| adding.set(false) }
        }

        if total == 0 {
            EmptyState {
                title: "No repositories yet",
                text: "Add a GitHub repository that publishes a .flatpak file with its releases. Floe uses your GitHub CLI login, so private repositories work too.",
            }
        } else {
            div { class: "flex flex-col gap-4",
                for id in ids {
                    RepoCard { key: "{id}", id }
                }
            }
        }
    }
}

/// Shown when `gh` is missing or signed out, since nothing works without it.
#[component]
fn GhBanner(on_settings: EventHandler<()>) -> Element {
    let store = use_store();
    let Some(st) = store.gh.read().clone() else { return rsx! {} };
    if st.installed && st.logged_in {
        return rsx! {};
    }
    let (title, text) = if !st.installed {
        ("GitHub CLI not found", "Floe downloads releases with the GitHub CLI. Install it (for example `sudo apt install gh` or `sudo dnf install gh`), then run `gh auth login`.")
    } else {
        ("GitHub CLI isn't signed in", "Run `gh auth login` in a terminal once, then press Re-check.")
    };
    rsx! {
        div { class: "flex flex-wrap items-start gap-3 rounded-xl border border-warning bg-warning-light p-4 text-warning-dark",
            Icon { name: IconName::Alert, class: "mt-0.5 size-5 shrink-0" }
            div { class: "min-w-0 flex-1",
                div { class: "text-sm font-semibold", "{title}" }
                p { class: "text-sm", "{text}" }
            }
            div { class: "flex gap-2",
                Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline,
                    onclick: move |_| { spawn_forever(app::refresh_gh_status(store)); },
                    "Re-check" }
                Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| on_settings.call(()), "Details" }
            }
        }
    }
}

#[component]
fn AddRepoCard(on_done: EventHandler<()>) -> Element {
    let mut store = use_store();
    let mut repo = use_signal(String::new);
    let mut pattern = use_signal(String::new);
    let mut app_id = use_signal(String::new);
    let mut pre = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    // Bumped after adding, to remount (clear) the uncontrolled inputs.
    let mut generation = use_signal(|| 0u32);

    let mut add = move || {
        let mut result = Err(String::new());
        store.update(|c| result = c.add(&repo.read(), &pattern.read(), &app_id.read(), pre()));
        match result {
            Ok(id) => {
                let name = store.cfg.peek().repo(id).map(|r| r.repo.clone()).unwrap_or_default();
                store.log(&name, "Added to the list", false, "");
                error.set(None);
                repo.set(String::new());
                pattern.set(String::new());
                app_id.set(String::new());
                pre.set(false);
                generation += 1;
                spawn_forever(app::check_repo(store, id));
                on_done.call(());
            }
            Err(e) => error.set(Some(e)),
        }
    };

    rsx! {
        Card {
            CardHeader {
                CardTitle { "Add a repository" }
                CardDescription { "Floe checks its GitHub releases and installs the attached flatpak bundle, like Obtainium does for Android." }
            }
            CardContent { class: "flex flex-col gap-4",
                div { key: "{generation}", class: "grid gap-4 @2xl:grid-cols-2",
                    Field { label: "Repository", hint: "owner/name, or paste the GitHub link",
                        Input { placeholder: "owner/app-name", value: repo(),
                            oninput: move |e: FormEvent| repo.set(e.value()) } }
                    Field { label: "Release file", hint: "Which asset to install. * matches anything.",
                        Input { placeholder: gh::DEFAULT_PATTERN, value: pattern(),
                            oninput: move |e: FormEvent| pattern.set(e.value()) } }
                    Field { label: "Flatpak app ID (optional)", hint: "Found automatically after the first install.",
                        Input { placeholder: "io.github.owner.App", value: app_id(),
                            oninput: move |e: FormEvent| app_id.set(e.value()) } }
                    div { class: "flex items-center gap-3 @2xl:pt-6",
                        Switch { checked: pre(), aria_label: "Beta updates", onchange: move |v| pre.set(v) }
                        div {
                            div { class: "text-sm font-medium", "Beta updates" }
                            div { class: "text-xs text-muted-foreground", "Also offer pre-releases (betas, release candidates) when they're newer than the stable release. You can switch any time." }
                        }
                    }
                }
                if let Some(e) = error() {
                    p { class: "text-sm text-destructive", "{e}" }
                }
                div { class: "flex gap-2",
                    Button { onclick: move |_| add(), Icon { name: IconName::Plus } "Add and check" }
                    Button { variant: ButtonVariant::Ghost, onclick: move |_| on_done.call(()), "Cancel" }
                }
            }
        }
    }
}

fn status_badge(s: Status) -> BadgeVariant {
    match s {
        Status::UpdateAvailable => BadgeVariant::Info,
        Status::UpToDate => BadgeVariant::Success,
        Status::Error | Status::NoAsset => BadgeVariant::Warning,
        Status::NotInstalled | Status::Unknown => BadgeVariant::Muted,
        Status::OnBeta => BadgeVariant::Accent,
    }
}

#[component]
fn RepoCard(id: u64) -> Element {
    let mut store = use_store();
    let Some(r) = store.cfg.read().repo(id).cloned() else { return rsx! {} };
    let busy = store.busy(id);
    let status = r.status();
    let mut notes = use_signal(|| false);
    let mut beta_notes = use_signal(|| false);
    let mut confirm_downgrade = use_signal(|| false);
    let mut editing = use_signal(|| false);
    let mut confirm_remove = use_signal(|| false);
    // None = automatic: open while downloading/installing.
    let mut console_open = use_signal(|| None::<bool>);
    let has_console = console::generation(id).is_some();
    let installing = matches!(busy, Some(Busy::Downloading | Busy::Installing));
    let show_console = console_open().unwrap_or(installing);

    let initials: String = r.name().chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
    let latest = r.latest.clone();
    let asset = r.latest_asset().cloned();
    let current = r.current_version().map(str::to_string);
    let has_app = !r.app_id.is_empty() && r.app_present;

    let target_is_beta = latest.as_ref().is_some_and(|l| l.prerelease);
    let target_tag = latest.as_ref().map(|l| l.tag.clone()).unwrap_or_default();
    let primary = match (status, target_is_beta) {
        (Status::UpdateAvailable, true) if r.current_version().is_some_and(floe::version::is_prerelease) => Some(format!("Update to {target_tag}")),
        (Status::UpdateAvailable | Status::NotInstalled, true) => Some(format!("Install beta {target_tag}")),
        (Status::UpdateAvailable, false) => Some(format!("Update to {target_tag}")),
        (Status::NotInstalled, false) => Some("Install".to_string()),
        _ => None,
    };
    let beta_offer = r.beta_on_offer().cloned();
    let stable_tag = r.stable.as_ref().map(|s| s.tag.clone());
    let beta_channel = r.beta_channel();
    let set_channel = move |on: bool| {
        let mut store = store;
        let mut never_checked = false;
        let mut name = String::new();
        store.update(|c| {
            if let Some(r) = c.repo_mut(id) {
                r.set_beta_channel(on);
                never_checked = r.stable.is_none() && r.beta.is_none();
                name = r.repo.clone();
            }
        });
        store.log(&name, if on { "Switched to the beta channel" } else { "Switched to the stable channel" }, false, "");
        if never_checked {
            spawn_forever(app::check_repo(store, id));
        }
    };

    rsx! {
        Card { size: CardSize::Sm, class: if status == Status::UpdateAvailable { "border-info" } else { "" },
            CardContent { class: "flex flex-col gap-3",
                div { class: "flex items-start gap-3",
                    div { class: "flex size-10 shrink-0 items-center justify-center rounded-lg bg-secondary text-sm font-semibold text-secondary-foreground",
                        "{initials}" }
                    div { class: "min-w-0 flex-1",
                        div { class: "flex flex-wrap items-center gap-2",
                            span { class: "truncate text-base font-semibold", "{r.name()}" }
                            span { class: "truncate text-sm text-muted-foreground", "{r.owner()}" }
                            if beta_channel {
                                Badge { variant: BadgeVariant::Warning, size: BadgeSize::Sm, "beta channel" }
                            }
                        }
                        div { class: "truncate text-xs text-muted-foreground",
                            if r.app_id.is_empty() { "App ID: found after the first install" } else { "{r.app_id}" }
                        }
                    }
                    if let Some(b) = busy {
                        Badge { variant: BadgeVariant::Secondary, class: "gap-1.5", Spinner { class: "size-3" } "{b.label()}" }
                    } else {
                        Badge { variant: status_badge(status), "{status.label()}" }
                    }
                }

                div { class: "grid gap-x-6 gap-y-1 text-sm @xl:grid-cols-3",
                    div {
                        span { class: "text-muted-foreground", "Installed: " }
                        if let Some(v) = &current {
                            span { class: "font-medium", "{v}" }
                        } else if r.is_installed() {
                            span { class: "font-medium", "yes" }
                        } else {
                            span { class: "text-muted-foreground", "—" }
                        }
                    }
                    div {
                        if let Some(l) = &latest {
                            span { class: "text-muted-foreground", if l.prerelease { "Latest beta: " } else { "Latest: " } }
                            span { class: "font-medium", "{l.tag}" }
                            span { class: "text-muted-foreground", " · {l.date()}" }
                        } else {
                            span { class: "text-muted-foreground", "Latest: " }
                            span { class: "text-muted-foreground", "not checked yet" }
                        }
                    }
                    div { class: "truncate",
                        span { class: "text-muted-foreground", "File: " }
                        if let Some(a) = &asset {
                            span { class: "font-medium", title: "{a.name}", "{a.name}" }
                            span { class: "text-muted-foreground", " · {file_size(a.size)}" }
                        } else if latest.is_some() {
                            span { class: "text-warning-dark", "nothing matches {r.pattern()}" }
                        } else {
                            span { class: "text-muted-foreground", "{r.pattern()}" }
                        }
                    }
                }

                div { class: "flex flex-wrap items-center gap-2 text-sm",
                    span { class: "text-muted-foreground", "Updates:" }
                    div { class: "inline-flex rounded-lg bg-muted p-0.5", role: "radiogroup", aria_label: "Release channel",
                        for (on, label, tip) in [(false, "Stable", "Only stable releases"), (true, "Beta", "Stable releases and newer betas / release candidates")] {
                            button {
                                key: "{label}",
                                role: "radio",
                                aria_checked: "{beta_channel == on}",
                                title: tip,
                                disabled: busy.is_some(),
                                class: if beta_channel == on {
                                    "rounded-md bg-background px-2.5 py-0.5 text-xs font-medium shadow-sm"
                                } else {
                                    "rounded-md px-2.5 py-0.5 text-xs text-muted-foreground hover:text-foreground cursor-pointer disabled:cursor-default"
                                },
                                onclick: move |_| if beta_channel != on { set_channel(on) },
                                "{label}"
                            }
                        }
                    }
                    if beta_channel {
                        if let (Some(st), true) = (&r.stable, target_is_beta) {
                            span { class: "text-xs text-muted-foreground", "Stable is {st.tag}" }
                        }
                    }
                }

                if let Some(b) = &beta_offer {
                    if status != Status::OnBeta {
                        div { class: "flex flex-wrap items-center gap-2 rounded-md bg-warning-light px-3 py-2 text-xs text-warning-dark",
                            Icon { name: IconName::Notes, class: "size-3.5 shrink-0" }
                            span { class: "min-w-0 flex-1",
                                "Beta {b.tag} ({b.date()}) is available. Betas can have bugs and may upgrade your data in ways older versions can't read, so read the notes first." }
                            if !b.body.trim().is_empty() {
                                Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| beta_notes.toggle(),
                                    if beta_notes() { "Hide notes" } else { "Beta notes" } }
                            }
                            Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline, disabled: busy.is_some(),
                                onclick: move |_| set_channel(true), "Switch to beta" }
                        }
                    }
                }

                if status == Status::OnBeta {
                    div { class: "flex flex-col gap-2 rounded-md bg-accent px-3 py-2 text-xs",
                        span {
                            "You're running beta {current.clone().unwrap_or_default()} on the stable channel, so Floe won't install betas for this app. "
                            "Switch back to Beta to keep getting beta updates"
                            if let Some(st) = &stable_tag { ", or return to stable {st}" }
                            "."
                        }
                        if confirm_downgrade() {
                            div { class: "flex flex-col gap-2 rounded-md border border-destructive bg-background p-2",
                                span { class: "text-destructive",
                                    "Going back reinstalls the older stable version. If the beta upgraded the app's data, the older version may refuse to open it until you restore a backup: check the beta's release notes for how to go back. Your data folder in ~/.var/app is not touched by Floe." }
                                div { class: "flex flex-wrap gap-2",
                                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive, disabled: busy.is_some(),
                                        onclick: move |_| {
                                            confirm_downgrade.set(false);
                                            spawn_forever(async move { app::update_repo(store, id).await; });
                                        },
                                        "Yes, install {stable_tag.clone().unwrap_or_default()}" }
                                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| confirm_downgrade.set(false), "Cancel" }
                                }
                            }
                        } else {
                            div { class: "flex flex-wrap gap-2",
                                Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline, disabled: busy.is_some(),
                                    onclick: move |_| set_channel(true), "Stay on beta" }
                                if let Some(st) = &stable_tag {
                                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, disabled: busy.is_some(),
                                        onclick: move |_| confirm_downgrade.set(true), "Return to stable {st}…" }
                                }
                            }
                        }
                    }
                }

                if target_is_beta && matches!(status, Status::UpdateAvailable | Status::NotInstalled) {
                    p { class: "text-xs text-warning-dark",
                        "{target_tag} is a pre-release. Read its release notes before installing, and back up the app's data if they say so." }
                }

                if let (Some(sug), true) = (&r.suggested_app_id, r.app_id.is_empty()) {
                    div { class: "flex flex-wrap items-center gap-2 rounded-md bg-info-light px-3 py-2 text-xs text-info-dark",
                        Icon { name: IconName::Link, class: "size-3.5 shrink-0" }
                        span { class: "min-w-0 flex-1",
                            "{sug} is installed and may be this app. Link it to track its version (updating would replace it with this repository's build)." }
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline,
                            onclick: {
                                let sug = sug.clone();
                                move |_| {
                                    let sug = sug.clone();
                                    store.update(|c| if let Some(r) = c.repo_mut(id) { r.app_id = sug; r.suggested_app_id = None; });
                                    spawn_forever(app::check_repo(store, id));
                                }
                            },
                            "Link" }
                    }
                }

                if r.installed_from_remote() {
                    p { class: "text-xs text-warning-dark",
                        "Installed from \"{r.installed_origin}\" (not from a GitHub bundle). Updating here replaces it with the GitHub release build; your app data is kept." }
                }

                if let Some(e) = &r.last_error {
                    div { class: "flex items-start gap-2 rounded-md bg-destructive-light px-3 py-2 text-xs text-destructive-dark",
                        Icon { name: IconName::Alert, class: "mt-0.5 size-3.5 shrink-0" }
                        span { class: "whitespace-pre-line break-words", "{e}" }
                    }
                }

                div { class: "flex flex-wrap items-center gap-2",
                    if let Some(label) = primary {
                        Button { size: ButtonSize::Sm, disabled: busy.is_some(),
                            onclick: move |_| { spawn_forever(async move { app::update_repo(store, id).await; }); },
                            Icon { name: IconName::Download } "{label}"
                        }
                    }
                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline, disabled: busy.is_some(),
                        onclick: move |_| { spawn_forever(app::check_repo(store, id)); },
                        Icon { name: IconName::Refresh } "Check"
                    }
                    if installing {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive, title: "Stop the download or install",
                            onclick: move |_| console::cancel(id),
                            Icon { name: IconName::Stop } "Cancel" }
                    }
                    if has_console {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost,
                            onclick: move |_| console_open.set(Some(!show_console)),
                            Icon { name: IconName::Terminal } if show_console { "Hide console" } else { "Console" } }
                    }
                    if has_app {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline,
                            onclick: { let a = r.app_id.clone(); move |_| app::launch(store, a.clone()) },
                            Icon { name: IconName::Play } "Open app"
                        }
                    }
                    if latest.as_ref().is_some_and(|l| !l.body.trim().is_empty()) {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| notes.toggle(),
                            Icon { name: IconName::Notes } if notes() { "Hide notes" } else { "Release notes" }
                        }
                    }
                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, title: "Open on GitHub",
                        onclick: {
                            let url = latest.as_ref().map(|l| l.url.clone()).filter(|u| !u.is_empty()).unwrap_or_else(|| r.url());
                            move |_| app::open_url(store, url.clone())
                        },
                        Icon { name: IconName::External } "GitHub"
                    }
                    div { class: "ml-auto flex gap-1",
                        if status == Status::UpToDate && asset.is_some() {
                            Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, disabled: busy.is_some(),
                                title: "Download and install the latest release again",
                                onclick: move |_| { spawn_forever(async move { app::update_repo(store, id).await; }); },
                                "Reinstall"
                            }
                        }
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Edit", aria_label: "Edit",
                            onclick: move |_| editing.toggle(), Icon { name: IconName::Pencil } }
                        if confirm_remove() {
                            Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive,
                                onclick: {
                                    let name = r.repo.clone();
                                    move |_| {
                                        store.update(|c| c.remove(id));
                                        store.log(&name, "Removed from the list (the app stays installed)", false, "");
                                    }
                                },
                                "Remove" }
                            Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| confirm_remove.set(false), "Keep" }
                        } else {
                            Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Remove from list", aria_label: "Remove from list",
                                onclick: move |_| confirm_remove.set(true), Icon { name: IconName::Trash } }
                        }
                    }
                }

                if confirm_remove() {
                    p { class: "text-xs text-muted-foreground",
                        "Removes it from Floe's list only. To uninstall the app itself: flatpak uninstall {r.app_id}" }
                }

                if beta_notes() {
                    if let Some(b) = &beta_offer {
                        div { class: "max-h-80 overflow-y-auto rounded-md border bg-muted p-3",
                            div { class: "mb-2 text-sm font-semibold", if b.name.is_empty() { "{b.tag}" } else { "{b.name}" } }
                            pre { class: "whitespace-pre-wrap break-words font-sans text-sm text-foreground", "{b.body.trim()}" }
                        }
                    }
                }

                if notes() {
                    if let Some(l) = &latest {
                        div { class: "max-h-80 overflow-y-auto rounded-md border bg-muted p-3",
                            div { class: "mb-2 text-sm font-semibold", if l.name.is_empty() { "{l.tag}" } else { "{l.name}" } }
                            pre { class: "whitespace-pre-wrap break-words font-sans text-sm text-foreground", "{l.body.trim()}" }
                        }
                    }
                }

                if show_console && has_console {
                    ConsolePanel { id, busy }
                }

                if editing() {
                    EditRepo { repo: r.clone(), on_done: move |_| editing.set(false) }
                }

                if let Some(t) = &r.last_checked {
                    div { class: "text-[0.6875rem] text-muted-foreground",
                        "Last checked {t}"
                        if let Some(at) = &r.installed_at { " · installed by Floe {at}" }
                    }
                }
            }
        }
    }
}

#[component]
fn EditRepo(repo: TrackedRepo, on_done: EventHandler<()>) -> Element {
    let mut store = use_store();
    let id = repo.id;
    let mut pattern = use_signal(|| repo.asset_pattern.clone());
    let mut app_id = use_signal(|| repo.app_id.clone());
    let mut pre = use_signal(|| repo.include_prereleases);
    let mut beta_pattern = use_signal(|| repo.beta_asset_pattern.clone());
    let mut error = use_signal(|| None::<String>);

    let save = move |_| {
        let a = app_id.read().trim().to_string();
        if !a.is_empty() && !floe::flatpak::is_app_id(&a) {
            return error.set(Some(format!("\"{a}\" isn't a flatpak app ID (like io.github.owner.App).")));
        }
        let p = pattern.read().trim().to_string();
        let pre_v = pre();
        let changed_source = {
            let cfg = store.cfg.peek();
            cfg.repo(id).is_some_and(|r| r.include_prereleases != pre_v || r.app_id != a)
        };
        store.update(|c| {
            if let Some(r) = c.repo_mut(id) {
                r.asset_pattern = if p == gh::DEFAULT_PATTERN { String::new() } else { p };
                if r.app_id != a {
                    r.app_present = false;
                    r.installed_version = None;
                }
                r.app_id = a;
                r.beta_asset_pattern = beta_pattern.read().trim().to_string();
                r.set_beta_channel(pre_v);
            }
        });
        on_done.call(());
        if changed_source {
            spawn_forever(app::check_repo(store, id));
        }
    };

    rsx! {
        div { class: "flex flex-col gap-4 rounded-md border p-4",
            div { class: "grid gap-4 @2xl:grid-cols-2",
                Field { label: "Release file", hint: "Default: *.flatpak",
                    Input { placeholder: gh::DEFAULT_PATTERN, value: pattern(), oninput: move |e: FormEvent| pattern.set(e.value()) } }
                Field { label: "Flatpak app ID", hint: "Used to read the installed version and to open the app.",
                    Input { placeholder: "io.github.owner.App", value: app_id(), oninput: move |e: FormEvent| app_id.set(e.value()) } }
                Field { label: "Beta release file (optional)", hint: "Only if betas name their bundle differently. Default: same as above.",
                    Input { placeholder: "e.g. *-beta*.flatpak", value: beta_pattern(), oninput: move |e: FormEvent| beta_pattern.set(e.value()) } }
            }
            div { class: "flex items-center gap-3",
                Switch { checked: pre(), aria_label: "Beta updates", onchange: move |v| pre.set(v) }
                div {
                    div { class: "text-sm", "Beta updates" }
                    div { class: "text-xs text-muted-foreground", "Also offer pre-releases when they're newer than the stable release." }
                }
            }
            if let Some(e) = error() {
                p { class: "text-sm text-destructive", "{e}" }
            }
            div { class: "flex gap-2",
                Button { size: ButtonSize::Sm, onclick: save, Icon { name: IconName::Check } "Save" }
                Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| on_done.call(()), "Cancel" }
            }
        }
    }
}

/// Live output of the running (or last) download and install, polled from the worker threads.
#[component]
pub fn ConsolePanel(id: u64, busy: Option<Busy>) -> Element {
    let store = use_store();
    let mut snap = use_signal(|| console::snapshot(id));
    use_future(move || async move {
        let mut last = None;
        loop {
            let g = console::generation(id);
            // While running, refresh at least once a second so the "quiet for" hint updates.
            if g != last || g.is_some_and(|(_, running)| running) {
                snap.set(console::snapshot(id));
                last = g;
            }
            app::sleep_ms(400).await;
        }
    });
    let Some(s) = snap() else { return rsx! {} };
    let scope = store.cfg.read().settings.scope;
    let system_install = s.lines.iter().any(|l| l.starts_with("$ flatpak install --system"));
    let start = s.lines.len().saturating_sub(500);
    let shown = &s.lines[start..];
    let copy = {
        let text = s.lines.join("\n");
        move |_| {
            let js = format!("navigator.clipboard.writeText({});", serde_json::to_string(&text).unwrap_or_default());
            document::eval(&js);
        }
    };

    rsx! {
        div { class: "flex flex-col gap-2 rounded-md border bg-muted/50 p-3",
            div { class: "flex flex-wrap items-center gap-2 text-xs",
                Icon { name: IconName::Terminal, class: "size-3.5" }
                span { class: "font-semibold", "Console" }
                if s.running {
                    Spinner { class: "size-3" }
                    span { class: "text-muted-foreground", if let Some(b) = busy { "{b.label()}" } else { "Running…" } }
                } else {
                    span { class: "text-muted-foreground", "finished" }
                }
                div { class: "ml-auto",
                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: copy, "Copy output" }
                }
            }
            if let Some((done, total)) = s.progress {
                div { class: "flex items-center gap-2 text-xs text-muted-foreground",
                    crate::ui::progress::Progress { value: done as f64, max: total.max(1) as f64, class: "flex-1" }
                    span { class: "tabular-nums", "{file_size(done)} / {file_size(total)}" }
                }
            }
            div { class: "flex max-h-72 flex-col-reverse overflow-auto rounded bg-zinc-950 px-3 py-2",
                pre { class: "whitespace-pre-wrap break-all font-mono text-[0.75rem] leading-5 text-zinc-100",
                    if start > 0 { "… {start} earlier lines (Copy output has everything)\n" }
                    for line in shown.iter() {
                        span { class: if line.starts_with("$ ") { "text-sky-300" } else if line.to_lowercase().starts_with("error") { "text-red-400" } else { "" },
                            "{line}\n" }
                    }
                }
            }
            if s.running && s.quiet_secs >= 20 && busy == Some(Busy::Installing) {
                p { class: "text-xs text-muted-foreground",
                    "No new output for {s.quiet_secs} s. The first install of an app also downloads its runtime (for example GNOME, several hundred MB) and flatpak prints little while it does. "
                    if scope == Scope::System || system_install { "Installing for all users waits for your administrator password; look for a password window (it may be behind other windows). " }
                    "You can keep waiting or press Cancel." }
            } else if s.running && s.quiet_secs >= 30 && busy == Some(Busy::Downloading) && s.progress.is_none_or(|(d, _)| d == 0) {
                p { class: "text-xs text-muted-foreground", "gh hasn't started writing the file after {s.quiet_secs} s. Check your network, or Cancel and try again." }
            }
        }
    }
}

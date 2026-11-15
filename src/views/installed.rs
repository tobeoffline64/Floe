use crate::app::{self, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::widgets::*;
use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;
use floe::flatpak::{self, InstalledApp, Match};

/// The flatpaks installed on this computer, which repository each one belongs to, and a
/// one-click way to start tracking GitHub-hosted ones.
#[component]
pub fn InstalledPage(on_apps: EventHandler<()>) -> Element {
    let mut store = use_store();
    let mut loading = use_signal(|| false);
    let mut show_all = use_signal(|| false);

    // Load (or refresh) the list whenever the page opens.
    use_hook(move || {
        loading.set(true);
        spawn(async move {
            app::refresh_installed(store).await;
            loading.set(false);
        });
    });

    let installed = store.installed.read().clone();
    let cfg = store.cfg.read().clone();

    let Some(apps) = installed else {
        return rsx! {
            PageHeader { title: "Installed flatpaks" }
            div { class: "flex items-center gap-2 text-sm text-muted-foreground", Spinner {} "Reading flatpak list…" }
        };
    };

    // Which tracked repository each app belongs to: linked by app ID, or a sure/possible match.
    let owner_of = |a: &InstalledApp| -> Option<(String, bool)> {
        if let Some(r) = cfg.repos.iter().find(|r| r.app_id == a.id) {
            return Some((r.repo.clone(), true));
        }
        cfg.repos
            .iter()
            .filter(|r| r.app_id.is_empty())
            .find(|r| flatpak::match_installed(&r.repo, &apps).is_some_and(|(m, _)| m.id == a.id))
            .map(|r| (r.repo.clone(), false))
    };

    let mut rows: Vec<(InstalledApp, Option<(String, bool)>)> = apps.iter().map(|a| (a.clone(), owner_of(a))).collect();
    // Bundles and tracked apps first, then by name.
    rows.sort_by_key(|(a, o)| (o.is_none(), !a.from_bundle(), a.name.to_lowercase()));
    let hidden = rows.iter().filter(|(a, o)| o.is_none() && !a.from_bundle() && flatpak::guess_repo(&a.id).is_none()).count();
    if !show_all() {
        rows.retain(|(a, o)| o.is_some() || a.from_bundle() || flatpak::guess_repo(&a.id).is_some());
    }
    let unlinked = cfg
        .repos
        .iter()
        .filter(|r| r.app_id.is_empty() && flatpak::match_installed(&r.repo, &apps).is_some_and(|(_, m)| m == Match::Sure))
        .count();

    rsx! {
        PageHeader { title: "Installed flatpaks",
            subtitle: format!("{} apps on this computer · bundles installed from files and GitHub-style IDs first", apps.len()),
            Button { variant: ButtonVariant::Outline, disabled: loading(),
                onclick: move |_| {
                    loading.set(true);
                    spawn(async move { app::refresh_installed(store).await; loading.set(false); });
                },
                if loading() { Spinner {} } else { Icon { name: IconName::Refresh } }
                "Refresh"
            }
            if unlinked > 0 {
                Button { onclick: move |_| app::link_installed_apps(store),
                    Icon { name: IconName::Link } "Link {unlinked} to my repositories" }
            }
        }

        p { class: "text-sm text-muted-foreground",
            "When you add a repository, Floe looks here for an app with the same ID (io.github.owner.Name) or a bundle with the same name, and links it so it shows the installed version instead of installing again." }

        if rows.is_empty() {
            EmptyState { title: "Nothing to show", text: "No flatpak bundles or GitHub-hosted apps are installed." }
        } else {
            Card { size: CardSize::Sm, class: "gap-0 py-0",
                for (i, (a, owner)) in rows.into_iter().enumerate() {
                    div { key: "{a.id}-{a.scope.key()}",
                        class: if i > 0 { "flex flex-wrap items-center gap-3 border-t px-4 py-3" } else { "flex flex-wrap items-center gap-3 px-4 py-3" },
                        div { class: "min-w-0 flex-1",
                            div { class: "flex flex-wrap items-center gap-2",
                                span { class: "font-medium", if a.name.is_empty() { "{a.id}" } else { "{a.name}" } }
                                if !a.version.is_empty() { span { class: "text-sm text-muted-foreground", "{a.version}" } }
                                Badge { size: BadgeSize::Sm, variant: if a.from_bundle() { BadgeVariant::Info } else { BadgeVariant::Muted },
                                    if a.from_bundle() { "bundle" } else { "{a.origin}" } }
                                Badge { size: BadgeSize::Sm, variant: BadgeVariant::Outline, "{a.scope.key()}" }
                            }
                            div { class: "truncate text-xs text-muted-foreground", "{a.id}" }
                        }
                        match owner {
                            Some((repo, true)) => rsx! {
                                span { class: "flex items-center gap-1 text-sm text-success-dark", Icon { name: IconName::Check, class: "size-4" } "Tracked: {repo}" }
                            },
                            Some((repo, false)) => rsx! {
                                Button { size: ButtonSize::Sm, variant: ButtonVariant::Outline,
                                    onclick: {
                                        let (id, repo) = (a.id.clone(), repo.clone());
                                        move |_| {
                                            let mut rid = None;
                                            store.update(|c| {
                                                if let Some(r) = c.repos.iter_mut().find(|r| r.repo == repo) {
                                                    r.app_id = id.clone();
                                                    r.suggested_app_id = None;
                                                    rid = Some(r.id);
                                                }
                                            });
                                            store.log(&repo, format!("Linked to installed app {id}"), false, "");
                                            if let Some(rid) = rid { spawn_forever(app::check_repo(store, rid)); }
                                        }
                                    },
                                    Icon { name: IconName::Link } "Link to {repo}" }
                            },
                            None => match flatpak::guess_repo(&a.id) {
                                Some(repo) => rsx! {
                                    Button { size: ButtonSize::Sm,
                                        title: "Add {repo} to your list, linked to this installed app",
                                        onclick: {
                                            let (id, repo) = (a.id.clone(), repo.clone());
                                            move |_| {
                                                let mut res = Err(String::new());
                                                store.update(|c| res = c.add(&repo, "", &id, false));
                                                match res {
                                                    Ok(rid) => {
                                                        store.log(&repo, format!("Added from installed app {id}"), false, "");
                                                        store.notify(format!("Tracking {repo}."), false);
                                                        spawn_forever(app::check_repo(store, rid));
                                                    }
                                                    Err(e) => store.notify(e, true),
                                                }
                                            }
                                        },
                                        Icon { name: IconName::Plus } "Track {repo}" }
                                },
                                None => rsx! { span { class: "text-xs text-muted-foreground", "Not from GitHub" } },
                            },
                        }
                    }
                }
            }
        }

        if hidden > 0 || show_all() {
            div {
                Button { variant: ButtonVariant::Ghost, size: ButtonSize::Sm, onclick: move |_| show_all.toggle(),
                    if show_all() { "Hide apps from Flathub and other remotes" } else { "Show {hidden} more apps from Flathub and other remotes" } }
                Button { variant: ButtonVariant::Ghost, size: ButtonSize::Sm, onclick: move |_| on_apps.call(()), "Back to Apps" }
            }
        }
    }
}

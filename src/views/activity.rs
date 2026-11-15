use crate::app::use_store;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::widgets::*;
use dioxus::prelude::*;

/// Everything Floe did this session, newest first, with the full command output on demand.
#[component]
pub fn ActivityPage() -> Element {
    let mut store = use_store();
    let log = store.log.read().clone();
    let mut open = use_signal(|| None::<usize>);
    let n = log.len();

    rsx! {
        PageHeader { title: "Activity", subtitle: "Checks, downloads and installs since Floe was opened.",
            if n > 0 {
                Button { variant: ButtonVariant::Outline, onclick: move |_| { store.log.write().clear(); open.set(None); }, "Clear" }
            }
        }
        if log.is_empty() {
            EmptyState { title: "Nothing yet", text: "Results of checks and installs show up here, including the full output of gh and flatpak when something goes wrong." }
        } else {
            Card { size: CardSize::Sm, class: "gap-0 py-0",
                for (i, e) in log.into_iter().enumerate() {
                    div { key: "{n - i}", class: if i > 0 { "border-t" } else { "" },
                        button {
                            class: "flex w-full items-start gap-3 px-4 py-2.5 text-left text-sm hover:bg-accent/50 cursor-pointer disabled:cursor-default",
                            disabled: e.details.is_empty(),
                            onclick: move |_| open.set(if open() == Some(i) { None } else { Some(i) }),
                            span { class: "w-16 shrink-0 tabular-nums text-xs leading-5 text-muted-foreground", "{e.time}" }
                            span { class: "w-44 shrink-0 truncate font-medium", title: "{e.repo}", "{e.repo}" }
                            span { class: if e.error { "min-w-0 flex-1 break-words text-destructive" } else { "min-w-0 flex-1 break-words" }, "{e.text}" }
                            if !e.details.is_empty() {
                                Icon { name: IconName::ChevronDown, class: if open() == Some(i) { "mt-0.5 size-4 rotate-180 text-muted-foreground" } else { "mt-0.5 size-4 text-muted-foreground" } }
                            }
                        }
                        if open() == Some(i) {
                            pre { class: "mx-4 mb-3 max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-3 text-xs", "{e.details}" }
                        }
                    }
                }
            }
        }
    }
}

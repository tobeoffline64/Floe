//! Small app-specific widgets built on top of the Rust/UI components (shared with PenguinFin).

use crate::ui::label::Label;
use dioxus::prelude::*;

// ---------- icons (Lucide, ISC license) ----------

#[derive(Clone, Copy, PartialEq)]
pub enum IconName {
    Package,
    Activity,
    Settings,
    Plus,
    Pencil,
    Trash,
    Alert,
    ChevronDown,
    Refresh,
    Download,
    Play,
    External,
    Check,
    Loader,
    Terminal,
    Notes,
    Disk,
    Stop,
    Link,
}

fn icon_paths(i: IconName) -> &'static [&'static str] {
    match i {
        IconName::Package => &[
            "M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z",
            "M12 22V12",
            "m3.3 7 8.7 5 8.7-5",
            "m7.5 4.27 9 5.15",
        ],
        IconName::Activity => &["M22 12h-4l-3 9L9 3l-3 9H2"],
        IconName::Settings => &["M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z", "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"],
        IconName::Plus => &["M5 12h14", "M12 5v14"],
        IconName::Pencil => &["M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"],
        IconName::Trash => &["M3 6h18", "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6", "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"],
        IconName::Alert => &["m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z", "M12 9v4", "M12 17h.01"],
        IconName::ChevronDown => &["m6 9 6 6 6-6"],
        IconName::Refresh => &[
            "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8",
            "M21 3v5h-5",
            "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16",
            "M8 16H3v5",
        ],
        IconName::Download => &["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "m7 10 5 5 5-5", "M12 15V3"],
        IconName::Play => &["M6 3 20 12 6 21Z"],
        IconName::External => &["M15 3h6v6", "M10 14 21 3", "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"],
        IconName::Check => &["M20 6 9 17l-5-5"],
        IconName::Loader => &["M21 12a9 9 0 1 1-6.219-8.56"],
        IconName::Terminal => &["m4 17 6-6-6-6", "M12 19h8"],
        IconName::Disk => &["M22 12H2", "M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z", "M6 16h.01", "M10 16h.01"],
        IconName::Stop => &["M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z", "M9 9h6v6H9z"],
        IconName::Link => &["M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71", "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"],
        IconName::Notes => &["M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z", "M14 2v4a2 2 0 0 0 2 2h4", "M10 9H8", "M16 13H8", "M16 17H8"],
    }
}

#[component]
pub fn Icon(name: IconName, #[props(into, default = "size-4".to_string())] class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            for d in icon_paths(name).iter() {
                path { d: "{d}" }
            }
        }
    }
}

/// A spinning loader icon.
#[component]
pub fn Spinner(#[props(into, default = "size-4".to_string())] class: String) -> Element {
    rsx! { Icon { name: IconName::Loader, class: "animate-spin {class}" } }
}

// ---------- form helpers ----------

#[component]
pub fn Field(#[props(into)] label: String, #[props(into, optional)] hint: Option<String>, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-col gap-1.5",
            Label { class: "text-xs font-medium text-muted-foreground", "{label}" }
            {children}
            if let Some(h) = hint {
                p { class: "text-xs text-muted-foreground", "{h}" }
            }
        }
    }
}

/// `appearance-none` stops WebKitGTK from drawing the control with the system GTK theme;
/// colors come from the app theme instead.
pub const SELECT_CLASS: &str = "appearance-none border-input bg-background text-foreground dark:bg-input/30 h-9 w-full rounded-md border pl-3 pr-9 text-sm shadow-xs outline-none cursor-pointer focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50";

/// A native `<select>` styled like the Rust/UI Input, with its own chevron.
#[component]
pub fn NativeSelect(value: String, options: Vec<(String, String)>, onchange: EventHandler<String>) -> Element {
    rsx! {
        div { class: "relative w-full",
            select {
                class: SELECT_CLASS,
                onchange: move |e| onchange.call(e.value()),
                for (k, label) in options {
                    option { value: "{k}", selected: k == value, "{label}" }
                }
            }
            Icon { name: IconName::ChevronDown,
                class: "pointer-events-none absolute right-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" }
        }
    }
}

#[component]
pub fn PageHeader(#[props(into)] title: String, #[props(into, optional)] subtitle: Option<String>, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-wrap items-end justify-between gap-4",
            div {
                h1 { class: "text-2xl font-semibold tracking-tight", "{title}" }
                if let Some(s) = subtitle {
                    p { class: "text-sm text-muted-foreground", "{s}" }
                }
            }
            div { class: "flex flex-wrap gap-2", {children} }
        }
    }
}

#[component]
pub fn EmptyState(#[props(into)] title: String, #[props(into)] text: String, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed p-10 text-center",
            h3 { class: "text-base font-semibold", "{title}" }
            p { class: "max-w-md text-sm text-muted-foreground", "{text}" }
            {children}
        }
    }
}

/// A labelled row with a control on the right (settings pages).
#[component]
pub fn SettingRow(#[props(into)] title: String, #[props(into, optional)] hint: Option<String>, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-wrap items-center justify-between gap-3",
            div { class: "min-w-0 flex-1",
                div { class: "text-sm font-medium", "{title}" }
                if let Some(h) = hint {
                    div { class: "text-xs text-muted-foreground", "{h}" }
                }
            }
            {children}
        }
    }
}

/// "24.1 MB"
pub fn file_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} kB", b / 1e3)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sizes() {
        assert_eq!(super::file_size(24_100_000), "24.1 MB");
        assert_eq!(super::file_size(900), "900 B");
    }
}

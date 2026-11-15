//! Floe desktop app entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;
mod views;
mod widgets;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use floe::config::{self, Settings};

/// Tailwind output, generated from `style/app.css` (see README).
const APP_CSS: &str = include_str!("../assets/app.css");

/// Shown while the webview starts (the window would otherwise be blank for a moment).
const BOOT_SKELETON: &str = r#"<div id="boot" aria-hidden="true" class="@container/app fixed inset-0 flex bg-background">
  <div class="flex w-14 @4xl/app:w-56 shrink-0 flex-col gap-3 border-r bg-sidenav p-3">
    <div class="pf-sk h-10 rounded-md"></div>
    <div class="pf-sk h-8 rounded-md"></div><div class="pf-sk h-8 rounded-md"></div><div class="pf-sk h-8 rounded-md"></div>
  </div>
  <div class="@container flex flex-1 flex-col gap-6 p-4 @3xl:p-8">
    <div class="pf-sk h-8 w-56 rounded-md"></div>
    <div class="pf-sk h-28 rounded-xl"></div><div class="pf-sk h-28 rounded-xl"></div><div class="pf-sk h-28 rounded-xl"></div>
  </div>
</div>"#;

fn index_html(settings: &Settings) -> String {
    format!(
        r#"<!DOCTYPE html>
<html class="{dark}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Floe</title>
<style>{APP_CSS}</style>
<style>html {{ font-size: {size}; }}</style>
</head>
<body>
{BOOT_SKELETON}
<div id="main"></div>
</body>
</html>"#,
        dark = if settings.dark_mode { "dark" } else { "" },
        size = settings.ui_scale.root_font_size(),
    )
}

/// `FLOE_WINDOW_SIZE=WxH` overrides the initial window size (handy for testing layouts).
fn initial_size() -> LogicalSize<f64> {
    std::env::var("FLOE_WINDOW_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some(LogicalSize::new(w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
        .unwrap_or(LogicalSize::new(1100.0, 780.0))
}

/// `FLOE_COMPAT_RENDERING=1`: slower but most compatible rendering (XWayland, no DMA-BUF),
/// in case a graphics driver shows a blank or flickering window.
fn compat_rendering() -> bool {
    std::env::var("FLOE_COMPAT_RENDERING").is_ok_and(|v| !v.is_empty() && v != "0")
}

fn main() {
    let path = config::config_path();
    let boot = config::Config::load(&path).map(|c| (path.clone(), c));
    let settings = boot.as_ref().map(|(_, c)| c.settings.clone()).unwrap_or_default();
    app::set_boot(boot);

    // The app pins its own GTK theme to match light/dark; GTK_THEME would override that.
    // SAFETY: still single-threaded here.
    unsafe { std::env::remove_var("GTK_THEME") };

    let (r, g, b) = if settings.dark_mode { (46, 46, 46) } else { (255, 255, 255) };
    let window = WindowBuilder::new()
        .with_title("Floe")
        .with_inner_size(initial_size())
        .with_min_inner_size(LogicalSize::new(640.0, 480.0));
    let cfg = Config::new()
        .with_window(window)
        .with_menu(None)
        .with_custom_index(index_html(&settings))
        .with_background_color((r, g, b, 255))
        .with_data_directory(config::data_dir().join("webview"))
        .with_disable_context_menu(!cfg!(debug_assertions))
        .with_disable_dma_buf_on_wayland(compat_rendering());
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(app::App);
}

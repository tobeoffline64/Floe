//! Floe core: the tracked-repository list, the `gh` and `flatpak` command wrappers and
//! version logic. Everything here is UI-independent and blocking (the app calls it from
//! worker threads).
pub mod config;
pub mod console;
pub mod flatpak;
pub mod gh;
pub mod host;
pub mod updater;
pub mod version;

//! GSettings access, Viaduct-style: a per-thread singleton that returns
//! `None` when the schema isn't installed (e.g. a bare `cargo run` before
//! `build.rs` compiled it), so every caller falls back to defaults instead
//! of panicking.

use std::cell::RefCell;

use gtk::gio;
use gtk::prelude::*;

pub const KEY_JUNK_FILTER: &str = "junk-filter";
pub const KEY_SOURCE_PATH: &str = "source-path";
pub const KEY_THEME: &str = "theme";
pub const KEY_WINDOW_WIDTH: &str = "window-width";
pub const KEY_WINDOW_HEIGHT: &str = "window-height";
pub const KEY_WINDOW_MAXIMIZED: &str = "window-maximized";

const SCHEMA_ID: &str = "org.virinvictus.Colophon";

thread_local! {
    // A transient gio::Settings drops its connect_changed handlers with
    // it, so the instance must live for the process (Viaduct hit this).
    static SETTINGS: RefCell<Option<Option<gio::Settings>>> = const { RefCell::new(None) };
}

pub fn settings() -> Option<gio::Settings> {
    SETTINGS.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| {
                let found = gio::SettingsSchemaSource::default()
                    .and_then(|source| source.lookup(SCHEMA_ID, true))
                    .is_some();
                found.then(|| gio::Settings::new(SCHEMA_ID))
            })
            .clone()
    })
}

/// The saved import source, if any.
pub fn source_path() -> Option<std::path::PathBuf> {
    let raw = settings()?.string(KEY_SOURCE_PATH);
    (!raw.is_empty()).then(|| std::path::PathBuf::from(raw.as_str()))
}

/// The saved theme selection, or "system" when unset or schema-less.
pub fn theme() -> String {
    settings()
        .map(|s| s.string(KEY_THEME).to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::theme::SYSTEM_ID.to_string())
}

//! Theming. A registry of named palettes (Kanagawa, Gruvbox, Nord, Rosé
//! Pine, Solarized) plus a "Follow system" mode that tracks the desktop's
//! light/dark preference. One [`Theme`] drives both the libadwaita CSS
//! variables and the chart colors, so a single definition themes the whole
//! app; `charts` reads [`active`] at draw time.

use std::cell::{Cell, RefCell};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

/// A named palette. The colour roles feed both the generated adwaita CSS
/// and the cairo chart widgets.
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    /// Polarity: drives GTK's prefer-dark setting so stock widgets match.
    pub dark: bool,
    pub bg: &'static str,        // view / sidebar background
    pub bg_window: &'static str, // window background
    pub bg_header: &'static str, // header bar
    pub bg_card: &'static str,   // cards, popovers, dialogs
    pub fg: &'static str,        // primary text
    pub fg_dim: &'static str,    // dim text, chart muted
    pub heading: &'static str,   // group headers, emphasised labels
    pub accent: &'static str,    // accent + primary chart series
    pub on_accent: &'static str, // text on the accent colour
    pub secondary: &'static str, // chart highlight / second series
    pub grid: &'static str,      // chart empty cells, hairlines, borders
    pub warn: &'static str,
    pub err: &'static str,
    pub ok: &'static str,
}

/// Selection value that follows the desktop light/dark preference.
pub const SYSTEM_ID: &str = "system";
const DEFAULT_DARK: &str = "kanagawa-dragon";
const DEFAULT_LIGHT: &str = "kanagawa-lotus";

/// Every selectable fixed theme, in menu order.
pub const THEMES: &[Theme] = &[
    Theme {
        id: "kanagawa-dragon",
        name: "Kanagawa Dragon",
        dark: true,
        bg: "#12120f",
        bg_window: "#181616",
        bg_header: "#0d0c0c",
        bg_card: "#1d1c19",
        fg: "#c5c9c5",
        fg_dim: "#a6a69c",
        heading: "#c8c093",
        accent: "#8ba4b0",
        on_accent: "#0d0c0c",
        secondary: "#b6927b",
        grid: "#282727",
        warn: "#c4b28a",
        err: "#c4746e",
        ok: "#87a987",
    },
    Theme {
        id: "kanagawa-wave",
        name: "Kanagawa Wave",
        dark: true,
        bg: "#1f1f28",
        bg_window: "#16161d",
        bg_header: "#16161d",
        bg_card: "#2a2a37",
        fg: "#dcd7ba",
        fg_dim: "#a89f8a",
        heading: "#c8c093",
        accent: "#7e9cd8",
        on_accent: "#16161d",
        secondary: "#ffa066",
        grid: "#363646",
        warn: "#e6c384",
        err: "#c34043",
        ok: "#98bb6c",
    },
    Theme {
        id: "kanagawa-lotus",
        name: "Kanagawa Lotus",
        dark: false,
        bg: "#f2ecbc",
        bg_window: "#e7dba0",
        bg_header: "#e5ddb0",
        bg_card: "#e5ddb0",
        fg: "#545464",
        fg_dim: "#8a8980",
        heading: "#43436c",
        accent: "#4d699b",
        on_accent: "#f2ecbc",
        secondary: "#cc6d00",
        grid: "#d5cea3",
        warn: "#836f4a",
        err: "#c84053",
        ok: "#6f894e",
    },
    Theme {
        id: "gruvbox-dark",
        name: "Gruvbox Dark",
        dark: true,
        bg: "#282828",
        bg_window: "#282828",
        bg_header: "#1d2021",
        bg_card: "#3c3836",
        fg: "#ebdbb2",
        fg_dim: "#a89984",
        heading: "#d5c4a1",
        accent: "#83a598",
        on_accent: "#282828",
        secondary: "#fe8019",
        grid: "#504945",
        warn: "#fabd2f",
        err: "#fb4934",
        ok: "#b8bb26",
    },
    Theme {
        id: "gruvbox-light",
        name: "Gruvbox Light",
        dark: false,
        bg: "#fbf1c7",
        bg_window: "#f2e5bc",
        bg_header: "#ebdbb2",
        bg_card: "#ebdbb2",
        fg: "#3c3836",
        fg_dim: "#7c6f64",
        heading: "#504945",
        accent: "#076678",
        on_accent: "#fbf1c7",
        secondary: "#af3a03",
        grid: "#d5c4a1",
        warn: "#b57614",
        err: "#9d0006",
        ok: "#79740e",
    },
    Theme {
        id: "nord",
        name: "Nord",
        dark: true,
        bg: "#2e3440",
        bg_window: "#2e3440",
        bg_header: "#2b303b",
        bg_card: "#3b4252",
        fg: "#eceff4",
        fg_dim: "#8b93a3",
        heading: "#d8dee9",
        accent: "#88c0d0",
        on_accent: "#2e3440",
        secondary: "#d08770",
        grid: "#434c5e",
        warn: "#ebcb8b",
        err: "#bf616a",
        ok: "#a3be8c",
    },
    Theme {
        id: "rose-pine",
        name: "Rosé Pine",
        dark: true,
        bg: "#191724",
        bg_window: "#191724",
        bg_header: "#1f1d2e",
        bg_card: "#1f1d2e",
        fg: "#e0def4",
        fg_dim: "#908caa",
        heading: "#ebbcba",
        accent: "#9ccfd8",
        on_accent: "#191724",
        secondary: "#f6c177",
        grid: "#403d52",
        warn: "#f6c177",
        err: "#eb6f92",
        ok: "#31748f",
    },
    Theme {
        id: "solarized-light",
        name: "Solarized Light",
        dark: false,
        bg: "#fdf6e3",
        bg_window: "#eee8d5",
        bg_header: "#eee8d5",
        bg_card: "#eee8d5",
        fg: "#657b83",
        fg_dim: "#93a1a1",
        heading: "#586e75",
        accent: "#268bd2",
        on_accent: "#fdf6e3",
        secondary: "#cb4b16",
        grid: "#e3ddc8",
        warn: "#b58900",
        err: "#dc322f",
        ok: "#859900",
    },
];

pub fn by_id(id: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.id == id)
}

fn dragon() -> &'static Theme {
    by_id(DEFAULT_DARK).expect("dragon is always present")
}

/// Resolve a stored selection (`SYSTEM_ID` or a theme id) against the
/// current system dark preference.
fn resolve(selection: &str, system_dark: bool) -> &'static Theme {
    if selection == SYSTEM_ID {
        by_id(if system_dark {
            DEFAULT_DARK
        } else {
            DEFAULT_LIGHT
        })
        .expect("defaults present")
    } else {
        by_id(selection).unwrap_or_else(dragon)
    }
}

thread_local! {
    static ACTIVE: RefCell<&'static Theme> = RefCell::new(dragon());
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static SELECTION: RefCell<String> = RefCell::new(String::from(SYSTEM_ID));
    // Dark until the settings portal says otherwise (spec: no portal backend
    // degrades to the dark default, never a failure).
    static SYSTEM_DARK: Cell<bool> = const { Cell::new(true) };
    // Held so the SettingChanged subscription outlives startup.
    static BUS: RefCell<Option<gio::DBusConnection>> = const { RefCell::new(None) };
    static REDRAW: RefCell<Vec<glib::WeakRef<gtk::Widget>>> = const { RefCell::new(Vec::new()) };
}

/// The theme charts should draw with. Reflects the last [`set`]/[`load`].
pub fn active() -> &'static Theme {
    ACTIVE.with(|a| *a.borrow())
}

/// Queue a redraw on `widget` whenever the applied theme changes. Weak
/// registration: dead widgets are pruned on the next theme change, so a
/// registered widget never outlives its window through this list.
pub fn register_redraw(widget: &impl IsA<gtk::Widget>) {
    let weak = widget.upcast_ref::<gtk::Widget>().downgrade();
    REDRAW.with(|r| r.borrow_mut().push(weak));
}

/// The portal's `color-scheme` values (org.freedesktop.appearance): 0 no
/// preference, 1 prefer dark, 2 prefer light. Matching AdwStyleManager,
/// only an explicit 1 is dark, so behaviour under GNOME is unchanged.
fn portal_scheme_is_dark(scheme: u32) -> bool {
    scheme == 1
}

/// Ask the settings portal for the current colour scheme. `None` on any
/// failure (no portal backend, no bus): the caller keeps the dark default.
fn read_portal_scheme(conn: &gio::DBusConnection) -> Option<u32> {
    let args = ("org.freedesktop.appearance", "color-scheme").to_variant();
    let reply_ty = glib::VariantTy::new("(v)").ok()?;
    let call = |method: &str| {
        conn.call_sync(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            method,
            Some(&args),
            Some(reply_ty),
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
        )
    };
    match call("ReadOne") {
        Ok(reply) => reply.child_value(0).as_variant()?.get::<u32>(),
        // Portals older than the ReadOne addition answer Read, which wraps
        // the value in a second layer of variant.
        Err(_) => call("Read")
            .ok()?
            .child_value(0)
            .as_variant()?
            .as_variant()?
            .get::<u32>(),
    }
}

/// Read the desktop's dark preference from `org.freedesktop.portal.Settings`
/// and keep following it. Replaces `adw::StyleManager`: the initial read is
/// synchronous (once, at startup, before the first frame), the subscription
/// re-resolves only the follow-system selection, and every failure path
/// leaves the dark default in place.
fn watch_system_dark() {
    let Ok(conn) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    if let Some(scheme) = read_portal_scheme(&conn) {
        SYSTEM_DARK.with(|d| d.set(portal_scheme_is_dark(scheme)));
    }
    conn.signal_subscribe(
        Some("org.freedesktop.portal.Desktop"),
        Some("org.freedesktop.portal.Settings"),
        Some("SettingChanged"),
        Some("/org/freedesktop/portal/desktop"),
        None,
        gio::DBusSignalFlags::NONE,
        |_, _, _, _, _, params| {
            let ns = params.child_value(0).get::<String>();
            let key = params.child_value(1).get::<String>();
            if ns.as_deref() != Some("org.freedesktop.appearance")
                || key.as_deref() != Some("color-scheme")
            {
                return;
            }
            let dark = params
                .child_value(2)
                .as_variant()
                .and_then(|v| v.get::<u32>())
                .map(portal_scheme_is_dark)
                .unwrap_or(true);
            SYSTEM_DARK.with(|d| d.set(dark));
            // Only the follow-system selection re-resolves on a system
            // flip; fixed themes hold their polarity.
            SELECTION.with(|s| {
                if *s.borrow() == SYSTEM_ID {
                    apply(resolve(SYSTEM_ID, dark));
                }
            });
        },
    );
    BUS.with(|b| *b.borrow_mut() = Some(conn));
}

/// Parse a `#rrggbb` string to an RGBA (opaque), black on malformed input.
pub fn rgba(hex: &str) -> gdk::RGBA {
    let byte = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .map(|b| b as f32 / 255.0)
            .unwrap_or(0.0)
    };
    gdk::RGBA::new(byte(1), byte(3), byte(5), 1.0)
}

// Phase 6a reference-sheet spike: previews the Hyprland-native target look
// (flat, square, 1px hard borders, hidden window buttons) as overrides on
// top of the still-present adwaita sheet. Gated on COLOPHON_FLAT so default
// behaviour is untouched; deleted when the owned stylesheet lands (6c).
fn flat_css(t: &Theme) -> String {
    format!(
        "\
window.csd {{ border-radius: 0; box-shadow: none; }}
headerbar {{ background: {bg_header}; background-image: none; box-shadow: none;
  border-bottom: 1px solid {grid}; min-height: 34px; }}
headerbar windowcontrols {{ opacity: 0; min-width: 0; margin: 0; }}
button, entry, row, toast, popover > contents, scrollbar slider,
progressbar trough, progressbar progress, .card, .pill,
list.boxed-list {{ border-radius: 0; }}
button {{ box-shadow: none; }}
.card, list.boxed-list {{ border: 1px solid {grid}; box-shadow: none; }}
popover > contents {{ border: 1px solid {grid}; box-shadow: none; }}
",
        bg_header = t.bg_header,
        grid = t.grid,
    )
}

fn css(t: &Theme) -> String {
    format!(
        "\
:root {{
  --window-bg-color: {bg_window};
  --window-fg-color: {fg};
  --view-bg-color: {bg};
  --view-fg-color: {fg};
  --headerbar-bg-color: {bg_header};
  --headerbar-fg-color: {fg};
  --sidebar-bg-color: {bg};
  --sidebar-fg-color: {fg};
  --card-bg-color: {bg_card};
  --card-fg-color: {fg};
  --popover-bg-color: {bg_card};
  --popover-fg-color: {fg};
  --dialog-bg-color: {bg_card};
  --dialog-fg-color: {fg};
  --accent-bg-color: {accent};
  --accent-fg-color: {on_accent};
  --accent-color: {accent};
  --warning-bg-color: {warn};
  --warning-fg-color: {on_accent};
  --warning-color: {warn};
  --error-bg-color: {err};
  --error-fg-color: {on_accent};
  --error-color: {err};
  --destructive-bg-color: {err};
  --destructive-fg-color: {on_accent};
  --destructive-color: {err};
  --success-bg-color: {ok};
  --success-fg-color: {on_accent};
  --success-color: {ok};
}}
.book-row .stats {{ color: {fg_dim}; }}
.group-header label {{ color: {heading}; font-weight: 600; }}
.group-member {{ border-left: 2px solid {grid}; }}
",
        bg = t.bg,
        bg_window = t.bg_window,
        bg_header = t.bg_header,
        bg_card = t.bg_card,
        fg = t.fg,
        fg_dim = t.fg_dim,
        heading = t.heading,
        accent = t.accent,
        on_accent = t.on_accent,
        warn = t.warn,
        err = t.err,
        ok = t.ok,
        grid = t.grid,
    )
}

fn css_with_overrides(t: &Theme) -> String {
    let mut sheet = css(t);
    if std::env::var_os("COLOPHON_FLAT").is_some() {
        sheet.push_str(&flat_css(t));
    }
    sheet
}

fn apply(t: &'static Theme) {
    ACTIVE.with(|a| *a.borrow_mut() = t);
    let Some(display) = gdk::Display::default() else {
        return;
    };
    // Flip GTK's default-theme variant too, so widget internals the owned
    // sheet doesn't reach (text selection, spinners, dialog guts) follow
    // the theme's polarity.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(t.dark);
    }
    PROVIDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(old) = slot.take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css_with_overrides(t));
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        *slot = Some(provider);
    });
    REDRAW.with(|r| {
        r.borrow_mut()
            .retain(|w| w.upgrade().map(|w| w.queue_draw()).is_some())
    });
}

/// Apply a theme selection (`SYSTEM_ID` or a theme id) and install its
/// palette; registered chart widgets are redrawn automatically. Data-bearing
/// surfaces are refreshed by the caller (the window repaints the visible
/// page).
pub fn set(selection: &str) {
    SELECTION.with(|s| *s.borrow_mut() = selection.to_string());
    apply(resolve(selection, SYSTEM_DARK.with(Cell::get)));
}

/// Install the initial theme and keep `SYSTEM_ID` in sync with the desktop
/// preference. Call once from `Application::connect_startup`.
pub fn load(selection: &str) {
    watch_system_dark();
    set(selection);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_defaults_exist() {
        let mut ids: Vec<&str> = THEMES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate theme id");
        assert!(by_id(DEFAULT_DARK).is_some_and(|t| t.dark));
        assert!(by_id(DEFAULT_LIGHT).is_some_and(|t| !t.dark));
    }

    #[test]
    fn portal_scheme_maps_like_adwaita() {
        // 1 = prefer dark; 0 (no preference) and 2 (prefer light) are
        // light, and unknown future values must not read as dark.
        assert!(portal_scheme_is_dark(1));
        assert!(!portal_scheme_is_dark(0));
        assert!(!portal_scheme_is_dark(2));
        assert!(!portal_scheme_is_dark(3));
    }

    #[test]
    fn system_resolves_by_polarity() {
        assert_eq!(resolve(SYSTEM_ID, true).id, DEFAULT_DARK);
        assert_eq!(resolve(SYSTEM_ID, false).id, DEFAULT_LIGHT);
        assert_eq!(resolve("nord", true).id, "nord");
        // Unknown id falls back to the dark default rather than panicking.
        assert_eq!(resolve("bogus", true).id, DEFAULT_DARK);
    }

    #[test]
    fn every_theme_parses_and_generates_css() {
        for t in THEMES {
            assert_eq!(rgba(t.accent).alpha(), 1.0);
            let sheet = css(t);
            assert!(sheet.contains(t.accent));
            assert!(sheet.contains("--window-bg-color"));
        }
    }
}

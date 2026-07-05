//! Theming. A registry of named palettes (Kanagawa, Gruvbox, Nord, Rosé
//! Pine, Solarized) plus a "Follow system" mode that tracks the desktop's
//! light/dark preference. One [`Theme`] drives both the libadwaita CSS
//! variables and the chart colors, so a single definition themes the whole
//! app; `charts` reads [`active`] at draw time.

use std::cell::RefCell;

use gtk::gdk;

/// A named palette. The colour roles feed both the generated adwaita CSS
/// and the cairo chart widgets.
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    /// Polarity: forces the libadwaita colour-scheme so stock widgets match.
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
}

/// The theme charts should draw with. Reflects the last [`set`]/[`load`].
pub fn active() -> &'static Theme {
    ACTIVE.with(|a| *a.borrow())
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

fn apply(t: &'static Theme) {
    ACTIVE.with(|a| *a.borrow_mut() = t);
    let Some(display) = gdk::Display::default() else {
        return;
    };
    PROVIDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(old) = slot.take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css(t));
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        *slot = Some(provider);
    });
}

/// Apply a theme selection (`SYSTEM_ID` or a theme id): force libadwaita's
/// colour-scheme to the theme's polarity (or follow the system in
/// `SYSTEM_ID`), then install its palette. Charts must be redrawn by the
/// caller afterwards (the window refreshes the visible page).
pub fn set(selection: &str) {
    SELECTION.with(|s| *s.borrow_mut() = selection.to_string());
    let sm = adw::StyleManager::default();
    if selection == SYSTEM_ID {
        sm.set_color_scheme(adw::ColorScheme::Default);
    } else {
        let dark = by_id(selection).map(|t| t.dark).unwrap_or(true);
        sm.set_color_scheme(if dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
    }
    apply(resolve(selection, sm.is_dark()));
}

/// Install the initial theme and keep `SYSTEM_ID` in sync with the desktop
/// preference. Call once from `Application::connect_startup`.
pub fn load(selection: &str) {
    set(selection);
    adw::StyleManager::default().connect_dark_notify(|sm| {
        // Only the follow-system selection re-resolves on a system flip;
        // fixed themes hold their polarity.
        SELECTION.with(|s| {
            if *s.borrow() == SYSTEM_ID {
                apply(resolve(SYSTEM_ID, sm.is_dark()));
            }
        });
    });
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

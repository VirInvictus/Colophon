//! Kanagawa Dragon theming. Follows the system light/dark preference:
//! the full Dragon sheet applies when dark, an accent-only sheet on stock
//! Adwaita light (Dragon has no light variant to fake). The active
//! provider is swapped on the StyleManager's `dark` notify.

use std::cell::RefCell;

use gtk::gdk;

const DARK_CSS: &str = include_str!("theme.css");
const LIGHT_CSS: &str = include_str!("theme-light.css");

/// Canonical Kanagawa Dragon palette, exported for Phase 3 chart ramps.
#[allow(dead_code)]
pub mod palette {
    pub const BLACK0: &str = "#0d0c0c";
    pub const BLACK1: &str = "#12120f";
    pub const BLACK2: &str = "#1d1c19";
    pub const BLACK3: &str = "#181616";
    pub const BLACK4: &str = "#282727";
    pub const BLACK5: &str = "#393836";
    pub const BLACK6: &str = "#625e5a";
    pub const WHITE: &str = "#c5c9c5";
    pub const OLD_WHITE: &str = "#c8c093";
    pub const FUJI_WHITE: &str = "#dcd7ba";
    pub const GRAY: &str = "#a6a69c";
    pub const GRAY2: &str = "#9e9b93";
    pub const GRAY3: &str = "#7a8382";
    pub const GREEN: &str = "#87a987";
    pub const GREEN2: &str = "#8a9a7b";
    pub const PINK: &str = "#a292a3";
    pub const ORANGE: &str = "#b6927b";
    pub const ORANGE2: &str = "#b98d7b";
    pub const BLUE: &str = "#658594";
    pub const BLUE2: &str = "#8ba4b0";
    pub const VIOLET: &str = "#8992a7";
    pub const RED: &str = "#c4746e";
    pub const AQUA: &str = "#8ea4a2";
    pub const ASH: &str = "#737c73";
    pub const TEAL: &str = "#949fb5";
    pub const YELLOW: &str = "#c4b28a";
}

thread_local! {
    static ACTIVE: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

fn apply(dark: bool) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        if let Some(old) = active.take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }
        let provider = gtk::CssProvider::new();
        provider.load_from_string(if dark { DARK_CSS } else { LIGHT_CSS });
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        *active = Some(provider);
    });
}

/// Install the theme and keep it in sync with the system preference.
/// Call once from `Application::connect_startup`.
pub fn load() {
    let style_manager = adw::StyleManager::default();
    apply(style_manager.is_dark());
    style_manager.connect_dark_notify(|sm| apply(sm.is_dark()));
}

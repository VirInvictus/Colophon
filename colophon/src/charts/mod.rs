//! Custom-drawn chart widgets (the Phase 3 charting decision: cairo on
//! `GtkDrawingArea`, no charting crate). Shared here: palette parsing,
//! the light/dark ramp, and text helpers. Text uses cairo's toy API,
//! which is fine for the short unshaped strings charts need (digits,
//! month abbreviations) and keeps pangocairo out of the dependency tree.

pub mod bar;
pub mod heatmap;
pub mod hour_heatmap;
pub mod line;
pub mod page_activity;
pub mod span_bar;

pub use bar::BarChart;
pub use heatmap::YearHeatmap;
pub use hour_heatmap::HourHeatmap;
pub use line::LineChart;
pub use page_activity::PageActivityStrip;
pub use span_bar::SpanBar;

use gtk::cairo;
use gtk::gdk;

use crate::theme::palette;

pub fn rgba(hex: &str) -> gdk::RGBA {
    let parse = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0) as f32 / 255.0;
    gdk::RGBA::new(parse(1), parse(3), parse(5), 1.0)
}

pub fn is_dark() -> bool {
    adw::StyleManager::default().is_dark()
}

pub fn accent(dark: bool) -> gdk::RGBA {
    rgba(if dark { palette::BLUE2 } else { palette::BLUE })
}

pub fn highlight(dark: bool) -> gdk::RGBA {
    rgba(if dark {
        palette::ORANGE
    } else {
        palette::ORANGE2
    })
}

pub fn text(dark: bool) -> gdk::RGBA {
    if dark {
        rgba(palette::WHITE)
    } else {
        gdk::RGBA::new(0.2, 0.2, 0.2, 1.0)
    }
}

pub fn muted(dark: bool) -> gdk::RGBA {
    rgba(if dark { palette::GRAY } else { palette::GRAY3 })
}

/// Empty-cell color for heat grids.
pub fn cell_bg(dark: bool) -> gdk::RGBA {
    if dark {
        rgba(palette::BLACK4)
    } else {
        gdk::RGBA::new(0.906, 0.906, 0.89, 1.0)
    }
}

/// Quantized heat ramp: level 0 is the empty cell, 1..=4 interpolate
/// toward the accent. Discrete levels on purpose (KoShelf-style);
/// continuous alpha hides magnitude.
pub fn heat(level: u8, dark: bool) -> gdk::RGBA {
    if level == 0 {
        return cell_bg(dark);
    }
    let t = f32::from(level.min(4)) / 4.0; // 0.25, 0.50, 0.75, 1.00
    let from = cell_bg(dark);
    let to = accent(dark);
    let mix = |a: f32, b: f32| a + (b - a) * t;
    gdk::RGBA::new(
        mix(from.red(), to.red()),
        mix(from.green(), to.green()),
        mix(from.blue(), to.blue()),
        1.0,
    )
}

/// KoShelf's quantizer: 0 for no activity, else 1..=4 scaled to the max.
pub fn heat_level(value: i64, max: i64) -> u8 {
    if value <= 0 || max <= 0 {
        return 0;
    }
    ((value as f64 / max as f64 * 4.0).ceil() as u8).clamp(1, 4)
}

pub fn set_source(cr: &cairo::Context, color: gdk::RGBA) {
    cr.set_source_rgba(
        color.red() as f64,
        color.green() as f64,
        color.blue() as f64,
        color.alpha() as f64,
    );
}

/// Toy-API text with a generic family; never assumes an installed font.
pub fn draw_text(cr: &cairo::Context, x: f64, y: f64, size: f64, color: gdk::RGBA, s: &str) {
    set_source(cr, color);
    cr.select_font_face(
        "sans-serif",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(size);
    cr.move_to(x, y);
    let _ = cr.show_text(s);
}

/// Width of `s` at `size`, for centering.
pub fn text_width(cr: &cairo::Context, size: f64, s: &str) -> f64 {
    cr.select_font_face(
        "sans-serif",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    cr.set_font_size(size);
    cr.text_extents(s).map(|e| e.width()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::heat_level;

    #[test]
    fn heat_level_quantizes_like_koshelf() {
        assert_eq!(heat_level(0, 100), 0);
        assert_eq!(heat_level(1, 100), 1);
        assert_eq!(heat_level(25, 100), 1);
        assert_eq!(heat_level(26, 100), 2);
        assert_eq!(heat_level(100, 100), 4);
        assert_eq!(heat_level(50, 0), 0);
    }
}

//! Weekday x hour-of-day heatmap: "when do I read" over the whole
//! history (spec.md Tier A #2). Rows are Mon..Sun, columns 00..23,
//! quantized intensity, per-cell tooltips.

use std::cell::RefCell;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

use crate::fmt::humanize_secs;

const CELL_W: f64 = 22.0;
const CELL_H: f64 = 16.0;
const GAP: f64 = 3.0;
const LEFT: f64 = 34.0;
const TOP: f64 = 16.0;

const DAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct HourHeatmap {
        pub grid: RefCell<[[i64; 24]; 7]>,
        pub max: std::cell::Cell<i64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HourHeatmap {
        const NAME: &'static str = "HourHeatmap";
        type Type = super::HourHeatmap;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for HourHeatmap {
        fn constructed(&self) {
            self.parent_constructed();
            let widget = self.obj();
            widget.set_content_width((LEFT + (CELL_W + GAP) * 24.0) as i32);
            widget.set_content_height((TOP + (CELL_H + GAP) * 7.0) as i32);
            widget.set_draw_func(glib::clone!(
                #[weak(rename_to = this)]
                widget,
                move |_, cr, w, h| this.draw(cr, w, h)
            ));
            widget.set_has_tooltip(true);
            widget.connect_query_tooltip(|this, x, y, _, tooltip| {
                match this.tooltip_at(f64::from(x), f64::from(y)) {
                    Some(text) => {
                        tooltip.set_text(Some(&text));
                        true
                    }
                    None => false,
                }
            });
            let weak = widget.downgrade();
            adw::StyleManager::default().connect_dark_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.queue_draw();
                }
            });
        }
    }
    impl WidgetImpl for HourHeatmap {}
    impl DrawingAreaImpl for HourHeatmap {}
}

glib::wrapper! {
    pub struct HourHeatmap(ObjectSubclass<imp::HourHeatmap>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for HourHeatmap {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl HourHeatmap {
    pub fn set_grid(&self, grid: [[i64; 24]; 7]) {
        let max = grid.iter().flatten().copied().max().unwrap_or(0);
        self.imp().max.set(max);
        self.imp().grid.replace(grid);
        self.queue_draw();
    }

    fn draw(&self, cr: &gtk::cairo::Context, _w: i32, _h: i32) {
        let grid = self.imp().grid.borrow();
        let max = self.imp().max.get();
        let dark = super::is_dark();

        for (row, label) in DAY_LABELS.iter().enumerate() {
            super::draw_text(
                cr,
                2.0,
                TOP + (CELL_H + GAP) * row as f64 + CELL_H - 3.5,
                9.0,
                super::muted(dark),
                label,
            );
        }
        for hour in [0usize, 6, 12, 18] {
            super::draw_text(
                cr,
                LEFT + (CELL_W + GAP) * hour as f64,
                TOP - 6.0,
                9.0,
                super::muted(dark),
                &format!("{hour:02}"),
            );
        }

        for (row, hours) in grid.iter().enumerate() {
            for (hour, &secs) in hours.iter().enumerate() {
                let level = super::heat_level(secs, max);
                super::set_source(cr, super::heat(level, dark));
                cr.rectangle(
                    LEFT + (CELL_W + GAP) * hour as f64,
                    TOP + (CELL_H + GAP) * row as f64,
                    CELL_W,
                    CELL_H,
                );
                let _ = cr.fill();
            }
        }
    }

    fn tooltip_at(&self, x: f64, y: f64) -> Option<String> {
        let col = ((x - LEFT) / (CELL_W + GAP)).floor();
        let row = ((y - TOP) / (CELL_H + GAP)).floor();
        if !(0.0..24.0).contains(&col) || !(0.0..7.0).contains(&row) {
            return None;
        }
        let (row, col) = (row as usize, col as usize);
        let secs = self.imp().grid.borrow()[row][col];
        Some(if secs > 0 {
            format!(
                "{} {:02}:00 \u{b7} {}",
                DAY_LABELS[row],
                col,
                humanize_secs(secs)
            )
        } else {
            format!("{} {:02}:00 \u{b7} no reading", DAY_LABELS[row], col)
        })
    }
}

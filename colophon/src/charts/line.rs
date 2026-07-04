//! Area/line trend chart for time series (spec.md Tier A #1, the
//! reading-speed trend). Nearest-point tooltips; x labels at the range
//! ends, y implied by the point tooltips (KoInsight/KoShelf convention:
//! axes stay quiet, hover carries the numbers).

use std::cell::RefCell;

use adw::subclass::prelude::*;
use chrono::NaiveDate;
use gtk::glib;
use gtk::prelude::*;

use crate::fmt::short_date;

#[derive(Debug, Clone)]
pub struct Point {
    pub date: NaiveDate,
    pub value: f64,
    /// Pre-formatted tooltip line for this point.
    pub display: String,
}

const HEIGHT: i32 = 150;
const PAD_TOP: f64 = 10.0;
const PAD_BOTTOM: f64 = 18.0;
const PAD_X: f64 = 6.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct LineChart {
        pub points: RefCell<Vec<Point>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LineChart {
        const NAME: &'static str = "LineChart";
        type Type = super::LineChart;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for LineChart {
        fn constructed(&self) {
            self.parent_constructed();
            let widget = self.obj();
            widget.set_content_height(HEIGHT);
            widget.set_hexpand(true);
            widget.set_draw_func(glib::clone!(
                #[weak(rename_to = this)]
                widget,
                move |_, cr, w, h| this.draw(cr, w, h)
            ));
            widget.set_has_tooltip(true);
            widget.connect_query_tooltip(|this, x, y, _, tooltip| {
                let _ = y;
                match this.tooltip_at(f64::from(x)) {
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
    impl WidgetImpl for LineChart {}
    impl DrawingAreaImpl for LineChart {}
}

glib::wrapper! {
    pub struct LineChart(ObjectSubclass<imp::LineChart>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for LineChart {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl LineChart {
    /// Points must be in date order.
    pub fn set_points(&self, points: Vec<Point>) {
        self.imp().points.replace(points);
        self.queue_draw();
    }

    fn x_of(&self, index: usize, count: usize, width: f64) -> f64 {
        if count <= 1 {
            return width / 2.0;
        }
        PAD_X + (width - 2.0 * PAD_X) * index as f64 / (count - 1) as f64
    }

    fn draw(&self, cr: &gtk::cairo::Context, w: i32, h: i32) {
        let points = self.imp().points.borrow();
        if points.is_empty() {
            return;
        }
        let dark = super::is_dark();
        let w = f64::from(w);
        let h = f64::from(h);
        let max = points.iter().map(|p| p.value).fold(0.0, f64::max);
        let baseline = h - PAD_BOTTOM;
        let plot = baseline - PAD_TOP;

        let y_of = |value: f64| -> f64 {
            if max > 0.0 {
                baseline - plot * (value / max)
            } else {
                baseline
            }
        };

        // Baseline rule.
        super::set_source(cr, super::cell_bg(dark));
        cr.rectangle(0.0, baseline, w, 1.0);
        let _ = cr.fill();

        let accent = super::accent(dark);
        let n = points.len();

        // Area fill under the line, at low alpha.
        cr.move_to(self.x_of(0, n, w), baseline);
        for (i, point) in points.iter().enumerate() {
            cr.line_to(self.x_of(i, n, w), y_of(point.value));
        }
        cr.line_to(self.x_of(n - 1, n, w), baseline);
        cr.close_path();
        cr.set_source_rgba(
            f64::from(accent.red()),
            f64::from(accent.green()),
            f64::from(accent.blue()),
            0.25,
        );
        let _ = cr.fill();

        // The line itself plus point dots.
        super::set_source(cr, accent);
        cr.set_line_width(2.0);
        for (i, point) in points.iter().enumerate() {
            let (x, y) = (self.x_of(i, n, w), y_of(point.value));
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
        let _ = cr.stroke();
        for (i, point) in points.iter().enumerate() {
            cr.arc(
                self.x_of(i, n, w),
                y_of(point.value),
                2.5,
                0.0,
                std::f64::consts::TAU,
            );
            let _ = cr.fill();
        }

        // Range labels at the ends.
        let first = short_date(points[0].date);
        super::draw_text(cr, PAD_X, h - 4.0, 10.0, super::muted(dark), &first);
        if n > 1 {
            let last = short_date(points[n - 1].date);
            let tw = super::text_width(cr, 10.0, &last);
            super::draw_text(cr, w - PAD_X - tw, h - 4.0, 10.0, super::muted(dark), &last);
        }
    }

    fn tooltip_at(&self, x: f64) -> Option<String> {
        let points = self.imp().points.borrow();
        if points.is_empty() {
            return None;
        }
        let w = f64::from(self.width());
        let n = points.len();
        let nearest = (0..n).min_by_key(|&i| (self.x_of(i, n, w) - x).abs() as i64)?;
        let point = &points[nearest];
        Some(format!(
            "{} \u{b7} {}",
            short_date(point.date),
            point.display
        ))
    }
}

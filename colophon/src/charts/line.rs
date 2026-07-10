//! Area/line trend chart for time series (spec.md Tier A #1, the
//! reading-speed trend). Supports a muted comparison series behind the
//! primary one (book vs library baseline); x positions scale by date so
//! differently-sampled series align. Nearest-point tooltips on the
//! primary series; x labels at the range ends (KoInsight/KoShelf
//! convention: axes stay quiet, hover carries the numbers).

use std::cell::RefCell;

use chrono::NaiveDate;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::fmt::short_date;

#[derive(Debug, Clone)]
pub struct Point {
    pub date: NaiveDate,
    pub value: f64,
    /// Pre-formatted tooltip line for this point.
    pub display: String,
}

#[derive(Debug, Clone)]
pub struct Series {
    pub points: Vec<Point>,
    /// Muted series render behind the primary: thinner, quieter, no
    /// dots, no tooltips.
    pub muted: bool,
}

const HEIGHT: i32 = 150;
const PAD_TOP: f64 = 10.0;
const PAD_BOTTOM: f64 = 18.0;
const PAD_X: f64 = 6.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct LineChart {
        pub series: RefCell<Vec<Series>>,
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
            widget.connect_query_tooltip(|this, x, _, _, tooltip| {
                match this.tooltip_at(f64::from(x)) {
                    Some(text) => {
                        tooltip.set_text(Some(&text));
                        true
                    }
                    None => false,
                }
            });
            crate::theme::register_redraw(&*widget);
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
    /// Single primary series; points must be in date order.
    pub fn set_points(&self, points: Vec<Point>) {
        self.set_series(vec![Series {
            points,
            muted: false,
        }]);
    }

    /// Every series' points must be in date order.
    pub fn set_series(&self, series: Vec<Series>) {
        self.imp().series.replace(series);
        self.queue_draw();
    }

    fn date_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        let series = self.imp().series.borrow();
        let min = series
            .iter()
            .filter_map(|s| s.points.first())
            .map(|p| p.date)
            .min()?;
        let max = series
            .iter()
            .filter_map(|s| s.points.last())
            .map(|p| p.date)
            .max()?;
        Some((min, max))
    }

    fn x_of(&self, date: NaiveDate, range: (NaiveDate, NaiveDate), width: f64) -> f64 {
        let span = (range.1 - range.0).num_days();
        if span <= 0 {
            return width / 2.0;
        }
        PAD_X + (width - 2.0 * PAD_X) * (date - range.0).num_days() as f64 / span as f64
    }

    fn draw(&self, cr: &gtk::cairo::Context, w: i32, h: i32) {
        let series = self.imp().series.borrow();
        let Some(range) = self.date_range() else {
            return;
        };
        let dark = super::is_dark();
        let w = f64::from(w);
        let h = f64::from(h);
        let max = series
            .iter()
            .flat_map(|s| &s.points)
            .map(|p| p.value)
            .fold(0.0, f64::max);
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

        // Muted comparison series first, primaries on top.
        let mut ordered: Vec<&Series> = series.iter().filter(|s| s.muted).collect();
        ordered.extend(series.iter().filter(|s| !s.muted));

        for s in ordered {
            if s.points.is_empty() {
                continue;
            }
            let color = if s.muted {
                super::muted(dark)
            } else {
                super::accent(dark)
            };

            if !s.muted {
                // Area fill under the primary line only.
                cr.move_to(self.x_of(s.points[0].date, range, w), baseline);
                for point in &s.points {
                    cr.line_to(self.x_of(point.date, range, w), y_of(point.value));
                }
                cr.line_to(
                    self.x_of(s.points[s.points.len() - 1].date, range, w),
                    baseline,
                );
                cr.close_path();
                cr.set_source_rgba(
                    f64::from(color.red()),
                    f64::from(color.green()),
                    f64::from(color.blue()),
                    0.25,
                );
                let _ = cr.fill();
            }

            super::set_source(cr, color);
            cr.set_line_width(if s.muted { 1.5 } else { 2.0 });
            for (i, point) in s.points.iter().enumerate() {
                let (x, y) = (self.x_of(point.date, range, w), y_of(point.value));
                if i == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();

            if !s.muted {
                for point in &s.points {
                    cr.arc(
                        self.x_of(point.date, range, w),
                        y_of(point.value),
                        2.5,
                        0.0,
                        std::f64::consts::TAU,
                    );
                    let _ = cr.fill();
                }
            }
        }

        // Range labels at the ends.
        let first = short_date(range.0);
        super::draw_text(cr, PAD_X, h - 4.0, 10.0, super::muted(dark), &first);
        if range.1 > range.0 {
            let last = short_date(range.1);
            let tw = super::text_width(cr, 10.0, &last);
            super::draw_text(cr, w - PAD_X - tw, h - 4.0, 10.0, super::muted(dark), &last);
        }
    }

    fn tooltip_at(&self, x: f64) -> Option<String> {
        let series = self.imp().series.borrow();
        let range = self.date_range()?;
        let w = f64::from(self.width());
        let primary = series.iter().find(|s| !s.muted)?;
        let point = primary
            .points
            .iter()
            .min_by_key(|p| (self.x_of(p.date, range, w) - x).abs() as i64)?;
        Some(format!(
            "{} \u{b7} {}",
            short_date(point.date),
            point.display
        ))
    }
}

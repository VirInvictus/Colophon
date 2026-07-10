//! GitHub-style year heatmap: Monday-start weeks as columns, quantized
//! intensity levels (spec.md Tier B #7), tooltip per day.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::fmt::{humanize_secs, short_date};

const CELL: f64 = 11.0;
const GAP: f64 = 3.0;
const LEFT: f64 = 22.0;
const TOP: f64 = 18.0;
const MIN_WEEKS: i64 = 8;
const MAX_WEEKS: i64 = 52;

#[derive(Default)]
pub struct Data {
    /// seconds, distinct pages per day
    days: BTreeMap<NaiveDate, (i64, u32)>,
    max_secs: i64,
    /// Monday of the leftmost column.
    grid_start: Option<NaiveDate>,
    today: Option<NaiveDate>,
    weeks: i64,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct YearHeatmap {
        pub data: RefCell<Data>,
        pub dark: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for YearHeatmap {
        const NAME: &'static str = "YearHeatmap";
        type Type = super::YearHeatmap;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for YearHeatmap {
        fn constructed(&self) {
            self.parent_constructed();
            let widget = self.obj();
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
            crate::theme::register_redraw(&*widget);
        }
    }
    impl WidgetImpl for YearHeatmap {}
    impl DrawingAreaImpl for YearHeatmap {}
}

glib::wrapper! {
    pub struct YearHeatmap(ObjectSubclass<imp::YearHeatmap>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for YearHeatmap {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl YearHeatmap {
    /// Feeds per-day totals; the grid spans up to a year of Monday-start
    /// weeks ending at `today`, shrinking (min 8 weeks) for young data.
    pub fn set_data(&self, daily: &BTreeMap<NaiveDate, colophon_core::DayTotal>, today: NaiveDate) {
        let days: BTreeMap<NaiveDate, (i64, u32)> = daily
            .iter()
            .map(|(d, t)| (*d, (t.seconds, t.pages)))
            .collect();
        let max_secs = days.values().map(|(s, _)| *s).max().unwrap_or(0);

        let this_monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        let weeks = match days.keys().next() {
            Some(first) => {
                let first_monday =
                    *first - Duration::days(first.weekday().num_days_from_monday() as i64);
                ((this_monday - first_monday).num_days() / 7 + 1).clamp(MIN_WEEKS, MAX_WEEKS)
            }
            None => MIN_WEEKS,
        };
        let grid_start = this_monday - Duration::days((weeks - 1) * 7);

        *self.imp().data.borrow_mut() = Data {
            days,
            max_secs,
            grid_start: Some(grid_start),
            today: Some(today),
            weeks,
        };
        self.set_content_width((LEFT + (CELL + GAP) * weeks as f64) as i32);
        self.set_content_height((TOP + (CELL + GAP) * 7.0) as i32);
        self.queue_draw();
    }

    fn draw(&self, cr: &gtk::cairo::Context, _w: i32, _h: i32) {
        let data = self.imp().data.borrow();
        let (Some(grid_start), Some(today)) = (data.grid_start, data.today) else {
            return;
        };
        let dark = super::is_dark();

        // Weekday guides.
        for (row, label) in [(0, "M"), (2, "W"), (4, "F")] {
            super::draw_text(
                cr,
                4.0,
                TOP + (CELL + GAP) * f64::from(row) + CELL - 1.5,
                9.0,
                super::muted(dark),
                label,
            );
        }

        let mut last_month = 0;
        for col in 0..data.weeks {
            let x = LEFT + (CELL + GAP) * col as f64;
            let week_start = grid_start + Duration::days(col * 7);

            // Month label above the column where the month changes.
            if week_start.month() != last_month {
                if last_month != 0 || week_start.day() <= 7 || col == 0 {
                    super::draw_text(
                        cr,
                        x,
                        TOP - 6.0,
                        9.0,
                        super::muted(dark),
                        &month_abbr(week_start.month()),
                    );
                }
                last_month = week_start.month();
            }

            for row in 0..7 {
                let date = week_start + Duration::days(row);
                if date > today {
                    continue;
                }
                let secs = data.days.get(&date).map(|(s, _)| *s).unwrap_or(0);
                let level = super::heat_level(secs, data.max_secs);
                super::set_source(cr, super::heat(level, dark));
                cr.rectangle(x, TOP + (CELL + GAP) * row as f64, CELL, CELL);
                let _ = cr.fill();
            }
        }
    }

    fn tooltip_at(&self, x: f64, y: f64) -> Option<String> {
        let data = self.imp().data.borrow();
        let grid_start = data.grid_start?;
        let today = data.today?;
        let col = ((x - LEFT) / (CELL + GAP)).floor();
        let row = ((y - TOP) / (CELL + GAP)).floor();
        if col < 0.0 || !(0.0..=6.0).contains(&row) || col >= data.weeks as f64 {
            return None;
        }
        let date = grid_start + Duration::days(col as i64 * 7 + row as i64);
        if date > today {
            return None;
        }
        Some(match data.days.get(&date) {
            Some((secs, pages)) => format!(
                "{} \u{b7} {} \u{b7} {} pages",
                short_date(date),
                humanize_secs(*secs),
                pages
            ),
            None => format!("{} \u{b7} no reading", short_date(date)),
        })
    }
}

fn month_abbr(month: u32) -> String {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month as usize) - 1]
        .to_owned()
}

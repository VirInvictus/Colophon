//! GitHub-style year heatmap: Monday-start weeks as columns, quantized
//! intensity levels (spec.md Tier B #10), tooltip per day.

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

#[derive(Default)]
pub struct Data {
    /// seconds, distinct pages, distinct books per day
    days: BTreeMap<NaiveDate, (i64, u32, u32)>,
    max_secs: i64,
    /// Monday of the leftmost column.
    grid_start: Option<NaiveDate>,
    /// The last day the grid covers: the selected year's December 31, so
    /// padding cells past the year's end stay empty.
    grid_end: Option<NaiveDate>,
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
            super::super::init_chart(
                &*widget,
                |this, cr, w, h| this.draw(cr, w, h),
                |this, x, y| this.tooltip_at(x, y),
            );
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
    /// Feeds per-day totals for one calendar year (spec.md "Year heatmap,
    /// year selection"): the grid spans January through December of
    /// `year`, Monday-start, up to the 53 weeks a year can span. Days the
    /// library has no data for render empty; cells past `today` are
    /// skipped so the current year grows in as the year does.
    pub fn set_data(
        &self,
        daily: &BTreeMap<NaiveDate, colophon_core::DayTotal>,
        today: NaiveDate,
        year: i32,
    ) {
        let year_start = NaiveDate::from_ymd_opt(year, 1, 1).expect("Jan 1 exists");
        let year_end = NaiveDate::from_ymd_opt(year, 12, 31).expect("Dec 31 exists");
        let days: BTreeMap<NaiveDate, (i64, u32, u32)> = daily
            .iter()
            .filter(|(d, _)| d.year() == year)
            .map(|(d, t)| (*d, (t.seconds, t.pages, t.books)))
            .collect();
        let max_secs = days.values().map(|(s, _, _)| *s).max().unwrap_or(0);

        let grid_start =
            year_start - Duration::days(year_start.weekday().num_days_from_monday() as i64);
        let weeks = (year_end - grid_start).num_days() / 7 + 1;

        *self.imp().data.borrow_mut() = Data {
            days,
            max_secs,
            grid_start: Some(grid_start),
            grid_end: Some(year_end),
            today: Some(today),
            weeks,
        };
        self.set_content_width((LEFT + (CELL + GAP) * weeks as f64) as i32);
        self.set_content_height((TOP + (CELL + GAP) * 7.0) as i32);
        self.queue_draw();
    }

    fn draw(&self, cr: &gtk::cairo::Context, _w: i32, _h: i32) {
        let data = self.imp().data.borrow();
        let (Some(grid_start), Some(grid_end), Some(today)) =
            (data.grid_start, data.grid_end, data.today)
        else {
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
                        crate::fmt::month_abbr(week_start.month()),
                    );
                }
                last_month = week_start.month();
            }

            for row in 0..7 {
                let date = week_start + Duration::days(row);
                if date > grid_end || date > today {
                    continue;
                }
                let secs = data.days.get(&date).map(|(s, _, _)| *s).unwrap_or(0);
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
        let grid_end = data.grid_end?;
        let today = data.today?;
        let col = ((x - LEFT) / (CELL + GAP)).floor();
        let row = ((y - TOP) / (CELL + GAP)).floor();
        if col < 0.0 || !(0.0..=6.0).contains(&row) || col >= data.weeks as f64 {
            return None;
        }
        let date = grid_start + Duration::days(col as i64 * 7 + row as i64);
        if date > grid_end || date > today {
            return None;
        }
        Some(day_tooltip(
            date,
            data.days.get(&date).map(|(s, p, b)| (*s, *p, *b)),
        ))
    }
}

/// One day's tooltip line (spec.md Tier B #10: time + pages + books).
fn day_tooltip(date: NaiveDate, total: Option<(i64, u32, u32)>) -> String {
    match total {
        Some((secs, pages, books)) => format!(
            "{} \u{b7} {} \u{b7} {} pages \u{b7} {} books",
            short_date(date),
            humanize_secs(secs),
            pages,
            books
        ),
        None => format!("{} \u{b7} no reading", short_date(date)),
    }
}

#[cfg(test)]
mod tests {
    use super::day_tooltip;
    use chrono::NaiveDate;

    #[test]
    fn tooltip_names_time_pages_and_books() {
        let date: NaiveDate = "2026-07-05".parse().unwrap();
        assert_eq!(
            day_tooltip(date, Some((7200, 40, 2))),
            "Jul 5 2026 \u{b7} 2h \u{b7} 40 pages \u{b7} 2 books"
        );
        assert_eq!(day_tooltip(date, None), "Jul 5 2026 \u{b7} no reading");
    }
}

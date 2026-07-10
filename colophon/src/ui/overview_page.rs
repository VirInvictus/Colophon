//! The "All Books" surface: library totals, streaks, and the
//! library-wide charts. Respects the junk filter by construction: the
//! window hands it the already-filtered entry set via `stats::overview`.

use std::cell::RefCell;

use chrono::NaiveDate;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::charts::bar::Bar;
use crate::charts::line::Point;
use crate::fmt::{humanize_secs, short_date};
use crate::stats::{Overview, SESSION_BUCKETS};
use crate::ui::rows;

type WindowChangedHandler = Box<dyn Fn()>;

mod imp {
    use super::*;
    use crate::charts::{BarChart, HourHeatmap, LineChart, YearHeatmap};
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "overview_page.ui")]
    pub struct OverviewPage {
        #[template_child]
        pub tiles: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub profile_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub profile_tiles: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub heatmap: TemplateChild<YearHeatmap>,
        #[template_child]
        pub hour_heatmap: TemplateChild<HourHeatmap>,
        #[template_child]
        pub speed_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub speed_chart: TemplateChild<LineChart>,
        #[template_child]
        pub session_caption: TemplateChild<gtk::Label>,
        #[template_child]
        pub session_chart: TemplateChild<BarChart>,
        #[template_child]
        pub session_starts_chart: TemplateChild<BarChart>,
        #[template_child]
        pub weekday_chart: TemplateChild<BarChart>,
        #[template_child]
        pub monthly_chart: TemplateChild<BarChart>,
        #[template_child]
        pub series_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub series_rows: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub author_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub author_rows: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub record_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub record_tiles: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub recap_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub recap_tiles: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub finished_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub finished_rows: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub forgotten_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub forgotten_rows: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub win_30: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub win_90: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub win_365: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub win_all: TemplateChild<gtk::ToggleButton>,
        pub on_window_changed: RefCell<Option<super::WindowChangedHandler>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OverviewPage {
        const NAME: &'static str = "OverviewPage";
        type Type = super::OverviewPage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
            crate::ui::clamp::Clamp::ensure_type();
            YearHeatmap::ensure_type();
            HourHeatmap::ensure_type();
            LineChart::ensure_type();
            BarChart::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for OverviewPage {
        fn constructed(&self) {
            self.parent_constructed();
            let page = self.obj();
            for button in [&self.win_30, &self.win_90, &self.win_365, &self.win_all] {
                button.connect_toggled(glib::clone!(
                    #[weak(rename_to = this)]
                    page,
                    move |button| {
                        // Each switch fires two toggled signals (off +
                        // on); recompute once, on the activation.
                        if button.is_active()
                            && let Some(handler) = this.imp().on_window_changed.borrow().as_ref()
                        {
                            handler();
                        }
                    }
                ));
            }
        }

        // Template children are parented to the template widget itself;
        // a plain gtk::Widget parent must unparent them on dispose.
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for OverviewPage {}
}

glib::wrapper! {
    pub struct OverviewPage(ObjectSubclass<imp::OverviewPage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

const WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

impl OverviewPage {
    /// The selected time window in days (`None` = all time).
    pub fn window_days(&self) -> Option<i64> {
        let imp = self.imp();
        if imp.win_30.is_active() {
            Some(30)
        } else if imp.win_90.is_active() {
            Some(90)
        } else if imp.win_365.is_active() {
            Some(365)
        } else {
            None
        }
    }

    pub fn set_on_window_changed(&self, handler: impl Fn() + 'static) {
        self.imp()
            .on_window_changed
            .replace(Some(Box::new(handler)));
    }

    pub fn set_data(&self, overview: &Overview, today: NaiveDate) {
        let imp = self.imp();

        imp.tiles.remove_all();
        let add_tile = |value: String, caption: &str, detail: Option<String>| {
            imp.tiles.append(&tile(&value, caption, detail.as_deref()));
        };
        // Total time carries a comparison to the previous equal-length
        // window when one is selected and had reading (spec.md).
        let total_detail = overview.period_delta.map(|d| {
            let pct = (d.pct * 100.0).round() as i64;
            let arrow = match pct.cmp(&0) {
                std::cmp::Ordering::Greater => '\u{2191}',
                std::cmp::Ordering::Less => '\u{2193}',
                std::cmp::Ordering::Equal => '\u{2192}',
            };
            format!("{arrow} {}% vs previous", pct.abs())
        });
        add_tile(
            humanize_secs(overview.total_secs),
            "total time",
            total_detail,
        );
        add_tile(overview.unique_pages.to_string(), "pages read", None);
        add_tile(overview.books.to_string(), "books", None);
        add_tile(overview.active_days.to_string(), "active days", None);
        add_tile(
            overview
                .streaks
                .current
                .map(|s| format!("{}d", s.days))
                .unwrap_or_else(|| "0d".into()),
            "current streak",
            overview
                .streaks
                .current
                .map(|s| format!("since {}", short_date(s.start))),
        );
        add_tile(
            overview
                .streaks
                .longest
                .map(|s| format!("{}d", s.days))
                .unwrap_or_else(|| "0d".into()),
            "longest streak",
            overview
                .streaks
                .longest
                .map(|s| format!("{} \u{2013} {}", short_date(s.start), short_date(s.end))),
        );
        if let Some((date, secs)) = overview.busiest {
            add_tile(humanize_secs(secs), "busiest day", Some(short_date(date)));
        }

        // Reading personality (spec.md "Reader profile"): three synthesised
        // traits, hidden when the window has too little reading to classify.
        imp.profile_tiles.remove_all();
        match crate::stats::reader_profile(overview) {
            Some(profile) => {
                imp.profile_title.set_visible(true);
                imp.profile_tiles.set_visible(true);
                let mut traits = vec![
                    &profile.chronotype,
                    &profile.session_style,
                    &profile.weekly_rhythm,
                ];
                // Variety is whole-library and only meaningful past a few
                // authors, so it is present only sometimes (spec.md).
                if let Some(variety) = &profile.variety {
                    traits.push(variety);
                }
                for t in traits {
                    imp.profile_tiles.append(&tile(t.label, &t.detail, None));
                }
            }
            None => {
                imp.profile_title.set_visible(false);
                imp.profile_tiles.set_visible(false);
            }
        }

        // Records (spec.md "Records"): all-time bests, hidden until there
        // is any reading. Unlike the totals tiles these are whole-history,
        // so they hold still when the window changes.
        imp.record_tiles.remove_all();
        let records = &overview.records;
        let has_records = !records.is_empty();
        imp.record_title.set_visible(has_records);
        imp.record_tiles.set_visible(has_records);
        if has_records {
            let longest_date = records.longest_session_date.map(short_date);
            imp.record_tiles.append(&tile(
                &humanize_secs(records.longest_session_secs),
                "longest session",
                longest_date.as_deref(),
            ));
            let biggest_date = records.biggest_day_date.map(short_date);
            imp.record_tiles.append(&tile(
                &humanize_secs(records.biggest_day_secs),
                "biggest day",
                biggest_date.as_deref(),
            ));
            let pages_date = records.most_pages_date.map(short_date);
            imp.record_tiles.append(&tile(
                &records.most_pages.to_string(),
                "most pages in a day",
                pages_date.as_deref(),
            ));
        }

        // Recap (spec.md "Recap"): a whole-history composite, so it stays put
        // (and stays meaningful) even when a shorter window is selected.
        imp.recap_tiles.remove_all();
        let recap = &overview.recap;
        let has_recap = !recap.is_empty();
        imp.recap_title.set_visible(has_recap);
        imp.recap_tiles.set_visible(has_recap);
        if has_recap {
            imp.recap_tiles.append(&tile(
                &recap.books_finished.to_string(),
                "books finished",
                None,
            ));
            if let Some(rate) = recap.completion_rate() {
                let started = format!(
                    "{} of {} started",
                    recap.books_finished, recap.books_started
                );
                imp.recap_tiles.append(&tile(
                    &format!("{}%", (rate * 100.0).round() as i64),
                    "completion",
                    Some(&started),
                ));
            }
            imp.recap_tiles
                .append(&tile(&humanize_secs(recap.total_secs), "total time", None));
            imp.recap_tiles.append(&tile(
                &format!("{}d", recap.longest_streak_days),
                "longest streak",
                None,
            ));
            imp.recap_tiles
                .append(&tile(&recap.sessions.to_string(), "sessions", None));
            if let Some((month, secs)) = recap.most_active_month {
                let secs_label = humanize_secs(secs);
                imp.recap_tiles.append(&tile(
                    &month_label(month),
                    "most active month",
                    Some(&secs_label),
                ));
            }
        }

        // Finished books (spec.md "Completions timeline"): completed works by
        // finish date, most recent first. Hidden until something is finished.
        imp.finished_rows.remove_all();
        let has_finished = !overview.finished_books.is_empty();
        imp.finished_title.set_visible(has_finished);
        imp.finished_rows.set_visible(has_finished);
        for f in overview.finished_books.iter().take(20) {
            let when = if f.from_completion {
                format!("finished {}", short_date(f.finish_date))
            } else {
                format!("last read {}", short_date(f.finish_date))
            };
            let subtitle = if f.author.trim().is_empty() {
                when
            } else {
                format!("{} \u{b7} {when}", f.author)
            };
            imp.finished_rows.append(&rows::value_row(
                &f.title,
                &humanize_secs(f.total_secs),
                Some(&subtitle),
            ));
        }

        imp.heatmap.set_data(&overview.daily, today);
        imp.hour_heatmap.set_grid(overview.hourly);

        imp.speed_title.set_text(match overview.speed_bucket {
            colophon_core::metrics::Bucket::Day => "Reading speed \u{b7} pages/hour by day",
            _ => "Reading speed \u{b7} pages/hour by week",
        });
        imp.speed_chart.set_points(
            overview
                .speed
                .iter()
                .map(|(date, point)| Point {
                    date: *date,
                    value: point.pages_per_hour,
                    display: format!(
                        "{:.0} pages/hour \u{b7} {} pages in {}",
                        point.pages_per_hour,
                        point.pages,
                        humanize_secs(point.seconds)
                    ),
                })
                .collect(),
        );

        let sessions = &overview.sessions;
        imp.session_caption.set_text(&format!(
            "{} sessions \u{b7} {:.1} per active day \u{b7} median {} \u{b7} longest {}{}",
            sessions.count,
            sessions.per_active_day,
            humanize_secs(sessions.median_secs),
            humanize_secs(sessions.longest_secs),
            sessions
                .longest_date
                .map(|d| format!(" ({})", short_date(d)))
                .unwrap_or_default(),
        ));
        imp.session_chart.set_bars(
            sessions
                .histogram
                .iter()
                .zip(SESSION_BUCKETS)
                .map(|(&count, (label, _))| Bar {
                    label: label.into(),
                    value: f64::from(count),
                    display: if count > 0 {
                        count.to_string()
                    } else {
                        String::new()
                    },
                    tooltip: Some(format!("{label}: {count} sessions")),
                })
                .collect(),
        );
        imp.session_starts_chart.set_bars(
            sessions
                .starts_by_hour
                .iter()
                .enumerate()
                .map(|(hour, &count)| Bar {
                    // Every hour is labelled; BarChart thins the row
                    // against its live width at draw time.
                    label: format!("{hour:02}"),
                    value: f64::from(count),
                    display: String::new(),
                    tooltip: Some(format!("{hour:02}:00 \u{b7} {count} sessions")),
                })
                .collect(),
        );

        imp.weekday_chart.set_bars(
            overview
                .weekday_avg_secs
                .iter()
                .zip(WEEKDAY_LABELS)
                .map(|(&secs, label)| Bar {
                    label: label.into(),
                    value: secs as f64,
                    display: if secs > 0 {
                        humanize_secs(secs)
                    } else {
                        String::new()
                    },
                    tooltip: Some(format!("{label}: {} on average", humanize_secs(secs))),
                })
                .collect(),
        );

        imp.monthly_chart.set_bars(
            overview
                .monthly
                .iter()
                .map(|&(month, secs)| Bar {
                    label: month_label(month),
                    value: secs as f64,
                    display: if secs > 0 {
                        humanize_secs(secs)
                    } else {
                        String::new()
                    },
                    tooltip: Some(format!("{}: {}", month_label(month), humanize_secs(secs))),
                })
                .collect(),
        );

        // Series (spec.md "Series"): whole-library composition, hidden when
        // no book carries series metadata.
        imp.series_rows.remove_all();
        let has_series = !overview.series.is_empty();
        imp.series_title.set_visible(has_series);
        imp.series_rows.set_visible(has_series);
        for s in &overview.series {
            let plural = if s.books == 1 { "" } else { "s" };
            let subtitle = if s.finished > 0 {
                format!("{} book{plural} \u{b7} {} finished", s.books, s.finished)
            } else {
                format!("{} book{plural}", s.books)
            };
            imp.series_rows.append(&rows::value_row(
                &s.name,
                &humanize_secs(s.total_secs),
                Some(&subtitle),
            ));
        }

        // Author affinity (spec.md "Rollups"): most-read authors, hidden
        // when no book carries author metadata. Top 10 keeps the card from
        // running long on a big library.
        imp.author_rows.remove_all();
        let has_authors = !overview.authors.is_empty();
        imp.author_title.set_visible(has_authors);
        imp.author_rows.set_visible(has_authors);
        for a in overview.authors.iter().take(10) {
            let plural = if a.books == 1 { "" } else { "s" };
            let subtitle = if a.finished > 0 {
                format!("{} book{plural} \u{b7} {} finished", a.books, a.finished)
            } else {
                format!("{} book{plural}", a.books)
            };
            imp.author_rows.append(&rows::value_row(
                &a.name,
                &humanize_secs(a.total_secs),
                Some(&subtitle),
            ));
        }

        // Forgotten books (spec.md "Forgotten books"): unfinished and
        // untouched for a while, a gentle nudge. Hidden when none qualify.
        imp.forgotten_rows.remove_all();
        let has_forgotten = !overview.forgotten.is_empty();
        imp.forgotten_title.set_visible(has_forgotten);
        imp.forgotten_rows.set_visible(has_forgotten);
        for f in overview.forgotten.iter().take(10) {
            let subtitle = if f.author.trim().is_empty() {
                format!("last read {}", short_date(f.last_read))
            } else {
                format!("{} \u{b7} last read {}", f.author, short_date(f.last_read))
            };
            imp.forgotten_rows.append(&rows::value_row(
                &f.title,
                &format!("{}d ago", f.days_since),
                Some(&subtitle),
            ));
        }
    }
}

fn month_label(month: NaiveDate) -> String {
    use chrono::Datelike;
    let abbr = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month.month() as usize) - 1];
    // Disambiguate January across year boundaries.
    if month.month() == 1 {
        format!("{abbr} {}", month.year())
    } else {
        abbr.to_owned()
    }
}

fn tile(value: &str, caption: &str, detail: Option<&str>) -> gtk::Widget {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let value_label = gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .css_classes(["title-2"])
        .build();
    let caption_label = gtk::Label::builder()
        .label(caption)
        .xalign(0.0)
        .css_classes(["caption", "dim-label"])
        .build();
    card.append(&value_label);
    card.append(&caption_label);
    if let Some(detail) = detail {
        let detail_label = gtk::Label::builder()
            .label(detail)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["caption", "dim-label"])
            .build();
        card.append(&detail_label);
    }
    let frame = gtk::Box::builder().css_classes(["card"]).build();
    frame.append(&card);
    // Tiles are read-only; keeping them out of the Tab order sends
    // keyboard focus to the charts and lists instead (6e).
    gtk::FlowBoxChild::builder()
        .child(&frame)
        .focusable(false)
        .build()
        .upcast()
}

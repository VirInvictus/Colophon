//! The "All Books" surface: library totals, streaks, and the
//! library-wide charts. Respects the junk filter by construction: the
//! window hands it the already-filtered entry set via `stats::overview`.

use adw::subclass::prelude::*;
use chrono::NaiveDate;
use gtk::glib;
use gtk::prelude::*;

use crate::charts::bar::Bar;
use crate::charts::line::Point;
use crate::fmt::{humanize_secs, short_date};
use crate::stats::{Overview, SESSION_BUCKETS};

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
        pub weekday_chart: TemplateChild<BarChart>,
        #[template_child]
        pub monthly_chart: TemplateChild<BarChart>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OverviewPage {
        const NAME: &'static str = "OverviewPage";
        type Type = super::OverviewPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
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

    impl ObjectImpl for OverviewPage {}
    impl WidgetImpl for OverviewPage {}
    impl BinImpl for OverviewPage {}
}

glib::wrapper! {
    pub struct OverviewPage(ObjectSubclass<imp::OverviewPage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

const WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

impl OverviewPage {
    pub fn set_data(&self, overview: &Overview, today: NaiveDate) {
        let imp = self.imp();

        imp.tiles.remove_all();
        let add_tile = |value: String, caption: &str, detail: Option<String>| {
            imp.tiles.append(&tile(&value, caption, detail.as_deref()));
        };
        add_tile(humanize_secs(overview.total_secs), "total time", None);
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
            "{} sessions \u{b7} median {} \u{b7} longest {}{}",
            sessions.count,
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
                })
                .collect(),
        );
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
    frame.upcast()
}

//! The "All Books" surface: library totals, streaks, and the
//! library-wide charts. Respects the junk filter by construction: the
//! window hands it the already-filtered entry set via `stats::overview`.

use adw::subclass::prelude::*;
use chrono::NaiveDate;
use gtk::glib;
use gtk::prelude::*;

use crate::charts::bar::Bar;
use crate::fmt::{humanize_secs, short_date};
use crate::stats::Overview;

mod imp {
    use super::*;
    use crate::charts::{BarChart, YearHeatmap};
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "overview_page.ui")]
    pub struct OverviewPage {
        #[template_child]
        pub tiles: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub heatmap: TemplateChild<YearHeatmap>,
        #[template_child]
        pub weekday_chart: TemplateChild<BarChart>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OverviewPage {
        const NAME: &'static str = "OverviewPage";
        type Type = super::OverviewPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            YearHeatmap::ensure_type();
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

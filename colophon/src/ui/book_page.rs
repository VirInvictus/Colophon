//! Per-book stat cards (spec.md Tier B #10). Numbers that KOReader also
//! shows use KOReader's math: the headline time is the capped total (the
//! device's "time spent reading"), with the uncapped sum alongside, and
//! the estimates run on capped avg_time.

use adw::prelude::*;
use adw::subclass::prelude::*;
use chrono::NaiveDate;
use gtk::glib;

use crate::fmt::{humanize_secs, short_date};
use crate::library::LibraryEntry;
use crate::stats::BookDetail;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "book_page.ui")]
    pub struct BookPage {
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub authors_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub rows: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BookPage {
        const NAME: &'static str = "BookPage";
        type Type = super::BookPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BookPage {}
    impl WidgetImpl for BookPage {}
    impl BinImpl for BookPage {}
}

glib::wrapper! {
    pub struct BookPage(ObjectSubclass<imp::BookPage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BookPage {
    pub fn set_book(&self, entry: &LibraryEntry, detail: &BookDetail) {
        let imp = self.imp();
        let book = &entry.book;

        imp.title_label.set_text(if book.title.trim().is_empty() {
            "(untitled)"
        } else {
            book.title.trim()
        });
        let authors = book.authors.trim();
        imp.authors_label.set_visible(!authors.is_empty());
        imp.authors_label.set_text(authors);

        let fraction = if book.pages > 0 {
            (entry.unique_pages as f64 / book.pages as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        imp.progress.set_fraction(fraction);
        imp.progress.set_text(Some(&format!(
            "{} / {} pages ({:.0}%)",
            entry.unique_pages,
            book.pages,
            fraction * 100.0
        )));

        imp.rows.remove_all();
        let add = |title: &str, value: String, subtitle: Option<String>| {
            imp.rows
                .append(&stat_row(title, &value, subtitle.as_deref()));
        };

        add(
            "Time spent reading",
            humanize_secs(detail.capped_secs),
            Some(format!(
                "as shown on device \u{b7} uncapped total {}",
                humanize_secs(detail.total_secs)
            )),
        );
        add(
            "Days reading",
            detail.days_reading.to_string(),
            date_range(detail.start_date, detail.last_date),
        );
        add(
            "Average per day",
            humanize_secs(detail.avg_secs_per_day),
            None,
        );
        add(
            "Average per page",
            match detail.avg_secs_per_page {
                Some(avg) => format!("{avg:.0} s"),
                None => "no data".into(),
            },
            None,
        );
        add(
            "Sessions",
            detail.sessions.to_string(),
            (detail.longest_session_secs > 0)
                .then(|| format!("longest {}", humanize_secs(detail.longest_session_secs))),
        );
        add(
            "Estimated time left",
            match detail.est_secs_left {
                Some(left) if detail.est_finish.is_some() => humanize_secs(left),
                _ => "no data".into(),
            },
            detail
                .est_finish
                .map(|d| format!("finish around {}", short_date(d))),
        );
        add(
            "Highlights \u{b7} notes",
            format!("{} \u{b7} {}", book.highlights, book.notes),
            None,
        );
    }
}

fn date_range(start: Option<NaiveDate>, last: Option<NaiveDate>) -> Option<String> {
    match (start, last) {
        (Some(s), Some(l)) if s != l => {
            Some(format!("{} \u{2013} {}", short_date(s), short_date(l)))
        }
        (Some(s), _) => Some(format!("started {}", short_date(s))),
        _ => None,
    }
}

fn stat_row(title: &str, value: &str, subtitle: Option<&str>) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();
    if let Some(subtitle) = subtitle {
        row.set_subtitle(subtitle);
    }
    let value_label = gtk::Label::builder()
        .label(value)
        .css_classes(["dim-label"])
        .build();
    row.add_suffix(&value_label);
    row
}

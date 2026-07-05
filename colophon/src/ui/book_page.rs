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
use crate::stats::{self, BookDetail};

mod imp {
    use super::*;
    use crate::charts::{LineChart, PageActivityStrip, SpanBar};
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "book_page.ui")]
    pub struct BookPage {
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub authors_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress_caption: TemplateChild<gtk::Label>,
        #[template_child]
        pub finished_badge: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress: TemplateChild<SpanBar>,
        #[template_child]
        pub rows: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub activity_strip: TemplateChild<PageActivityStrip>,
        #[template_child]
        pub speed_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub speed_caption: TemplateChild<gtk::Label>,
        #[template_child]
        pub speed_chart: TemplateChild<LineChart>,
        #[template_child]
        pub completions_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub completion_rows: TemplateChild<gtk::ListBox>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BookPage {
        const NAME: &'static str = "BookPage";
        type Type = super::BookPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            PageActivityStrip::ensure_type();
            LineChart::ensure_type();
            SpanBar::ensure_type();
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

        let p = stats::progress(entry);
        imp.progress.set_data(p.spans.clone(), p.furthest);
        imp.finished_badge.set_visible(p.finished);
        imp.progress_caption.set_text(&progress_caption(&p));

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
            detail.est_finish.map(|d| {
                let conf = detail
                    .est_confidence
                    .map(|c| format!(" \u{b7} {c} confidence"))
                    .unwrap_or_default();
                format!("finish around {}{conf}", short_date(d))
            }),
        );
        if let Some(m) = &detail.momentum {
            add("Momentum", m.label.to_string(), Some(m.detail.clone()));
        }
        add(
            "Highlights \u{b7} notes",
            format!("{} \u{b7} {}", book.highlights, book.notes),
            None,
        );
        if detail.revisited_pages > 0 {
            add(
                "Pages revisited",
                detail.revisited_pages.to_string(),
                Some("read more than once".into()),
            );
        }

        imp.activity_strip.set_data(stats::page_activity(entry));

        let completions = stats::book_completions(entry);
        let has_completions = !completions.is_empty();
        imp.completions_title.set_visible(has_completions);
        imp.completion_rows.set_visible(has_completions);
        imp.completion_rows.remove_all();
        for (i, completion) in completions.iter().enumerate() {
            let start = chrono::DateTime::from_timestamp(completion.start_time, 0)
                .map(|d| short_date(d.with_timezone(&chrono::Local).date_naive()));
            let end = chrono::DateTime::from_timestamp(completion.end_time, 0)
                .map(|d| short_date(d.with_timezone(&chrono::Local).date_naive()));
            let row = adw::ActionRow::builder()
                .title(format!("Read-through {}", i + 1))
                .subtitle(match (start, end) {
                    (Some(s), Some(e)) if s != e => format!("{s} \u{2013} {e}"),
                    (Some(s), _) => s,
                    _ => String::new(),
                })
                .build();
            let span_days = ((completion.end_time - completion.start_time) / 86_400).max(0) + 1;
            let value = gtk::Label::builder()
                .label(format!(
                    "{} \u{b7} {} sessions \u{b7} {:.0} pages/hour \u{b7} {:.0} pages/day \u{b7} {:.0}% covered",
                    humanize_secs(completion.seconds),
                    completion.sessions,
                    completion.pages_per_hour,
                    completion.pages_read as f64 / span_days as f64,
                    completion.coverage * 100.0
                ))
                .css_classes(["dim-label"])
                .build();
            row.add_suffix(&value);
            imp.completion_rows.append(&row);
        }
    }

    /// The book's speed trend over the library baseline. Both series
    /// share the bucket so they stay commensurable.
    pub fn set_speed(
        &self,
        book: Vec<crate::charts::line::Point>,
        library: Vec<crate::charts::line::Point>,
        bucket: colophon_core::metrics::Bucket,
    ) {
        let imp = self.imp();
        let has_data = !book.is_empty();
        imp.speed_title.set_visible(has_data);
        imp.speed_caption.set_visible(has_data);
        imp.speed_chart.set_visible(has_data);
        if !has_data {
            return;
        }
        imp.speed_title.set_text(match bucket {
            colophon_core::metrics::Bucket::Day => "Reading speed \u{b7} pages/hour by day",
            _ => "Reading speed \u{b7} pages/hour by week",
        });
        imp.speed_caption
            .set_text("this book, with the whole library shown muted behind it");
        imp.speed_chart.set_series(vec![
            crate::charts::line::Series {
                points: library,
                muted: true,
            },
            crate::charts::line::Series {
                points: book,
                muted: false,
            },
        ]);
    }
}

/// The line under the progress bar. Leads with how far through the book
/// you got (the honest "progress"), then the pages KOReader logged. When
/// coverage trails the furthest position, some of the book was read before
/// KOReader was tracking; the bar shows that gap and this names it.
fn progress_caption(p: &stats::Progress) -> String {
    let logged = format!("{} of {} pages logged", p.unique_pages, p.pages);
    let cov_pct = if p.pages > 0 {
        (p.unique_pages as f64 / p.pages as f64 * 100.0).round()
    } else {
        0.0
    };
    if p.finished {
        // The gap between reaching the end and logged coverage is reading
        // done outside KOReader.
        let gap = ((p.furthest - cov_pct / 100.0) * 100.0).round();
        if gap >= 5.0 {
            format!("{logged} ({cov_pct:.0}%) \u{b7} ~{gap:.0}% read before KOReader")
        } else {
            format!("{logged} ({cov_pct:.0}%)")
        }
    } else {
        format!(
            "{:.0}% through \u{b7} {logged}",
            (p.furthest * 100.0).round()
        )
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

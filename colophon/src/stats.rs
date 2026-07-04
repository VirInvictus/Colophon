//! Display-side aggregate math for the two content surfaces: the
//! "All Books" overview and the per-book detail. Pure and GTK-free;
//! everything computes from in-memory `LibraryEntry` data, so the junk
//! filter applies by simply passing the filtered entry set.
//!
//! Device-parity rules (spec.md): the per-book estimates use KOReader's
//! own math — capped `avg_time` from the rescaled view, time left =
//! pages left x avg_time, finish date = today + time_left / (capped
//! time per reading day).

use std::collections::BTreeMap;
use std::rc::Rc;

use chrono::{Datelike, Duration, NaiveDate, TimeZone};
use colophon_core::metrics::{self, local_date};
use colophon_core::model::{DayTotal, PageEvent, Streaks};

use crate::library::LibraryEntry;

pub struct Overview {
    pub total_secs: i64,
    pub unique_pages: i64,
    pub books: usize,
    pub active_days: usize,
    pub busiest: Option<(NaiveDate, i64)>,
    pub daily: BTreeMap<NaiveDate, DayTotal>,
    pub streaks: Streaks,
    /// Mean seconds per weekday (Mon..Sun), normalized by how many of
    /// that weekday elapsed between the first reading day and `today`
    /// (not by days-with-data; raw sums skew toward old data).
    pub weekday_avg_secs: [i64; 7],
}

pub fn overview<Tz: TimeZone>(entries: &[Rc<LibraryEntry>], tz: &Tz, today: NaiveDate) -> Overview {
    let all_events: Vec<PageEvent> = entries
        .iter()
        .flat_map(|e| e.events.iter().copied())
        .collect();
    let daily = metrics::daily_totals(&all_events, tz);
    let days = daily.keys().copied().collect();
    let streaks = metrics::streaks(&days, today);

    Overview {
        total_secs: entries.iter().map(|e| e.book.total_read_time).sum(),
        unique_pages: entries.iter().map(|e| e.unique_pages).sum(),
        books: entries.len(),
        active_days: daily.len(),
        busiest: daily
            .iter()
            .max_by_key(|(_, t)| t.seconds)
            .map(|(d, t)| (*d, t.seconds)),
        weekday_avg_secs: weekday_averages(&daily, today),
        daily,
        streaks,
    }
}

/// Mean seconds per weekday, denominator = occurrences of that weekday in
/// [first reading day, today] inclusive. Empty history yields zeros.
pub fn weekday_averages(daily: &BTreeMap<NaiveDate, DayTotal>, today: NaiveDate) -> [i64; 7] {
    let Some((&first, _)) = daily.iter().next() else {
        return [0; 7];
    };
    let mut totals = [0i64; 7];
    for (date, day) in daily {
        totals[date.weekday().num_days_from_monday() as usize] += day.seconds;
    }
    let mut out = [0i64; 7];
    for (weekday, total) in totals.into_iter().enumerate() {
        let count = weekday_occurrences(first, today, weekday);
        out[weekday] = if count > 0 { total / count } else { 0 };
    }
    out
}

/// How many times weekday `w` (0 = Monday) occurs in [from, to] inclusive.
fn weekday_occurrences(from: NaiveDate, to: NaiveDate, weekday: usize) -> i64 {
    if to < from {
        return 0;
    }
    let span_days = (to - from).num_days() + 1;
    let first_w = from.weekday().num_days_from_monday() as i64;
    let offset = (weekday as i64 - first_w).rem_euclid(7);
    if offset >= span_days {
        0
    } else {
        (span_days - offset - 1) / 7 + 1
    }
}

pub struct BookDetail {
    /// Uncapped total, the raw sum ("total time spent on this book").
    pub total_secs: i64,
    /// Capped total, what the device shows as "time spent reading".
    pub capped_secs: i64,
    pub days_reading: usize,
    pub avg_secs_per_day: i64,
    pub avg_secs_per_page: Option<f64>,
    pub start_date: Option<NaiveDate>,
    pub last_date: Option<NaiveDate>,
    pub sessions: usize,
    pub longest_session_secs: i64,
    /// KOReader's estimate: (pages - last_page) * capped avg_time.
    pub est_secs_left: Option<i64>,
    /// KOReader's estimate: today + time_left / (capped secs per day).
    pub est_finish: Option<NaiveDate>,
}

pub fn book_detail<Tz: TimeZone>(entry: &LibraryEntry, tz: &Tz, today: NaiveDate) -> BookDetail {
    let book = &entry.book;
    let events = &entry.events;

    let dates: std::collections::BTreeSet<NaiveDate> = events
        .iter()
        .filter(|e| e.duration > 0)
        .map(|e| local_date(e.start_time, tz))
        .collect();
    let days_reading = dates.len();

    let sessions = metrics::sessions(events, colophon_core::model::DEFAULT_SESSION_GAP_SECS);
    let avg_secs_per_page = metrics::avg_seconds_per_page(entry.capped_secs, entry.view_pages);

    // KOReader's time-left/finish-date math (main.lua:1641-1643), capped
    // numbers throughout, so Colophon never contradicts the device.
    let pages_left = (book.pages - entry.last_page).max(0);
    let est_secs_left = avg_secs_per_page.map(|avg| (pages_left as f64 * avg) as i64);
    let est_finish = est_secs_left.and_then(|left| {
        if days_reading == 0 || entry.capped_secs == 0 {
            return None;
        }
        let per_day = entry.capped_secs as f64 / days_reading as f64;
        let days = (left as f64 / per_day).ceil() as i64;
        today.checked_add_signed(Duration::days(days))
    });

    BookDetail {
        total_secs: book.total_read_time,
        capped_secs: entry.capped_secs,
        days_reading,
        avg_secs_per_day: if days_reading > 0 {
            entry.capped_secs / days_reading as i64
        } else {
            0
        },
        avg_secs_per_page,
        start_date: dates.iter().next().copied(),
        last_date: dates.iter().next_back().copied(),
        sessions: sessions.len(),
        longest_session_secs: sessions.iter().map(|s| s.seconds).max().unwrap_or(0),
        est_secs_left,
        est_finish,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use colophon_core::Book;

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn ts(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap().timestamp()
    }

    fn ev(page: i64, start_time: i64, duration: i64) -> PageEvent {
        PageEvent {
            book_id: 1,
            page,
            start_time,
            duration,
            total_pages: 100,
        }
    }

    fn entry(events: Vec<PageEvent>) -> Rc<LibraryEntry> {
        let total: i64 = events.iter().map(|e| e.duration).sum();
        Rc::new(LibraryEntry {
            book: Book {
                id: 1,
                all_ids: vec![1],
                title: "T".into(),
                authors: "A".into(),
                notes: 0,
                highlights: 0,
                pages: 100,
                series: None,
                language: None,
                md5: None,
                total_read_time: total,
                total_read_pages: 0,
                last_open: 0,
            },
            unique_pages: 0,
            events,
            capped_secs: total,
            view_pages: 50,
            last_page: 50,
        })
    }

    #[test]
    fn weekday_occurrences_counts_inclusively() {
        // 2026-06-29 is a Monday; two full weeks = 2 of each weekday.
        assert_eq!(
            weekday_occurrences(date("2026-06-29"), date("2026-07-12"), 0),
            2
        );
        // Mon..Wed span contains one Monday, one Wednesday, zero Fridays.
        assert_eq!(
            weekday_occurrences(date("2026-06-29"), date("2026-07-01"), 0),
            1
        );
        assert_eq!(
            weekday_occurrences(date("2026-06-29"), date("2026-07-01"), 2),
            1
        );
        assert_eq!(
            weekday_occurrences(date("2026-06-29"), date("2026-07-01"), 4),
            0
        );
    }

    #[test]
    fn weekday_averages_normalize_by_elapsed_weekdays() {
        // Reading 600 s on each of two consecutive Mondays, today is the
        // second Monday: average Monday = 600, not 1200 (raw-sum skew).
        let mut daily = BTreeMap::new();
        daily.insert(
            date("2026-06-29"),
            DayTotal {
                seconds: 600,
                ..Default::default()
            },
        );
        daily.insert(
            date("2026-07-06"),
            DayTotal {
                seconds: 600,
                ..Default::default()
            },
        );
        let avg = weekday_averages(&daily, date("2026-07-06"));
        assert_eq!(avg[0], 600);
        assert_eq!(avg[1], 0);
    }

    #[test]
    fn overview_sums_and_streaks() {
        let a = entry(vec![
            ev(1, ts(2026, 7, 2, 10), 60),
            ev(2, ts(2026, 7, 3, 10), 120),
        ]);
        let b = entry(vec![ev(1, ts(2026, 7, 3, 12), 30)]);
        let ov = overview(&[a, b], &Utc, date("2026-07-03"));
        assert_eq!(ov.books, 2);
        assert_eq!(ov.total_secs, 210);
        assert_eq!(ov.active_days, 2);
        assert_eq!(ov.streaks.current.unwrap().days, 2);
        assert_eq!(ov.busiest.unwrap().0, date("2026-07-03"));
        assert_eq!(ov.busiest.unwrap().1, 150);
    }

    #[test]
    fn book_detail_uses_koreader_estimate_math() {
        // 50 view-pages read in 5000 s capped over one day; avg_time =
        // 100 s/page; 50 pages left => 5000 s left; per-day = 5000 =>
        // finish tomorrow.
        let events: Vec<_> = (1..=50)
            .map(|p| ev(p, ts(2026, 7, 3, 8) + p * 100, 100))
            .collect();
        let e = entry(events);
        let d = book_detail(&e, &Utc, date("2026-07-03"));
        assert_eq!(d.days_reading, 1);
        assert_eq!(d.avg_secs_per_page, Some(100.0));
        assert_eq!(d.est_secs_left, Some(5000));
        assert_eq!(d.est_finish, Some(date("2026-07-04")));
        assert_eq!(d.sessions, 1);
        assert_eq!(d.longest_session_secs, 5000);
        assert_eq!(d.start_date, Some(date("2026-07-03")));
    }

    #[test]
    fn book_detail_handles_no_data() {
        let e = Rc::new(LibraryEntry {
            capped_secs: 0,
            view_pages: 0,
            last_page: 0,
            events: Vec::new(),
            unique_pages: 0,
            book: entry(Vec::new()).book.clone(),
        });
        let d = book_detail(&e, &Utc, date("2026-07-03"));
        assert_eq!(d.days_reading, 0);
        assert_eq!(d.avg_secs_per_page, None);
        assert_eq!(d.est_finish, None);
        assert_eq!(d.sessions, 0);
    }
}

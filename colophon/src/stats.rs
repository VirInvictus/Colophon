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
use colophon_core::metrics::{self, Bucket, local_date};
use colophon_core::model::{DayTotal, PageEvent, SpeedPoint, Streaks};

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
    /// Weekday x hour-of-day seconds (Mon..Sun rows), whole history.
    pub hourly: [[i64; 24]; 7],
    /// Seconds per calendar month from the first reading month through
    /// `today`'s month, empty months included (rendered, not skipped).
    pub monthly: Vec<(NaiveDate, i64)>,
    /// Reading speed over time (pages/hour per bucket, keyed by bucket
    /// start): daily buckets under ~10 weeks of history, weekly after.
    pub speed: Vec<(NaiveDate, SpeedPoint)>,
    pub speed_bucket: Bucket,
    pub sessions: SessionSummary,
}

#[derive(Debug, Default, PartialEq)]
pub struct SessionSummary {
    pub count: usize,
    pub median_secs: i64,
    pub longest_secs: i64,
    pub longest_date: Option<NaiveDate>,
    /// Session-length histogram; bucket bounds in `SESSION_BUCKETS`.
    pub histogram: [u32; 6],
    /// Sessions by local start hour (00..23).
    pub starts_by_hour: [u32; 24],
    /// Mean sessions per day *with reading* (not per calendar day).
    pub per_active_day: f64,
}

/// Histogram bucket labels and upper bounds in seconds; the last bucket
/// is open-ended.
pub const SESSION_BUCKETS: [(&str, i64); 6] = [
    ("<5m", 300),
    ("5\u{2013}15m", 900),
    ("15\u{2013}30m", 1800),
    ("30\u{2013}60m", 3600),
    ("1\u{2013}2h", 7200),
    (">2h", i64::MAX),
];

/// The window-independent part of the overview: the whole-history
/// aggregates that a time-window selection never touches (streaks, the
/// year heatmap's daily map, monthly bars) plus the flattened event list
/// they were built from. Computing these is the expensive half of the
/// overview (`daily_totals` alone is the single biggest render cost), so
/// the window caches an `OverviewBase` and only recomputes it when the
/// filtered entry set changes (junk toggle, re-import), not on every
/// window toggle.
pub struct OverviewBase {
    all_events: Vec<PageEvent>,
    daily: BTreeMap<NaiveDate, DayTotal>,
    streaks: Streaks,
    monthly: Vec<(NaiveDate, i64)>,
}

/// Builds the window-independent aggregates once for a filtered entry set.
pub fn overview_base<Tz: TimeZone>(
    entries: &[Rc<LibraryEntry>],
    tz: &Tz,
    today: NaiveDate,
) -> OverviewBase {
    let all_events: Vec<PageEvent> = entries
        .iter()
        .flat_map(|e| e.events.iter().copied())
        .collect();
    let daily = metrics::daily_totals(&all_events, tz);
    let days = daily.keys().copied().collect();
    let streaks = metrics::streaks(&days, today);
    let monthly = monthly_totals(&daily, today);
    OverviewBase {
        all_events,
        daily,
        streaks,
        monthly,
    }
}

/// Computes the overview from a cached [`OverviewBase`]. `window_days =
/// None` means all-time; `Some(n)` scopes the totals tiles and the
/// behaviour charts (hourly, speed, sessions, weekday) to the last `n`
/// *calendar* days ending today (not "last n days with data",
/// Kodashboard's KPI bug). The whole-history sections (streaks, year
/// heatmap, monthly) come straight from the base: windowing a streak or a
/// year grid would just lie.
pub fn overview_windowed<Tz: TimeZone>(
    base: &OverviewBase,
    entries: &[Rc<LibraryEntry>],
    tz: &Tz,
    today: NaiveDate,
    window_days: Option<i64>,
) -> Overview {
    let cutoff = window_days.map(|n| today - Duration::days(n - 1));
    let in_window =
        |e: &PageEvent| cutoff.is_none_or(|c| metrics::local_date(e.start_time, tz) >= c);
    let windowed: Vec<PageEvent> = base
        .all_events
        .iter()
        .copied()
        .filter(|e| in_window(e))
        .collect();
    let windowed_daily = match cutoff {
        Some(c) => base.daily.range(c..).map(|(d, t)| (*d, *t)).collect(),
        None => base.daily.clone(),
    };

    // Windowed totals: event sums, not the cached book counters (those
    // are all-time only; for all-time the two reconcile exactly).
    let mut unique_pages = 0;
    let mut books = 0usize;
    for entry in entries {
        let events: Vec<PageEvent> = entry
            .events
            .iter()
            .copied()
            .filter(|e| in_window(e))
            .collect();
        if events.is_empty() {
            continue;
        }
        books += 1;
        unique_pages += metrics::unique_pages_read(metrics::coverage(&events), entry.book.pages);
    }

    let speed_bucket = speed_bucket_for(windowed_daily.keys().next().copied(), today);
    let speed = metrics::speed_series(&windowed, tz, speed_bucket)
        .into_iter()
        .collect();

    Overview {
        total_secs: windowed.iter().map(|e| e.duration).sum(),
        unique_pages,
        books,
        active_days: windowed_daily.len(),
        busiest: windowed_daily
            .iter()
            .max_by_key(|(_, t)| t.seconds)
            .map(|(d, t)| (*d, t.seconds)),
        weekday_avg_secs: weekday_averages(&windowed_daily, today),
        hourly: metrics::hourly_profile(&windowed, tz),
        monthly: base.monthly.clone(),
        speed,
        speed_bucket,
        sessions: session_summary(&windowed, tz),
        daily: base.daily.clone(),
        streaks: base.streaks,
    }
}

/// The speed-trend bucket rule, shared with the per-book trend so the
/// two series stay commensurable.
pub fn speed_bucket_for(first: Option<NaiveDate>, today: NaiveDate) -> Bucket {
    match first {
        Some(first) if (today - first).num_days() > 70 => Bucket::Week,
        _ => Bucket::Day,
    }
}

/// Seconds per month, first reading month through today's month, empty
/// months rendered as zeros.
pub fn monthly_totals(
    daily: &BTreeMap<NaiveDate, DayTotal>,
    today: NaiveDate,
) -> Vec<(NaiveDate, i64)> {
    let Some((&first, _)) = daily.iter().next() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut month = first.with_day(1).expect("day 1 exists");
    let last = today.with_day(1).expect("day 1 exists");
    while month <= last {
        out.push((month, 0));
        month = next_month(month);
    }
    for (date, day) in daily {
        let key = date.with_day(1).expect("day 1 exists");
        if let Some(slot) = out.iter_mut().find(|(m, _)| *m == key) {
            slot.1 += day.seconds;
        }
    }
    out
}

fn next_month(month: NaiveDate) -> NaiveDate {
    let (year, m) = (month.year(), month.month());
    if m == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).expect("valid date")
    } else {
        NaiveDate::from_ymd_opt(year, m + 1, 1).expect("valid date")
    }
}

pub fn session_summary<Tz: TimeZone>(events: &[PageEvent], tz: &Tz) -> SessionSummary {
    let sessions = metrics::sessions(events, colophon_core::model::DEFAULT_SESSION_GAP_SECS);
    if sessions.is_empty() {
        return SessionSummary::default();
    }
    let mut lengths: Vec<i64> = sessions.iter().map(|s| s.seconds).collect();
    lengths.sort_unstable();
    let longest = sessions
        .iter()
        .max_by_key(|s| s.seconds)
        .expect("non-empty");

    let mut histogram = [0u32; 6];
    for &len in &lengths {
        let slot = SESSION_BUCKETS
            .iter()
            .position(|(_, bound)| len < *bound)
            .unwrap_or(SESSION_BUCKETS.len() - 1);
        histogram[slot] += 1;
    }

    let mut starts_by_hour = [0u32; 24];
    let mut active_days = std::collections::BTreeSet::new();
    for session in &sessions {
        let local = tz
            .timestamp_opt(session.start_time, 0)
            .single()
            .expect("epoch timestamp maps to exactly one instant");
        starts_by_hour[chrono::Timelike::hour(&local) as usize] += 1;
        active_days.insert(local.date_naive());
    }
    let per_active_day = sessions.len() as f64 / active_days.len().max(1) as f64;

    SessionSummary {
        count: sessions.len(),
        median_secs: lengths[lengths.len() / 2],
        longest_secs: longest.seconds,
        longest_date: Some(metrics::local_date(longest.start_time, tz)),
        histogram,
        starts_by_hour,
        per_active_day,
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

/// Per-page reading intensity for the activity strip (spec.md Tier A
/// #5), on the stable current page axis. Display uses sqrt scaling
/// capped at the 90th percentile (KoShelf's numbers), so one page you
/// fell asleep on doesn't flatten the rest of the book.
pub struct PageActivity {
    /// The book's current page count (the x axis).
    pub pages: i64,
    /// (page, total seconds, read count), sorted by page; only pages
    /// with any activity appear.
    pub per_page: Vec<(i64, i64, u32)>,
    /// 90th-percentile of the nonzero per-page seconds; the display cap.
    pub cap_secs: i64,
}

pub fn page_activity(entry: &LibraryEntry) -> PageActivity {
    // Already reduced to one row per page by the `page_totals` query, in
    // page order; keep only pages with real (positive-duration) reads.
    let per_page: Vec<(i64, i64, u32)> = entry
        .page_totals
        .iter()
        .filter(|pt| pt.reads > 0)
        .map(|pt| (pt.page, pt.secs, pt.reads))
        .collect();

    let mut durations: Vec<i64> = per_page.iter().map(|&(_, secs, _)| secs).collect();
    durations.sort_unstable();
    let cap_secs = if durations.is_empty() {
        0
    } else {
        durations[(durations.len() * 9 / 10).min(durations.len() - 1)]
    };

    PageActivity {
        pages: entry.book.pages,
        per_page,
        cap_secs,
    }
}

/// Inferred read-throughs for one book (spec.md "Completion").
pub fn book_completions(entry: &LibraryEntry) -> Vec<colophon_core::Completion> {
    metrics::completions(
        &entry.events,
        entry.book.pages,
        &metrics::CompletionConfig::default(),
    )
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

    /// One-shot overview (base + windowed), exercising the real two-step
    /// path the window drives via its cache.
    fn overview<Tz: TimeZone>(
        entries: &[Rc<LibraryEntry>],
        tz: &Tz,
        today: NaiveDate,
        window_days: Option<i64>,
    ) -> Overview {
        let base = overview_base(entries, tz, today);
        overview_windowed(&base, entries, tz, today, window_days)
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
            page_totals: Vec::new(),
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
    fn monthly_totals_render_empty_months() {
        let mut daily = BTreeMap::new();
        daily.insert(
            date("2026-03-15"),
            DayTotal {
                seconds: 100,
                ..Default::default()
            },
        );
        daily.insert(
            date("2026-06-01"),
            DayTotal {
                seconds: 200,
                ..Default::default()
            },
        );
        let months = monthly_totals(&daily, date("2026-07-03"));
        let expected = [
            (date("2026-03-01"), 100),
            (date("2026-04-01"), 0),
            (date("2026-05-01"), 0),
            (date("2026-06-01"), 200),
            (date("2026-07-01"), 0),
        ];
        assert_eq!(months, expected);
    }

    #[test]
    fn session_summary_buckets_and_records() {
        // Three sessions: 2 min, 20 min, 90 min, on different days.
        let mut events = Vec::new();
        for (day, pages) in [(1, 2), (2, 20), (3, 90)] {
            for i in 0..pages {
                events.push(ev(i + 1, ts(2026, 7, day, 10) + i * 60, 60));
            }
        }
        let summary = session_summary(&events, &Utc);
        assert_eq!(summary.count, 3);
        assert_eq!(summary.median_secs, 20 * 60);
        assert_eq!(summary.longest_secs, 90 * 60);
        assert_eq!(summary.longest_date, Some(date("2026-07-03")));
        assert_eq!(summary.histogram, [1, 0, 1, 0, 1, 0]);
    }

    #[test]
    fn overview_picks_daily_then_weekly_speed_buckets() {
        // 9 days of history: daily buckets.
        let young = entry(vec![ev(1, ts(2026, 6, 24, 10), 60)]);
        let ov = overview(&[young], &Utc, date("2026-07-03"), None);
        assert_eq!(ov.speed_bucket, Bucket::Day);

        // Half a year: weekly buckets.
        let old = entry(vec![ev(1, ts(2026, 1, 1, 10), 60)]);
        let ov = overview(&[old], &Utc, date("2026-07-03"), None);
        assert_eq!(ov.speed_bucket, Bucket::Week);
        assert!(!ov.speed.is_empty());
    }

    #[test]
    fn overview_window_scopes_totals_but_not_history() {
        // One old book (June 1) and one recent (July 2-3).
        let old = entry(vec![ev(1, ts(2026, 6, 1, 10), 600)]);
        let recent = entry(vec![
            ev(1, ts(2026, 7, 2, 10), 60),
            ev(2, ts(2026, 7, 3, 10), 120),
        ]);
        let ov = overview(&[old, recent], &Utc, date("2026-07-03"), Some(30));

        // Windowed tiles: only the recent book's events count.
        assert_eq!(ov.total_secs, 180);
        assert_eq!(ov.books, 1);
        assert_eq!(ov.active_days, 2);

        // Whole-history sections still see June: monthly covers Jun+Jul,
        // and the daily map keeps the old day for the year heatmap.
        assert_eq!(ov.monthly.len(), 2);
        assert!(ov.daily.contains_key(&date("2026-06-01")));

        // A 1-day window cuts yesterday's event too.
        let recent2 = entry(vec![
            ev(1, ts(2026, 7, 2, 10), 60),
            ev(2, ts(2026, 7, 3, 10), 120),
        ]);
        let ov = overview(&[recent2], &Utc, date("2026-07-03"), Some(1));
        assert_eq!(ov.total_secs, 120);
    }

    #[test]
    fn session_summary_tracks_starts_and_density() {
        // Two sessions on one day (10:00, 21:00), one the next (10:00).
        let mut events = vec![
            ev(1, ts(2026, 7, 1, 10), 60),
            ev(2, ts(2026, 7, 1, 21), 60),
            ev(3, ts(2026, 7, 2, 10), 60),
        ];
        events.sort_by_key(|e| e.start_time);
        let summary = session_summary(&events, &Utc);
        assert_eq!(summary.count, 3);
        assert_eq!(summary.starts_by_hour[10], 2);
        assert_eq!(summary.starts_by_hour[21], 1);
        assert!((summary.per_active_day - 1.5).abs() < 1e-9);
    }

    #[test]
    fn overview_sums_and_streaks() {
        let a = entry(vec![
            ev(1, ts(2026, 7, 2, 10), 60),
            ev(2, ts(2026, 7, 3, 10), 120),
        ]);
        let b = entry(vec![ev(1, ts(2026, 7, 3, 12), 30)]);
        let ov = overview(&[a, b], &Utc, date("2026-07-03"), None);
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
    fn page_activity_aggregates_on_the_stable_axis() {
        use colophon_core::PageTotal;
        let mut e = entry(Vec::new());
        // page_totals arrive pre-grouped, one row per page, ordered by page.
        // Page 5 read twice (100 s), page 6 once (30 s); ten light pages to
        // give the percentile something to cap against.
        let mut page_totals = vec![
            PageTotal {
                page: 5,
                secs: 100,
                reads: 2,
            },
            PageTotal {
                page: 6,
                secs: 30,
                reads: 1,
            },
        ];
        for p in 10..20 {
            page_totals.push(PageTotal {
                page: p,
                secs: 10,
                reads: 1,
            });
        }
        Rc::get_mut(&mut e).unwrap().page_totals = page_totals;

        let activity = page_activity(&e);
        assert_eq!(activity.pages, 100);
        assert_eq!(activity.per_page[0], (5, 100, 2));
        assert_eq!(activity.per_page[1], (6, 30, 1));
        assert_eq!(activity.per_page.len(), 12);
        // p90 of [10x10, 30, 100]: index (12-1)*9/10 = 9 -> 30.
        assert_eq!(activity.cap_secs, 30);
    }

    #[test]
    fn book_completions_plumb_through() {
        // A full read at one page a minute is one completion.
        let events: Vec<_> = (1..=100)
            .map(|p| ev(p, ts(2026, 6, 1, 8) + p * 60, 60))
            .collect();
        let e = entry(events);
        let completions = book_completions(&e);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].pages_read, 100);
    }

    #[test]
    fn book_detail_handles_no_data() {
        let e = Rc::new(LibraryEntry {
            capped_secs: 0,
            view_pages: 0,
            last_page: 0,
            events: Vec::new(),
            page_totals: Vec::new(),
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

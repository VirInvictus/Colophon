//! Day bucketing, daily totals, and streaks.
//!
//! A "day" is a local-timezone calendar day. Functions are generic over
//! `chrono::TimeZone` so the app passes `chrono::Local` and tests pass a
//! fixed offset. Timestamps in the database are plain unix epoch seconds
//! with no stored timezone (RESEARCH.md §1).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate, TimeZone};

use crate::model::{DayTotal, PageEvent, Streak, Streaks};

/// The local calendar date a timestamp falls on.
pub fn local_date<Tz: TimeZone>(ts: i64, tz: &Tz) -> NaiveDate {
    // Epoch-second lookups map to exactly one instant; `single()` only
    // fails for ambiguous *wall-clock* inputs, which this is not.
    tz.timestamp_opt(ts, 0)
        .single()
        .expect("epoch timestamp maps to exactly one instant")
        .date_naive()
}

/// Per-day aggregates over any set of events, keyed by local date.
pub fn daily_totals<Tz: TimeZone>(events: &[PageEvent], tz: &Tz) -> BTreeMap<NaiveDate, DayTotal> {
    // Accumulator per day: running totals plus the distinct (book, page)
    // and book sets that become the `pages`/`books` counts.
    type Acc = (DayTotal, BTreeSet<(i64, i64)>, BTreeSet<i64>);
    let mut days: BTreeMap<NaiveDate, Acc> = BTreeMap::new();

    for event in events {
        if event.duration <= 0 {
            continue;
        }
        let date = local_date(event.start_time, tz);
        let (total, pages, books) = days.entry(date).or_default();
        total.seconds += event.duration;
        total.events += 1;
        pages.insert((event.book_id, event.page));
        books.insert(event.book_id);
    }

    days.into_iter()
        .map(|(date, (mut total, pages, books))| {
            total.pages = pages.len() as u32;
            total.books = books.len() as u32;
            (date, total)
        })
        .collect()
}

/// Weekday x hour-of-day reading profile: seconds per (weekday, hour)
/// cell, weekday-major with Monday = 0. Each event's whole duration is
/// attributed to its start hour, matching KOReader's own per-day
/// histograms (spec.md Tier A "When-do-I-read heatmap").
pub fn hourly_profile<Tz: TimeZone>(events: &[PageEvent], tz: &Tz) -> [[i64; 24]; 7] {
    let mut grid = [[0i64; 24]; 7];
    for event in events {
        if event.duration <= 0 {
            continue;
        }
        let local = tz
            .timestamp_opt(event.start_time, 0)
            .single()
            .expect("epoch timestamp maps to exactly one instant");
        let weekday = local.date_naive().weekday().num_days_from_monday() as usize;
        let hour = chrono::Timelike::hour(&local) as usize;
        grid[weekday][hour] += event.duration;
    }
    grid
}

/// Streaks over a set of reading days, per the converged convention
/// (readingstreak, Kodashboard, and KoShelf all agree; RESEARCH.md §6):
/// the current streak is alive iff the last reading day is `today` or
/// yesterday, and a gap of two or more days zeroes it.
pub fn streaks(days: &BTreeSet<NaiveDate>, today: NaiveDate) -> Streaks {
    let mut longest: Option<Streak> = None;
    let mut run_start: Option<NaiveDate> = None;
    let mut prev: Option<NaiveDate> = None;

    for &day in days {
        match prev {
            Some(p) if day == p + Duration::days(1) => {}
            _ => run_start = Some(day),
        }
        let start = run_start.expect("run_start set on run entry");
        let run = Streak {
            days: (day - start).num_days() as u32 + 1,
            start,
            end: day,
        };
        if longest.is_none_or(|l| run.days > l.days) {
            longest = Some(run);
        }
        prev = Some(day);
    }

    let current = match (prev, run_start) {
        (Some(last), Some(start)) if (today - last).num_days() <= 1 => Some(Streak {
            days: (last - start).num_days() as u32 + 1,
            start,
            end: last,
        }),
        _ => None,
    };

    Streaks { current, longest }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn day_set(dates: &[&str]) -> BTreeSet<NaiveDate> {
        dates.iter().map(|s| d(s)).collect()
    }

    #[test]
    fn hourly_profile_buckets_by_start_hour_and_weekday() {
        // 2026-07-02 is a Thursday (weekday 3); 21:30 local.
        let thu_evening = Utc
            .with_ymd_and_hms(2026, 7, 2, 21, 30, 0)
            .unwrap()
            .timestamp();
        let events = vec![
            PageEvent {
                book_id: 1,
                page: 1,
                start_time: thu_evening,
                duration: 60,
                total_pages: 100,
            },
            // Spills past the hour but is attributed to its start hour.
            PageEvent {
                book_id: 1,
                page: 2,
                start_time: thu_evening + 29 * 60,
                duration: 120,
                total_pages: 100,
            },
        ];
        let grid = hourly_profile(&events, &Utc);
        assert_eq!(grid[3][21], 180);
        assert_eq!(grid[3][22], 0);
        assert_eq!(grid.iter().flatten().sum::<i64>(), 180);
    }

    #[test]
    fn hourly_profile_respects_timezone() {
        // 01:00 UTC Friday is 21:00 Thursday in UTC-4.
        let ts = Utc
            .with_ymd_and_hms(2026, 7, 3, 1, 0, 0)
            .unwrap()
            .timestamp();
        let events = vec![PageEvent {
            book_id: 1,
            page: 1,
            start_time: ts,
            duration: 60,
            total_pages: 100,
        }];
        let toronto = chrono::FixedOffset::west_opt(4 * 3600).unwrap();
        let grid = hourly_profile(&events, &toronto);
        assert_eq!(grid[3][21], 60); // Thursday 21:00 local
        assert_eq!(hourly_profile(&events, &Utc)[4][1], 60); // Friday 01:00 UTC
    }

    #[test]
    fn local_date_uses_the_given_timezone() {
        // 2026-07-03 01:00 UTC is still 2026-07-02 in UTC-4 (Toronto DST).
        let ts = Utc
            .with_ymd_and_hms(2026, 7, 3, 1, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(local_date(ts, &Utc), d("2026-07-03"));
        let toronto = chrono::FixedOffset::west_opt(4 * 3600).unwrap();
        assert_eq!(local_date(ts, &toronto), d("2026-07-02"));
    }

    #[test]
    fn daily_totals_dedupes_pages_and_books() {
        let base = Utc
            .with_ymd_and_hms(2026, 7, 1, 12, 0, 0)
            .unwrap()
            .timestamp();
        let events = vec![
            PageEvent {
                book_id: 1,
                page: 10,
                start_time: base,
                duration: 60,
                total_pages: 100,
            },
            PageEvent {
                book_id: 1,
                page: 10,
                start_time: base + 60,
                duration: 30,
                total_pages: 100,
            },
            PageEvent {
                book_id: 2,
                page: 1,
                start_time: base + 120,
                duration: 10,
                total_pages: 50,
            },
            // Next day.
            PageEvent {
                book_id: 1,
                page: 11,
                start_time: base + 86_400,
                duration: 40,
                total_pages: 100,
            },
        ];
        let totals = daily_totals(&events, &Utc);
        assert_eq!(totals.len(), 2);
        let first = totals[&d("2026-07-01")];
        assert_eq!(first.seconds, 100);
        assert_eq!(first.events, 3);
        assert_eq!(first.pages, 2); // page 10 re-read counts once
        assert_eq!(first.books, 2);
        assert_eq!(totals[&d("2026-07-02")].seconds, 40);
    }

    #[test]
    fn streak_alive_when_read_today() {
        let days = day_set(&["2026-06-30", "2026-07-01", "2026-07-02", "2026-07-03"]);
        let got = streaks(&days, d("2026-07-03"));
        assert_eq!(got.current.unwrap().days, 4);
        assert_eq!(got.longest.unwrap().days, 4);
    }

    #[test]
    fn streak_survives_not_having_read_yet_today() {
        let days = day_set(&["2026-07-01", "2026-07-02"]);
        let got = streaks(&days, d("2026-07-03"));
        assert_eq!(got.current.unwrap().days, 2);
        assert_eq!(got.current.unwrap().end, d("2026-07-02"));
    }

    #[test]
    fn streak_dies_after_a_two_day_gap() {
        let days = day_set(&["2026-07-01", "2026-07-02"]);
        let got = streaks(&days, d("2026-07-04"));
        assert!(got.current.is_none());
        assert_eq!(got.longest.unwrap().days, 2);
    }

    #[test]
    fn longest_streak_remembers_its_range() {
        let days = day_set(&[
            "2026-06-01",
            "2026-06-02",
            "2026-06-03", // 3-day run
            "2026-06-10", // singleton
            "2026-07-02",
            "2026-07-03",
        ]);
        let got = streaks(&days, d("2026-07-03"));
        let longest = got.longest.unwrap();
        assert_eq!(longest.days, 3);
        assert_eq!(longest.start, d("2026-06-01"));
        assert_eq!(longest.end, d("2026-06-03"));
        assert_eq!(got.current.unwrap().days, 2);
    }

    #[test]
    fn empty_history_has_no_streaks() {
        let got = streaks(&BTreeSet::new(), d("2026-07-03"));
        assert_eq!(got, Streaks::default());
    }
}

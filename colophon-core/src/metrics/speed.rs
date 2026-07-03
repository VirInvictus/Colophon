//! Reading-speed series: distinct pages per uncapped hour, bucketed by
//! day, week, or month. No existing tool computes this (RESEARCH.md §8);
//! definitions follow `spec.md`.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate, TimeZone};

use crate::metrics::days::local_date;
use crate::model::{PageEvent, SpeedPoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Day,
    /// ISO-style Monday-start weeks, keyed by the Monday.
    Week,
    /// Keyed by the first of the month.
    Month,
}

impl Bucket {
    fn start_of(self, date: NaiveDate) -> NaiveDate {
        match self {
            Bucket::Day => date,
            Bucket::Week => date - Duration::days(date.weekday().num_days_from_monday() as i64),
            Bucket::Month => date.with_day(1).expect("day 1 exists in every month"),
        }
    }
}

/// Speed per bucket over any set of events, keyed by the bucket's first
/// day. Buckets with reading time but zero pages (possible in theory with
/// corrupt rows) report 0.0 pages/hour.
pub fn speed_series<Tz: TimeZone>(
    events: &[PageEvent],
    tz: &Tz,
    bucket: Bucket,
) -> BTreeMap<NaiveDate, SpeedPoint> {
    let mut acc: BTreeMap<NaiveDate, (i64, BTreeSet<(i64, i64)>)> = BTreeMap::new();

    for event in events {
        if event.duration <= 0 {
            continue;
        }
        let key = bucket.start_of(local_date(event.start_time, tz));
        let (seconds, pages) = acc.entry(key).or_default();
        *seconds += event.duration;
        pages.insert((event.book_id, event.page));
    }

    acc.into_iter()
        .map(|(key, (seconds, pages))| {
            let pages = pages.len() as u32;
            let pages_per_hour = if seconds > 0 {
                pages as f64 / (seconds as f64 / 3600.0)
            } else {
                0.0
            };
            (
                key,
                SpeedPoint {
                    pages,
                    seconds,
                    pages_per_hour,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn ev(page: i64, start_time: i64, duration: i64) -> PageEvent {
        PageEvent {
            book_id: 1,
            page,
            start_time,
            duration,
            total_pages: 100,
        }
    }

    fn ts(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap().timestamp()
    }

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn daily_speed_is_pages_per_uncapped_hour() {
        // 30 distinct pages in 30 minutes = 60 pages/hour.
        let events: Vec<_> = (1..=30)
            .map(|p| ev(p, ts(2026, 7, 1, 12) + p * 60, 60))
            .collect();
        let series = speed_series(&events, &Utc, Bucket::Day);
        let point = series[&date("2026-07-01")];
        assert_eq!(point.pages, 30);
        assert_eq!(point.seconds, 1800);
        assert!((point.pages_per_hour - 60.0).abs() < 1e-9);
    }

    #[test]
    fn weeks_key_on_monday() {
        // 2026-07-01 is a Wednesday; its week starts Monday 2026-06-29.
        let series = speed_series(&[ev(1, ts(2026, 7, 1, 12), 60)], &Utc, Bucket::Week);
        assert!(series.contains_key(&date("2026-06-29")));
    }

    #[test]
    fn months_key_on_the_first() {
        let events = vec![
            ev(1, ts(2026, 7, 1, 12), 60),
            ev(2, ts(2026, 7, 30, 12), 60),
        ];
        let series = speed_series(&events, &Utc, Bucket::Month);
        assert_eq!(series.len(), 1);
        assert_eq!(series[&date("2026-07-01")].pages, 2);
    }

    #[test]
    fn rereads_count_once_per_bucket() {
        let events = vec![ev(1, ts(2026, 7, 1, 12), 60), ev(1, ts(2026, 7, 1, 13), 60)];
        let series = speed_series(&events, &Utc, Bucket::Day);
        let point = series[&date("2026-07-01")];
        assert_eq!(point.pages, 1);
        assert_eq!(point.seconds, 120);
    }
}

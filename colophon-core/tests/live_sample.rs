//! Cross-checks against the real (gitignored) sample database copied from
//! Brandon's Kindle at `research/samples/statistics.sqlite3`. Skips
//! silently when the sample is absent (fresh clone, CI), so the suite
//! never depends on personal data. Assertions are invariants, not exact
//! values: the sample gets refreshed as the device gets read on.

use std::path::PathBuf;

use chrono::Utc;
use colophon_core::metrics::{self, Bucket};
use colophon_core::model::DEFAULT_SESSION_GAP_SECS;
use colophon_core::{EXPECTED_SCHEMA_VERSION, StatsDb};

fn sample_path() -> Option<PathBuf> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../research/samples/statistics.sqlite3");
    path.exists().then_some(path)
}

#[test]
fn live_sample_reconciles() {
    let Some(path) = sample_path() else {
        eprintln!("live sample not present; skipping");
        return;
    };

    let db = StatsDb::open(&path).unwrap();
    assert_eq!(db.schema_version().unwrap(), EXPECTED_SCHEMA_VERSION);

    let books = db.books().unwrap();
    assert!(!books.is_empty());

    let mut total_events = 0usize;
    for book in &books {
        let events = db.events(book).unwrap();
        total_events += events.len();

        // KOReader maintains total_read_time incrementally; it must equal
        // the raw sum (verified true on the real device, 2026-07-03).
        assert_eq!(
            metrics::uncapped_seconds(&events),
            book.total_read_time,
            "uncapped total mismatch for {:?}",
            book.title
        );

        let coverage = metrics::coverage(&events);
        assert!((0.0..=1.0).contains(&coverage), "coverage out of range");

        let sessions = metrics::sessions(&events, DEFAULT_SESSION_GAP_SECS);
        assert!(sessions.len() <= events.len());
        assert!(
            sessions
                .iter()
                .all(|s| s.seconds > 0 && s.start_time <= s.end_time)
        );

        // The rescaled view redistributes durations; small integer-division
        // losses are inherent to KOReader's view (duration / span), so the
        // rescaled sum can only be <= the raw sum.
        let rescaled = db.rescaled_events(book).unwrap();
        let rescaled_sum: i64 = rescaled.iter().map(|e| e.duration).sum();
        assert!(rescaled_sum <= book.total_read_time);
    }

    let all = db.all_events().unwrap();
    assert_eq!(all.len(), total_events);

    // Whole-history derived metrics must at least compute sanely.
    let totals = metrics::daily_totals(&all, &Utc);
    assert!(!totals.is_empty());
    let days = totals.keys().copied().collect();
    let today = *totals.keys().last().unwrap();
    let streaks = metrics::streaks(&days, today);
    assert!(streaks.longest.is_some());
    assert!(!metrics::speed_series(&all, &Utc, Bucket::Day).is_empty());
}

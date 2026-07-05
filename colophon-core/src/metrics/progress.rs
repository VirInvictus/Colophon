//! Page-coverage and reading-time totals.
//!
//! Coverage uses interval union on a normalized page axis (KoInsight's
//! idea, RESEARCH.md §5.1): each event's page, out of the page count *it
//! was recorded against*, becomes a fractional span; the merged span
//! length is the fraction of the book actually visited. Re-reading pages
//! or changing font size cannot inflate it.
//!
//! Capped totals reproduce KOReader's own `STATISTICS_SQL_BOOK_CAPPED_
//! TOTALS_QUERY` (per distinct page, total time clamped to `max_sec`);
//! KOReader's on-screen "time spent reading" and its sec/page `avg_time`
//! come from these, so Colophon must use them wherever it mirrors those
//! numbers or the app won't match the device.

use std::collections::BTreeMap;

use crate::model::PageEvent;

/// The merged read intervals on the `[0, 1]` page axis (spec.md
/// "Read-span coverage (positional)"): sorted, non-overlapping spans of
/// which parts of the book `events` actually visited.
///
/// Each event contributes the span `[(page-1)/total, page/total]` using
/// its *own* recorded `total_pages`. Events with a nonpositive
/// `total_pages` or a page outside `1..=total_pages` are skipped
/// defensively; both would be corrupt rows. Empty when nothing valid.
pub fn coverage_spans(events: &[PageEvent]) -> Vec<(f64, f64)> {
    let mut spans: Vec<(f64, f64)> = events
        .iter()
        .filter(|e| e.total_pages > 0 && e.page >= 1 && e.page <= e.total_pages)
        .map(|e| {
            let total = e.total_pages as f64;
            ((e.page - 1) as f64 / total, e.page as f64 / total)
        })
        .collect();
    if spans.is_empty() {
        return Vec::new();
    }

    spans.sort_by(|a, b| a.partial_cmp(b).expect("spans are finite"));

    let mut merged = Vec::new();
    let (mut lo, mut hi) = spans[0];
    for &(next_lo, next_hi) in &spans[1..] {
        // Tolerance for float error at shared page boundaries.
        if next_lo <= hi + 1e-9 {
            hi = hi.max(next_hi);
        } else {
            merged.push((lo, hi.min(1.0)));
            (lo, hi) = (next_lo, next_hi);
        }
    }
    merged.push((lo, hi.min(1.0)));
    merged
}

/// Fraction (0..=1) of a book's page axis covered by `events`: the total
/// length of the merged read spans. A coverage measure, not progress; see
/// [`furthest_position`].
pub fn coverage(events: &[PageEvent]) -> f64 {
    coverage_spans(events)
        .iter()
        .map(|(lo, hi)| hi - lo)
        .sum::<f64>()
        .min(1.0)
}

/// The deepest fractional position (0..=1) any event reached: the largest
/// merged-span upper bound (spec.md "Furthest position reached"). This is
/// the *progress* measure, unaffected by an unlogged leading gap, so a
/// book read to its final page reports 1.0 even when [`coverage`] is far
/// below that. 0 when no valid events.
pub fn furthest_position(events: &[PageEvent]) -> f64 {
    coverage_spans(events)
        .last()
        .map(|&(_, hi)| hi)
        .unwrap_or(0.0)
}

/// `coverage` scaled to a concrete page count, rounded.
pub fn unique_pages_read(coverage: f64, pages: i64) -> i64 {
    (coverage * pages as f64).round() as i64
}

/// A recorded page (out of `total_pages` at record time) mapped onto the
/// book's `current_pages` axis, matching the `page_stat` view's own
/// arithmetic (RESEARCH.md §1): the view fans a stored row across
/// `first_page..=last_page`, and this returns that `last_page`, the highest
/// current-axis page the event touches. Used to recover the most recently
/// read page from the latest raw event without scanning the fanned-out
/// view a second time.
pub fn rescaled_last_page(page: i64, total_pages: i64, current_pages: i64) -> i64 {
    if total_pages <= 0 {
        return page;
    }
    let first = (page - 1) * current_pages / total_pages + 1;
    first.max(page * current_pages / total_pages)
}

/// Plain `sum(duration)`: the default "time read" everywhere.
pub fn uncapped_seconds(events: &[PageEvent]) -> i64 {
    events.iter().map(|e| e.duration).sum()
}

/// KOReader-parity capped totals over `(page, duration)` pairs (pass
/// rescaled-view rows for exact device parity, like KOReader does):
/// per distinct page, total time clamped to `max_sec`. Returns
/// `(seconds, distinct_pages)`.
pub fn capped_seconds(pairs: impl IntoIterator<Item = (i64, i64)>, max_sec: i64) -> (i64, i64) {
    let mut per_page: BTreeMap<i64, i64> = BTreeMap::new();
    for (page, duration) in pairs {
        *per_page.entry(page).or_default() += duration;
    }
    let pages = per_page.len() as i64;
    let seconds = per_page.values().map(|&secs| secs.min(max_sec)).sum();
    (seconds, pages)
}

/// KOReader's `avg_time` (seconds per page, from capped totals); feeds its
/// time-left and finish-date estimates.
pub fn avg_seconds_per_page(capped_seconds: i64, distinct_pages: i64) -> Option<f64> {
    (distinct_pages > 0).then(|| capped_seconds as f64 / distinct_pages as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(page: i64, total_pages: i64) -> PageEvent {
        PageEvent {
            book_id: 1,
            page,
            start_time: 0,
            duration: 10,
            total_pages,
        }
    }

    #[test]
    fn contiguous_pages_cover_their_fraction() {
        // Pages 1..=50 of 100.
        let events: Vec<_> = (1..=50).map(|p| ev(p, 100)).collect();
        assert!((coverage(&events) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn rereads_do_not_inflate_coverage() {
        let mut events: Vec<_> = (1..=50).map(|p| ev(p, 100)).collect();
        events.extend((1..=50).map(|p| ev(p, 100)));
        assert!((coverage(&events) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn pagination_drift_is_commensurable() {
        // First half read at 100 pages, the same physical half re-read at
        // 200 pages after a font change: still half the book.
        let mut events: Vec<_> = (1..=50).map(|p| ev(p, 100)).collect();
        events.extend((1..=100).map(|p| ev(p, 200)));
        assert!((coverage(&events) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn disjoint_spans_sum() {
        // Pages 1..=10 and 91..=100 of 100.
        let mut events: Vec<_> = (1..=10).map(|p| ev(p, 100)).collect();
        events.extend((91..=100).map(|p| ev(p, 100)));
        assert!((coverage(&events) - 0.2).abs() < 1e-9);
    }

    #[test]
    fn corrupt_rows_are_skipped() {
        assert_eq!(coverage(&[ev(5, 0), ev(0, 100), ev(101, 100)]), 0.0);
        assert!(coverage_spans(&[ev(5, 0)]).is_empty());
        assert_eq!(furthest_position(&[ev(5, 0)]), 0.0);
    }

    #[test]
    fn coverage_spans_merge_and_leave_gaps() {
        // Read pages 1..=10 and 91..=100 of 100: two spans with a gap.
        let mut events: Vec<_> = (1..=10).map(|p| ev(p, 100)).collect();
        events.extend((91..=100).map(|p| ev(p, 100)));
        let spans = coverage_spans(&events);
        assert_eq!(spans.len(), 2);
        assert!((spans[0].0 - 0.0).abs() < 1e-9 && (spans[0].1 - 0.10).abs() < 1e-9);
        assert!((spans[1].0 - 0.90).abs() < 1e-9 && (spans[1].1 - 1.00).abs() < 1e-9);
    }

    #[test]
    fn furthest_ignores_a_leading_gap() {
        // The Royal Assassin shape: nothing until ~29%, then read to the
        // end. Coverage is partial, but furthest reaches the last page.
        let events: Vec<_> = (30..=100).map(|p| ev(p, 100)).collect();
        assert!((coverage(&events) - 0.71).abs() < 1e-9);
        assert!((furthest_position(&events) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn unique_pages_rounds() {
        assert_eq!(unique_pages_read(0.5, 867), 434);
        assert_eq!(unique_pages_read(0.0, 867), 0);
    }

    #[test]
    fn rescaled_last_page_matches_the_view_formula() {
        // No repagination: the page maps to itself.
        assert_eq!(rescaled_last_page(20, 100, 100), 20);
        // Recorded at 100 pages, now rendered at 200: page 20 spans the
        // current-axis pages 39..=40, so its last page is 40.
        assert_eq!(rescaled_last_page(20, 100, 200), 40);
        // Recorded at 200, now 100: page 40 collapses onto page 20.
        assert_eq!(rescaled_last_page(40, 200, 100), 20);
        // Degenerate rows pass the page through unchanged.
        assert_eq!(rescaled_last_page(7, 0, 100), 7);
    }

    #[test]
    fn capped_totals_clamp_per_page() {
        // Page 1: 100 + 50 = 150, clamped to 120. Page 2: 30 stays.
        let (secs, pages) = capped_seconds([(1, 100), (1, 50), (2, 30)], 120);
        assert_eq!(secs, 150);
        assert_eq!(pages, 2);
        assert_eq!(avg_seconds_per_page(secs, pages), Some(75.0));
    }

    #[test]
    fn avg_time_undefined_without_pages() {
        assert_eq!(avg_seconds_per_page(0, 0), None);
    }
}

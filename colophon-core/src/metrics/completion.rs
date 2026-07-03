//! Inferred read-through detection, adopted from KoShelf
//! (`compute/completion_detection.rs` there; RESEARCH.md §5.2).
//!
//! KOReader's database has no "finished" flag, so completions must be
//! inferred from the event stream. A read-through is a progression of
//! events covering enough of the book (78 %) including both an early page
//! (first 20 %) and a late page (last 2 %); the 78 % floor exists because
//! even a cover-to-cover read rarely logs every page. A jump back to the
//! start (first 5 %) splits a new progression only when the events from
//! that point onward would form a valid completion by themselves, which
//! distinguishes a genuine re-read from flipping back to check a map or
//! re-reading a chapter.
//!
//! Positions are normalized per event against the event's own recorded
//! `total_pages`, so a re-read at a different font size is detected just
//! as well as the first read.

use crate::metrics::progress::{coverage, unique_pages_read};
use crate::metrics::sessions::sessions;
use crate::model::{Completion, PageEvent};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompletionConfig {
    /// Minimum fraction of the page axis a progression must cover.
    pub min_coverage: f64,
    /// A valid completion must touch a page starting in this early
    /// fraction of the book...
    pub early_fraction: f64,
    /// ...and a page ending in this final fraction.
    pub late_fraction: f64,
    /// A backwards jump into this leading fraction is a restart candidate.
    pub restart_fraction: f64,
    /// Session gap used for the per-completion session count.
    pub session_gap_secs: i64,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            min_coverage: 0.78,
            early_fraction: 0.20,
            late_fraction: 0.02,
            restart_fraction: 0.05,
            session_gap_secs: crate::model::DEFAULT_SESSION_GAP_SECS,
        }
    }
}

/// Detects read-throughs in one book's events. `pages` is the book's
/// current page count, used only to express `coverage` as page numbers.
pub fn completions(events: &[PageEvent], pages: i64, config: &CompletionConfig) -> Vec<Completion> {
    let mut clean: Vec<PageEvent> = events
        .iter()
        .filter(|e| e.duration > 0 && e.total_pages > 0 && e.page >= 1 && e.page <= e.total_pages)
        .copied()
        .collect();
    clean.sort_by_key(|e| e.start_time);

    let mut progressions: Vec<&[PageEvent]> = Vec::new();
    let mut current_start = 0;
    let mut touched_early = false;

    for i in 0..clean.len() {
        let frac_hi = clean[i].page as f64 / clean[i].total_pages as f64;
        if i > current_start {
            let prev = &clean[i - 1];
            let prev_lo = (prev.page - 1) as f64 / prev.total_pages as f64;
            let is_restart_jump =
                frac_hi <= config.restart_fraction && prev_lo > config.early_fraction;
            if is_restart_jump && touched_early && is_valid(&clean[i..], config) {
                progressions.push(&clean[current_start..i]);
                current_start = i;
                touched_early = false;
            }
        }
        let frac_lo = (clean[i].page - 1) as f64 / clean[i].total_pages as f64;
        touched_early |= frac_lo <= config.early_fraction;
    }
    if current_start < clean.len() {
        progressions.push(&clean[current_start..]);
    }

    progressions
        .into_iter()
        .filter(|p| is_valid(p, config))
        .map(|p| {
            let cov = coverage(p);
            let seconds: i64 = p.iter().map(|e| e.duration).sum();
            let last = p.last().expect("progression is non-empty");
            let pages_read = unique_pages_read(cov, pages);
            Completion {
                start_time: p[0].start_time,
                end_time: last.start_time + last.duration,
                seconds,
                sessions: sessions(p, config.session_gap_secs).len() as u32,
                coverage: cov,
                pages_read,
                pages_per_hour: if seconds > 0 {
                    pages_read as f64 / (seconds as f64 / 3600.0)
                } else {
                    0.0
                },
            }
        })
        .collect()
}

fn is_valid(events: &[PageEvent], config: &CompletionConfig) -> bool {
    if events.is_empty() {
        return false;
    }
    let mut has_early = false;
    let mut has_late = false;
    for e in events {
        let total = e.total_pages as f64;
        has_early |= (e.page - 1) as f64 / total <= config.early_fraction;
        has_late |= e.page as f64 / total >= 1.0 - config.late_fraction;
    }
    has_early && has_late && coverage(events) >= config.min_coverage
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;

    /// A straight read of pages `range` (of 100), one page a minute,
    /// starting at `t0`.
    fn run(range: std::ops::RangeInclusive<i64>, t0: i64) -> Vec<PageEvent> {
        range
            .enumerate()
            .map(|(i, page)| PageEvent {
                book_id: 1,
                page,
                start_time: t0 + i as i64 * 60,
                duration: 60,
                total_pages: 100,
            })
            .collect()
    }

    #[test]
    fn full_read_is_one_completion() {
        let events = run(1..=100, 0);
        let got = completions(&events, 100, &CompletionConfig::default());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pages_read, 100);
        assert!((got[0].coverage - 1.0).abs() < 1e-9);
        assert!((got[0].pages_per_hour - 60.0).abs() < 1e-9);
    }

    #[test]
    fn gappy_read_still_counts() {
        // Skimmed: every page except a 15-page dead zone in the middle
        // (85 % coverage), with first and last pages present.
        let mut events = run(1..=60, 0);
        events.extend(run(76..=100, 100 * HOUR));
        let got = completions(&events, 100, &CompletionConfig::default());
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn abandoned_book_is_no_completion() {
        let events = run(1..=40, 0);
        assert!(completions(&events, 100, &CompletionConfig::default()).is_empty());
    }

    #[test]
    fn missing_the_ending_is_no_completion() {
        // 90 % coverage but never reaches the last 2 %.
        let events = run(1..=90, 0);
        assert!(completions(&events, 100, &CompletionConfig::default()).is_empty());
    }

    #[test]
    fn reread_splits_into_two_completions() {
        let mut events = run(1..=100, 0);
        events.extend(run(1..=100, 1000 * HOUR));
        let got = completions(&events, 100, &CompletionConfig::default());
        assert_eq!(got.len(), 2);
        assert!(got[0].end_time <= got[1].start_time);
    }

    #[test]
    fn flipping_back_to_a_map_does_not_split() {
        // Mid-read, briefly revisit page 2 (a map), then continue. The
        // remainder alone (2, 61..=100) is not a valid completion, so no
        // split; the whole thing is one read-through.
        let mut events = run(1..=60, 0);
        events.push(PageEvent {
            book_id: 1,
            page: 2,
            start_time: 61 * 60,
            duration: 30,
            total_pages: 100,
        });
        events.extend(run(61..=100, 62 * 60));
        let got = completions(&events, 100, &CompletionConfig::default());
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn reread_at_a_different_font_size_is_detected() {
        // First read at 100 pages, re-read at 200 (font change).
        let mut events = run(1..=100, 0);
        events.extend((1..=200).map(|page| PageEvent {
            book_id: 1,
            page,
            start_time: 1000 * HOUR + page * 60,
            duration: 60,
            total_pages: 200,
        }));
        let got = completions(&events, 100, &CompletionConfig::default());
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn session_count_is_carried_per_completion() {
        // Two sessions separated by an hour.
        let mut events = run(1..=50, 0);
        events.extend(run(51..=100, 50 * 60 + HOUR));
        let got = completions(&events, 100, &CompletionConfig::default());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].sessions, 2);
    }
}

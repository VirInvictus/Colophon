//! Session detection.
//!
//! A session is a per-book cluster of page events where each event starts
//! no more than `gap_secs` after the previous event's *end*
//! (`start_time + duration`). 300 s is the convention KoShelf established
//! and the only real implementation in the wild; sessions never span
//! books. See `spec.md` "Derived-metric definitions".

use std::collections::{BTreeSet, HashMap};

use crate::model::{PageEvent, Session};

/// Clusters `events` (any order, any mix of books) into sessions, returned
/// in chronological order.
pub fn sessions(events: &[PageEvent], gap_secs: i64) -> Vec<Session> {
    let mut per_book: HashMap<i64, Vec<&PageEvent>> = HashMap::new();
    for event in events {
        if event.duration > 0 {
            per_book.entry(event.book_id).or_default().push(event);
        }
    }

    let mut out = Vec::new();
    for (book_id, mut book_events) in per_book {
        book_events.sort_by_key(|e| e.start_time);

        let mut current: Vec<&PageEvent> = Vec::new();
        for event in book_events {
            if let Some(last) = current.last() {
                let last_end = last.start_time + last.duration;
                if event.start_time - last_end > gap_secs {
                    out.push(finish(book_id, &current));
                    current.clear();
                }
            }
            current.push(event);
        }
        if !current.is_empty() {
            out.push(finish(book_id, &current));
        }
    }

    out.sort_by_key(|s| s.start_time);
    out
}

fn finish(book_id: i64, events: &[&PageEvent]) -> Session {
    let first = events.first().expect("session has at least one event");
    let last = events.last().expect("session has at least one event");
    let pages: BTreeSet<i64> = events.iter().map(|e| e.page).collect();
    Session {
        book_id,
        start_time: first.start_time,
        end_time: last.start_time + last.duration,
        seconds: events.iter().map(|e| e.duration).sum(),
        events: events.len() as u32,
        pages: pages.len() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(book_id: i64, page: i64, start_time: i64, duration: i64) -> PageEvent {
        PageEvent {
            book_id,
            page,
            start_time,
            duration,
            total_pages: 100,
        }
    }

    #[test]
    fn single_run_is_one_session() {
        let events = vec![ev(1, 1, 0, 60), ev(1, 2, 60, 60), ev(1, 3, 120, 30)];
        let got = sessions(&events, 300);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].seconds, 150);
        assert_eq!(got[0].events, 3);
        assert_eq!(got[0].pages, 3);
        assert_eq!(got[0].end_time, 150);
    }

    #[test]
    fn gap_is_measured_from_previous_end_not_start() {
        // Second event starts 350 s after the first *starts*, but only
        // 250 s after it ends: same session.
        let same = sessions(&[ev(1, 1, 0, 100), ev(1, 2, 350, 10)], 300);
        assert_eq!(same.len(), 1);

        // 301 s after the first ends: new session.
        let split = sessions(&[ev(1, 1, 0, 100), ev(1, 2, 401, 10)], 300);
        assert_eq!(split.len(), 2);
    }

    #[test]
    fn sessions_never_span_books() {
        // Interleaved books reading "at the same time" (e.g. switching
        // between a novel and a reference) stay separate sessions.
        let events = vec![ev(1, 1, 0, 60), ev(2, 1, 60, 60), ev(1, 2, 120, 60)];
        let got = sessions(&events, 300);
        assert_eq!(got.len(), 2);
        assert_eq!(got.iter().filter(|s| s.book_id == 1).count(), 1);
        assert_eq!(got.iter().filter(|s| s.book_id == 2).count(), 1);
    }

    #[test]
    fn zero_duration_events_are_ignored() {
        let got = sessions(&[ev(1, 1, 0, 0)], 300);
        assert!(got.is_empty());
    }

    #[test]
    fn rereading_a_page_counts_once_for_pages() {
        let events = vec![ev(1, 5, 0, 60), ev(1, 5, 60, 60), ev(1, 6, 120, 60)];
        let got = sessions(&events, 300);
        assert_eq!(got[0].pages, 2);
        assert_eq!(got[0].events, 3);
    }
}

//! Display-side library model: junk filtering and same-title grouping.
//! Pure and GTK-free. Grouping is display-only (spec.md "Book identity"):
//! two files of the same work (same title/authors, different md5s) sit
//! together in the list but stay separate entries.
//!
//! Entries are shared as `Rc`: the loader builds them once per import and
//! every refilter/regroup just clones pointers, not event vectors.

use std::collections::HashMap;
use std::rc::Rc;

use colophon_core::sidecar::ReadStatus;
use colophon_core::{Book, PageEvent, PageTotal, metrics};

use crate::stats::FINISHED_THRESHOLD;

#[derive(Debug, Clone)]
pub struct LibraryEntry {
    pub book: Book,
    /// Interval-union unique pages read, out of `book.pages`.
    pub unique_pages: i64,
    /// Raw page-turn events (time axis), chronological.
    pub events: Vec<PageEvent>,
    /// Per current-axis page aggregates from the `page_stat` view (one row
    /// per page, not the fanned-out rows); feeds the activity strip. The
    /// view itself is never materialized in memory.
    pub page_totals: Vec<PageTotal>,
    /// KOReader-parity numbers derived from the `page_stat` view at load
    /// time (the device's own math runs on the view): capped total
    /// seconds, distinct pages on the current page axis, and the most
    /// recently read page.
    pub capped_secs: i64,
    pub view_pages: i64,
    pub last_page: i64,
    /// User-declared status from the book's `.sdr` sidecar, when a library
    /// folder is configured and the sidecar was found; `None` otherwise.
    pub declared_status: Option<ReadStatus>,
}

impl LibraryEntry {
    /// Whether the book is finished. The sidecar's declared status is
    /// authoritative when present (spec.md); otherwise fall back to the
    /// inferred furthest-position heuristic. This is the single source of
    /// "finished" for every aggregate and marker.
    pub fn is_finished(&self) -> bool {
        match &self.declared_status {
            Some(status) => status.is_finished(),
            None => metrics::furthest_position(&self.events) >= FINISHED_THRESHOLD,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LibraryGroup {
    pub title: String,
    pub authors: String,
    pub entries: Vec<Rc<LibraryEntry>>,
}

impl LibraryGroup {
    pub fn is_multi(&self) -> bool {
        self.entries.len() > 1
    }
}

fn group_key(book: &Book) -> (String, String) {
    (book.title.trim().to_owned(), book.authors.trim().to_owned())
}

/// Junk-filters, groups by (title, authors), and orders: groups by their
/// most recently opened member (desc), members within a group likewise.
pub fn grouped(
    entries: &[Rc<LibraryEntry>],
    junk_filter: bool,
    junk_threshold_secs: i64,
) -> Vec<LibraryGroup> {
    let mut groups: Vec<LibraryGroup> = Vec::new();
    let mut index: HashMap<(String, String), usize> = HashMap::new();

    for entry in entries {
        if junk_filter && entry.book.is_junk(junk_threshold_secs) {
            continue;
        }
        let key = group_key(&entry.book);
        match index.get(&key) {
            Some(&i) => groups[i].entries.push(Rc::clone(entry)),
            None => {
                index.insert(key.clone(), groups.len());
                groups.push(LibraryGroup {
                    title: key.0,
                    authors: key.1,
                    entries: vec![Rc::clone(entry)],
                });
            }
        }
    }

    for group in &mut groups {
        group.entries.sort_by_key(|e| -e.book.last_open);
    }
    groups.sort_by_key(|g| {
        -g.entries
            .iter()
            .map(|e| e.book.last_open)
            .max()
            .unwrap_or(0)
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use colophon_core::model::DEFAULT_JUNK_THRESHOLD_SECS;

    fn book(
        title: &str,
        authors: &str,
        md5: &str,
        read_secs: i64,
        last_open: i64,
    ) -> Rc<LibraryEntry> {
        Rc::new(LibraryEntry {
            book: Book {
                id: 0,
                all_ids: vec![0],
                title: title.into(),
                authors: authors.into(),
                notes: 0,
                highlights: 0,
                pages: 100,
                series: None,
                language: None,
                md5: Some(md5.into()),
                total_read_time: read_secs,
                total_read_pages: 0,
                last_open,
            },
            unique_pages: 0,
            events: Vec::new(),
            page_totals: Vec::new(),
            capped_secs: 0,
            view_pages: 0,
            last_page: 0,
            declared_status: None,
        })
    }

    #[test]
    fn same_title_two_files_group_without_merging() {
        let entries = vec![
            book("Novel Two", "Author Two", "aaaa", 632, 200),
            book("Novel Two", "Author Two", "bbbb", 400, 300),
        ];
        let groups = grouped(&entries, true, DEFAULT_JUNK_THRESHOLD_SECS);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].is_multi());
        assert_eq!(groups[0].entries.len(), 2);
        // Most recently opened member first; both md5s survive.
        assert_eq!(groups[0].entries[0].book.md5.as_deref(), Some("bbbb"));
        assert_eq!(groups[0].entries[1].book.md5.as_deref(), Some("aaaa"));
    }

    #[test]
    fn different_titles_stay_separate() {
        let entries = vec![
            book("Novel Two", "Author Two", "aaaa", 632, 200),
            book("Novel One", "Author One", "cccc", 36_047, 100),
        ];
        let groups = grouped(&entries, true, DEFAULT_JUNK_THRESHOLD_SECS);
        assert_eq!(groups.len(), 2);
        assert!(!groups[0].is_multi());
    }

    #[test]
    fn junk_filter_drops_short_reads() {
        let entries = vec![
            book("Novel One", "Author One", "cccc", 36_047, 100),
            book("Some Plugin README", "N/A", "dddd", 40, 400),
        ];
        let on = grouped(&entries, true, DEFAULT_JUNK_THRESHOLD_SECS);
        assert_eq!(on.len(), 1);
        assert_eq!(on[0].title, "Novel One");

        let off = grouped(&entries, false, DEFAULT_JUNK_THRESHOLD_SECS);
        assert_eq!(off.len(), 2);
    }

    #[test]
    fn junk_member_drops_out_of_its_group() {
        // One copy read seriously, one barely touched: with the filter on
        // the group collapses to a singleton.
        let entries = vec![
            book("Novel Two", "Author Two", "aaaa", 632, 200),
            book("Novel Two", "Author Two", "bbbb", 10, 300),
        ];
        let groups = grouped(&entries, true, DEFAULT_JUNK_THRESHOLD_SECS);
        assert_eq!(groups.len(), 1);
        assert!(!groups[0].is_multi());
    }

    #[test]
    fn groups_order_by_most_recent_member() {
        let entries = vec![
            book("Old", "A", "aaaa", 1000, 100),
            book("New", "B", "bbbb", 1000, 900),
            book("Old", "A", "cccc", 1000, 950), // second copy bumps the group
        ];
        let groups = grouped(&entries, true, DEFAULT_JUNK_THRESHOLD_SECS);
        assert_eq!(groups[0].title, "Old");
        assert_eq!(groups[1].title, "New");
    }

    #[test]
    fn whitespace_variants_group_together() {
        let entries = vec![
            book("Novel Two ", "Author Two", "aaaa", 1000, 100),
            book("Novel Two", "Author Two ", "bbbb", 1000, 200),
        ];
        let groups = grouped(&entries, true, DEFAULT_JUNK_THRESHOLD_SECS);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].is_multi());
    }

    #[test]
    fn empty_in_empty_out() {
        assert!(grouped(&[], true, DEFAULT_JUNK_THRESHOLD_SECS).is_empty());
    }
}

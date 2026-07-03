//! Plain data types shared across the crate.
//!
//! Everything here is a passive value type; all behaviour lives in `db` and
//! `metrics`. Field semantics follow `spec.md`'s normative definitions.

use chrono::NaiveDate;

/// KOReader's default page-turn duration floor (events shorter are
/// discarded at record time) and ceiling (longer are clamped). Colophon
/// never re-derives these from data; they matter when reproducing
/// KOReader's capped-total queries.
pub const KOREADER_DEFAULT_MIN_SEC: i64 = 5;
pub const KOREADER_DEFAULT_MAX_SEC: i64 = 120;

/// Gap that opens a new reading session (see `metrics::sessions`).
pub const DEFAULT_SESSION_GAP_SECS: i64 = 300;

/// Books with less total read time than this are junk-filtered from
/// library-wide views by default (plugin READMEs etc. show up as "books").
pub const DEFAULT_JUNK_THRESHOLD_SECS: i64 = 300;

/// One row of KOReader's `book` table, after md5-merging (see
/// [`crate::StatsDb::books`]). Metadata edits in KOReader create a second
/// row for the same file (its unique index is `(title, authors, md5)`);
/// rows sharing an md5 are one book.
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    /// Canonical row id: of the merged rows, the one most recently opened.
    pub id: i64,
    /// Every `book.id` this book covers (canonical first). Event queries
    /// must match against all of them.
    pub all_ids: Vec<i64>,
    pub title: String,
    pub authors: String,
    /// Count of notes, not content (content lives in `.sdr` sidecars).
    pub notes: i64,
    /// Count of highlights, not content.
    pub highlights: i64,
    /// Page count under the *current* rendering (font size etc.).
    pub pages: i64,
    pub series: Option<String>,
    pub language: Option<String>,
    /// KOReader's partial-MD5 content hash; the stable book identity.
    pub md5: Option<String>,
    /// Uncapped cumulative seconds, maintained by KOReader itself.
    pub total_read_time: i64,
    /// Cumulative page-*turn* counter (re-reads increment it), not unique
    /// pages read. Never treat as progress.
    pub total_read_pages: i64,
    /// Unix seconds; 0 when KOReader never stamped one.
    pub last_open: i64,
}

impl Book {
    /// Junk heuristic for library-wide views (see `spec.md`).
    pub fn is_junk(&self, threshold_secs: i64) -> bool {
        self.total_read_time < threshold_secs
    }
}

/// One raw row of `page_stat_data`: a single page-turn event.
///
/// `duration` is already clamped to KOReader's `[min_sec, max_sec]` at
/// record time. `total_pages` is the book's page count *when the event was
/// recorded*, which is how KOReader survives font-size repagination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageEvent {
    pub book_id: i64,
    pub page: i64,
    pub start_time: i64,
    pub duration: i64,
    pub total_pages: i64,
}

/// One row of the `page_stat` view: the same events rescaled by KOReader
/// onto the book's *current* page count, so the page axis is stable across
/// the book's whole history. Use this for anything page-positional; use
/// [`PageEvent`] when real timestamps/durations matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RescaledEvent {
    pub book_id: i64,
    pub page: i64,
    pub start_time: i64,
    pub duration: i64,
}

/// A reading session: a per-book cluster of events (see `metrics::sessions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    pub book_id: i64,
    /// Start of the first event.
    pub start_time: i64,
    /// End (`start_time + duration`) of the last event.
    pub end_time: i64,
    /// Sum of member durations (not `end_time - start_time`; suspends and
    /// sub-`min_sec` turns leave gaps inside a session).
    pub seconds: i64,
    pub events: u32,
    /// Distinct pages touched.
    pub pages: u32,
}

/// Aggregate for one local calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DayTotal {
    pub seconds: i64,
    pub events: u32,
    /// Distinct (book, page) pairs.
    pub pages: u32,
    /// Distinct books.
    pub books: u32,
}

/// A run of consecutive reading days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Streak {
    pub days: u32,
    pub start: NaiveDate,
    pub end: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Streaks {
    /// `None` when the last reading day is before yesterday.
    pub current: Option<Streak>,
    pub longest: Option<Streak>,
}

/// One point of a reading-speed series (see `metrics::speed`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedPoint {
    /// Distinct (book, page) pairs in the bucket.
    pub pages: u32,
    /// Uncapped seconds in the bucket.
    pub seconds: i64,
    pub pages_per_hour: f64,
}

/// An inferred read-through of one book (see `metrics::completion`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Completion {
    pub start_time: i64,
    pub end_time: i64,
    /// Uncapped reading seconds inside this read-through.
    pub seconds: i64,
    pub sessions: u32,
    /// Fraction of the book's page axis covered, 0..=1.
    pub coverage: f64,
    /// `coverage` scaled to the book's current page count, rounded.
    pub pages_read: i64,
    pub pages_per_hour: f64,
}

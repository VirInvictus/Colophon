//! Read-only access to a copy of KOReader's `statistics.sqlite3`.
//!
//! Hard rule (see `CLAUDE.md`): Colophon never opens KOReader's live
//! database file, even read-only, and never writes to a path KOReader
//! owns. [`snapshot`] copies the live file (plain filesystem copy, no
//! SQLite connection to the source); [`StatsDb::open`] then only ever
//! opens such copies with `SQLITE_OPEN_READ_ONLY`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};

use crate::model::{Book, PageEvent, PageTotal, RescaledEvent};

/// The `PRAGMA user_version` this crate was written against (the schema on
/// Brandon's device). Older databases exist in the wild; `open` surfaces
/// the version so callers can warn rather than misread.
pub const EXPECTED_SCHEMA_VERSION: i64 = 20221111;

#[derive(Debug)]
pub struct StatsDb {
    conn: Connection,
}

impl StatsDb {
    /// Opens a local copy of a KOReader `statistics.sqlite3`, read-only.
    ///
    /// Fails (rather than creating an empty database) when the file is
    /// missing, and fails early when the file lacks the expected tables.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("opening {} read-only", path.display()))?;

        let db = Self { conn };
        for required in ["book", "page_stat_data"] {
            if !db.has_table(required)? {
                bail!(
                    "{} is not a KOReader statistics database (missing table `{required}`)",
                    path.display()
                );
            }
        }
        Ok(db)
    }

    fn has_table(&self, name: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// KOReader's schema version (`PRAGMA user_version`), e.g. 20221111.
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    /// All books, md5-merged.
    ///
    /// KOReader's `book` unique index is `(title, authors, md5)`, so
    /// editing a book's metadata creates a second row for the same file.
    /// Rows sharing a non-empty md5 are merged: the most recently opened
    /// row is canonical, read time/pages are summed, note/highlight counts
    /// take the max (they are point-in-time counts, not increments).
    /// Same-title books with *different* md5s (two files of one work) stay
    /// separate; grouping those is a display decision, not a data one.
    pub fn books(&self) -> Result<Vec<Book>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, authors, notes, highlights, pages, series, language, md5,
                    total_read_time, total_read_pages, last_open
             FROM book ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Book {
                    id: row.get(0)?,
                    all_ids: vec![row.get(0)?],
                    title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    authors: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    notes: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    highlights: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    pages: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    series: row.get(6)?,
                    language: row.get(7)?,
                    md5: row.get(8)?,
                    total_read_time: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
                    total_read_pages: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                    last_open: row.get::<_, Option<i64>>(11)?.unwrap_or(0),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(merge_by_md5(rows))
    }

    /// Raw page-turn events for one (merged) book, ordered by time.
    pub fn events(&self, book: &Book) -> Result<Vec<PageEvent>> {
        let mut out = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT id_book, page, start_time, duration, total_pages
             FROM page_stat_data WHERE id_book = ?1 ORDER BY start_time",
        )?;
        for id in &book.all_ids {
            let rows = stmt
                .query_map([id], event_from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            out.extend(rows);
        }
        out.sort_by_key(|e| e.start_time);
        Ok(out)
    }

    /// Every raw page-turn event in the database, ordered by time.
    pub fn all_events(&self) -> Result<Vec<PageEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id_book, page, start_time, duration, total_pages
             FROM page_stat_data ORDER BY start_time",
        )?;
        let rows = stmt
            .query_map([], event_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Per current-axis page aggregates from the `page_stat` view, ordered
    /// by page: the `GROUP BY page` reduction that replaces pulling the
    /// fanned-out view into memory (the view expands each stored row across
    /// the `numbers` join, up to ~1000x). One row per page instead, at most
    /// `book.pages` of them. Feeds the capped totals, the distinct-page
    /// count, and the per-page activity strip.
    ///
    /// `secs` sums *all* view rows for the page (0-duration rows included,
    /// so the page still counts toward KOReader's capped distinct-page
    /// total); `reads` counts only positive-duration rows (the activity
    /// strip's read count).
    pub fn page_totals(&self, book: &Book) -> Result<Vec<PageTotal>> {
        let sql = format!(
            "SELECT page, SUM(duration) AS secs, SUM(duration > 0) AS reads
             FROM page_stat WHERE id_book IN ({}) GROUP BY page ORDER BY page",
            id_list(book)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(PageTotal {
                    page: row.get(0)?,
                    secs: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    reads: row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Events for one (merged) book from the `page_stat` view: rescaled by
    /// KOReader onto the book's current page count, so the page axis is
    /// stable across font-size changes. Ordered by time.
    pub fn rescaled_events(&self, book: &Book) -> Result<Vec<RescaledEvent>> {
        let mut out = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT id_book, page, start_time, duration
             FROM page_stat WHERE id_book = ?1 ORDER BY start_time",
        )?;
        for id in &book.all_ids {
            let rows = stmt
                .query_map([id], |row| {
                    Ok(RescaledEvent {
                        book_id: row.get(0)?,
                        page: row.get(1)?,
                        start_time: row.get(2)?,
                        duration: row.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            out.extend(rows);
        }
        out.sort_by_key(|e| e.start_time);
        Ok(out)
    }
}

/// A comma-separated SQL `IN` list of a merged book's row ids. The ids are
/// our own integer primary keys (never user input), so inlining them is
/// safe and avoids a variable-length bound-parameter dance.
fn id_list(book: &Book) -> String {
    book.all_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PageEvent> {
    Ok(PageEvent {
        book_id: row.get(0)?,
        page: row.get(1)?,
        start_time: row.get(2)?,
        duration: row.get(3)?,
        total_pages: row.get(4)?,
    })
}

fn merge_by_md5(rows: Vec<Book>) -> Vec<Book> {
    let mut by_md5: HashMap<String, Vec<Book>> = HashMap::new();
    let mut merged = Vec::new();

    for book in rows {
        match &book.md5 {
            Some(md5) if !md5.is_empty() => {
                by_md5.entry(md5.to_lowercase()).or_default().push(book);
            }
            // No identity to merge on; pass through as-is.
            _ => merged.push(book),
        }
    }

    for (_, mut group) in by_md5 {
        // Canonical row: most recently opened, id as tie-break.
        group.sort_by_key(|b| (b.last_open, b.id));
        let mut canonical = group.pop().expect("group is never empty");
        for other in &group {
            canonical.total_read_time += other.total_read_time;
            canonical.total_read_pages += other.total_read_pages;
            canonical.notes = canonical.notes.max(other.notes);
            canonical.highlights = canonical.highlights.max(other.highlights);
        }
        canonical.all_ids.extend(group.iter().map(|b| b.id));
        merged.push(canonical);
    }

    merged.sort_by_key(|b| b.id);
    merged
}

/// Copies a `statistics.sqlite3` (plus `-wal`/`-shm` sidecars if present)
/// into `dest_dir` and returns the path of the copied database.
///
/// The source is copied with plain `std::fs::copy`; no SQLite connection
/// ever touches it. The *copy* is then opened briefly read-write to fold
/// any WAL content into the main file (`journal_mode=DELETE`), so later
/// read-only opens see a complete, self-contained database. Writing to our
/// own copy is fine; writing to the source never happens.
pub fn snapshot(source: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<PathBuf> {
    let source = source.as_ref();
    let dest_dir = dest_dir.as_ref();
    let name = source
        .file_name()
        .with_context(|| format!("{} has no file name", source.display()))?;

    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("creating {}", dest_dir.display()))?;
    let dest = dest_dir.join(name);
    std::fs::copy(source, &dest)
        .with_context(|| format!("copying {} to {}", source.display(), dest.display()))?;

    for suffix in ["-wal", "-shm"] {
        let mut side_name = name.to_os_string();
        side_name.push(suffix);
        let side_src = source.with_file_name(&side_name);
        let side_dest = dest_dir.join(&side_name);
        if side_src.exists() {
            std::fs::copy(&side_src, &side_dest).with_context(|| {
                format!("copying {} to {}", side_src.display(), side_dest.display())
            })?;
        } else {
            // A stale sidecar from an earlier snapshot would corrupt the
            // fresh copy on open.
            let _ = std::fs::remove_file(&side_dest);
        }
    }

    let conn = Connection::open(&dest)
        .with_context(|| format!("opening snapshot copy {}", dest.display()))?;
    conn.pragma_update(None, "journal_mode", "DELETE")
        .context("checkpointing snapshot copy")?;
    drop(conn);

    Ok(dest)
}

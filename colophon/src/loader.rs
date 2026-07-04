//! The only app-side code that touches the database, always run inside
//! `gio::spawn_blocking`. `StatsDb` (a rusqlite connection, `!Sync`) is
//! created and dropped inside these functions and never crosses into
//! widget code.
//!
//! Import protocol (the hard rule, mechanized): the user-picked file is
//! NEVER opened in place; `colophon_core::snapshot()` copies it into a
//! staging dir, the staged copy is validated by actually opening it, and
//! only then is it renamed over the canonical snapshot. A bad pick can't
//! clobber a good snapshot, and no SQLite connection ever touches the
//! source.

use std::path::Path;

use anyhow::{Context, Result};
use colophon_core::{StatsDb, metrics};

use crate::library::LibraryEntry;

#[derive(Debug)]
pub struct LibrarySnapshot {
    pub schema_version: i64,
    pub entries: Vec<LibraryEntry>,
}

/// Opens the canonical snapshot read-only and computes the per-book
/// display data: interval-union unique pages from the raw events, plus
/// the KOReader-parity numbers that must come from the rescaled
/// `page_stat` view (capped totals, distinct current-axis pages, last
/// read page) because that is what the device's own queries run on.
pub fn load_snapshot(path: &Path) -> Result<LibrarySnapshot> {
    let db = StatsDb::open(path)?;
    let schema_version = db.schema_version()?;
    let mut entries = Vec::new();
    for book in db.books()? {
        let events = db.events(&book)?;
        let coverage = metrics::coverage(&events);

        let rescaled = db.rescaled_events(&book)?;
        let (capped_secs, view_pages) = metrics::capped_seconds(
            rescaled.iter().map(|e| (e.page, e.duration)),
            colophon_core::model::KOREADER_DEFAULT_MAX_SEC,
        );
        let last_page = rescaled
            .iter()
            .max_by_key(|e| e.start_time)
            .map(|e| e.page)
            .unwrap_or(0);

        entries.push(LibraryEntry {
            unique_pages: metrics::unique_pages_read(coverage, book.pages),
            events,
            rescaled,
            capped_secs,
            view_pages,
            last_page,
            book,
        });
    }
    Ok(LibrarySnapshot {
        schema_version,
        entries,
    })
}

/// Staged import: snapshot `source` into `staging_dir`, validate the
/// staged copy, promote it to `canonical`, then load it.
pub fn import(source: &Path, staging_dir: &Path, canonical: &Path) -> Result<LibrarySnapshot> {
    let staged = colophon_core::snapshot(source, staging_dir)
        .context("copying the database (is the device still mounted?)")?;

    // Validate before promoting: open it and actually read the book table.
    {
        let db = StatsDb::open(&staged)?;
        db.books().context("reading the copied database")?;
    }

    if let Some(parent) = canonical.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    // snapshot() checkpointed the staged copy, so it is a single file; any
    // sidecars next to the canonical path are stale leftovers.
    for suffix in ["-wal", "-shm"] {
        let mut name = canonical.file_name().unwrap_or_default().to_os_string();
        name.push(suffix);
        let _ = std::fs::remove_file(canonical.with_file_name(name));
    }
    std::fs::rename(&staged, canonical)
        .with_context(|| format!("installing snapshot at {}", canonical.display()))?;
    let _ = std::fs::remove_dir_all(staging_dir);

    load_snapshot(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The real (gitignored) Kindle sample; tests skip when absent.
    fn sample() -> Option<PathBuf> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../research/samples/statistics.sqlite3");
        path.exists().then_some(path)
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("colophon-app-test-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn import_and_load_round_trip_on_live_sample() {
        let Some(source) = sample() else {
            eprintln!("live sample not present; skipping");
            return;
        };
        let root = temp_dir("import");
        let staging = root.join("staging");
        let canonical = root.join("statistics.sqlite3");

        let source_mtime = std::fs::metadata(&source).unwrap().modified().unwrap();
        let snap = import(&source, &staging, &canonical).unwrap();

        assert!(canonical.exists());
        assert!(!staging.exists(), "staging dir cleaned after promote");
        assert!(!snap.entries.is_empty());
        for entry in &snap.entries {
            assert!(entry.unique_pages >= 0);
            assert!(entry.unique_pages <= entry.book.pages.max(1));
        }
        // The source was only ever fs-copied, never opened or written.
        assert_eq!(
            std::fs::metadata(&source).unwrap().modified().unwrap(),
            source_mtime
        );

        let reloaded = load_snapshot(&canonical).unwrap();
        assert_eq!(reloaded.entries.len(), snap.entries.len());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn failed_import_leaves_canonical_untouched() {
        let root = temp_dir("failed-import");
        let staging = root.join("staging");
        let canonical = root.join("statistics.sqlite3");
        std::fs::write(&canonical, b"pretend this is a good snapshot").unwrap();

        let bogus = root.join("bogus.sqlite3");
        std::fs::write(&bogus, b"not a database at all").unwrap();

        assert!(import(&bogus, &staging, &canonical).is_err());
        assert_eq!(
            std::fs::read(&canonical).unwrap(),
            b"pretend this is a good snapshot"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}

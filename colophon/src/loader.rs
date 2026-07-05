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
pub fn load_snapshot(path: &Path, sidecar_dir: Option<&Path>) -> Result<LibrarySnapshot> {
    let db = StatsDb::open(path)?;
    let schema_version = db.schema_version()?;
    let mut entries = Vec::new();
    for book in db.books()? {
        let events = db.events(&book)?;
        let coverage = metrics::coverage(&events);

        // Capped totals and the activity strip come from the rescaled
        // `page_stat` view, but as a per-page `GROUP BY` (one row per page)
        // rather than the fanned-out rows: same numbers, a fraction of the
        // memory (RESEARCH §1, the view expands each row up to ~1000x).
        let page_totals = db.page_totals(&book)?;
        let (capped_secs, view_pages) = metrics::capped_seconds(
            page_totals.iter().map(|p| (p.page, p.secs)),
            colophon_core::model::KOREADER_DEFAULT_MAX_SEC,
        );
        // Last read page on the current axis: the latest raw event rescaled
        // like the view would, avoiding a second scan of the fanned-out
        // view just for this one number.
        let last_page = events
            .last()
            .map(|e| metrics::rescaled_last_page(e.page, e.total_pages, book.pages))
            .unwrap_or(0);

        entries.push(LibraryEntry {
            unique_pages: metrics::unique_pages_read(coverage, book.pages),
            events,
            page_totals,
            capped_secs,
            view_pages,
            last_page,
            book,
            declared_status: None,
        });
    }
    // Reconcile the inferred "finished" against the device's own declared
    // status, read from the user-provided `.sdr` sidecars: one file per book,
    // named by the book's md5, that the user copied in themselves. Colophon
    // never reads the device. A book with no sidecar here simply keeps the
    // inference (spec.md).
    if let Some(dir) = sidecar_dir {
        for entry in &mut entries {
            if let Some(md5) = &entry.book.md5 {
                let path = dir.join(format!("{}.lua", md5.to_lowercase()));
                if path.exists()
                    && let Ok(meta) = colophon_core::sidecar::parse_sidecar_file(&path)
                {
                    entry.declared_status = meta.status;
                }
            }
        }
    }
    Ok(LibrarySnapshot {
        schema_version,
        entries,
    })
}

/// Staged import: snapshot `source` into `staging_dir`, validate the
/// staged copy, promote it to `canonical`, then load it.
pub fn import(
    source: &Path,
    staging_dir: &Path,
    canonical: &Path,
    sidecar_dir: Option<&Path>,
) -> Result<LibrarySnapshot> {
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

    load_snapshot(canonical, sidecar_dir)
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
        let snap = import(&source, &staging, &canonical, None).unwrap();

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

        let reloaded = load_snapshot(&canonical, None).unwrap();
        assert_eq!(reloaded.entries.len(), snap.entries.len());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn load_snapshot_reconciles_declared_status_from_sidecars() {
        // Needs both the gitignored stats DB and a `.sdr` sidecar sample.
        let Some(source) = sample() else {
            eprintln!("live sample not present; skipping");
            return;
        };
        let sample_sidecar = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../research/samples/Royal Assassin - Robin Hobb (1705).sdr/metadata.epub.lua");
        if !sample_sidecar.exists() {
            eprintln!("sidecar sample not present; skipping");
            return;
        }
        let root = temp_dir("sidecar-reconcile");
        let canonical = root.join("statistics.sqlite3");
        import(&source, &root.join("staging"), &canonical, None).unwrap();

        // With no sidecar dir, nothing is declared.
        let bare = load_snapshot(&canonical, None).unwrap();
        assert!(bare.entries.iter().all(|e| e.declared_status.is_none()));

        // Copy the real sidecar into a per-book cache named by its md5, the
        // way the app stores what the user hands it.
        let meta = colophon_core::sidecar::parse_sidecar_file(&sample_sidecar).unwrap();
        let md5 = meta.partial_md5.expect("sidecar carries an md5");
        let cache = root.join("sidecars");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::copy(
            &sample_sidecar,
            cache.join(format!("{}.lua", md5.to_lowercase())),
        )
        .unwrap();

        let snap = load_snapshot(&canonical, Some(&cache)).unwrap();
        assert!(
            snap.entries.iter().any(|e| e
                .book
                .md5
                .as_deref()
                .is_some_and(|m| m.eq_ignore_ascii_case(&md5))
                && e.declared_status == Some(colophon_core::sidecar::ReadStatus::Complete)
                && e.is_finished()),
            "the book matching the sidecar md5 should read declared-complete"
        );

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

        assert!(import(&bogus, &staging, &canonical, None).is_err());
        assert_eq!(
            std::fs::read(&canonical).unwrap(),
            b"pretend this is a good snapshot"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}

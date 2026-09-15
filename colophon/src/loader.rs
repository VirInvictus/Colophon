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

/// The epoch window a page turn can honestly fall in (1970-01-01 ..=
/// 9999-12-31 UTC). KOReader stamps real epoch seconds, so timestamps
/// outside it only ever come from a corrupt or foreign database: the
/// extremes are exactly what chrono maps to no instant, which is what
/// the day/hour bucketing used to panic on. `0` is excluded to match
/// the app's own reading of the value (`fmt::relative_date` renders it
/// "never"); negatives predate epoch time and cannot be a page turn.
fn readable_start_time(ts: i64) -> bool {
    (1..=253_402_300_799).contains(&ts)
}

/// Opens the canonical snapshot read-only and computes the per-book
/// display data: interval-union unique pages from the raw events, plus
/// the KOReader-parity numbers (capped totals, distinct canonical-axis
/// pages, last read page). Since D1 those are recomputed in SQL from the
/// raw rows onto the canonical page count (`StatsDb::page_totals`); the
/// math mirrors what the device's own queries produce, but the
/// `page_stat` view itself is never queried.
pub fn load_snapshot(path: &Path, sidecar_dir: Option<&Path>) -> Result<LibrarySnapshot> {
    let db = StatsDb::open(path)?;
    let schema_version = db.schema_version()?;
    let mut entries = Vec::new();
    for book in db.books()? {
        let events = db.events(&book)?;
        // The import dialog accepts any *.db, so events can carry corrupt
        // timestamps. Every day/hour bucket maps them through chrono, which
        // yields no instant for values past the calendar's edge and used to
        // panic the render path (`metrics::days`/`speed`). Drop them once,
        // here, at the single funnel every widget's events flow through.
        let events: Vec<_> = events
            .into_iter()
            .filter(|e| readable_start_time(e.start_time))
            .collect();
        let coverage = metrics::coverage(&events);

        // Capped totals and the activity strip use the same math as the
        // rescaled `page_stat` view, computed by `page_totals` as a
        // canonical-axis rescale reduced per page in SQL (D1: the view's
        // per-row axis conflates merged books, so the view itself is
        // never queried; and materializing it would fan each row out up
        // to ~1000x, RESEARCH §1).
        let page_totals = db.page_totals(&book)?;
        let (capped_secs, view_pages) = metrics::capped_seconds(
            page_totals.iter().map(|p| (p.page, p.secs)),
            colophon_core::model::KOREADER_DEFAULT_MAX_SEC,
        );
        // Last read page on the canonical axis: the latest raw event
        // rescaled onto the book's current page count, straight from the
        // raw rows rather than a second fan-out query just for this one
        // number. Unknown page count: last_page stays None and
        // page-derived stats hide downstream (spec.md "Unknown page
        // count").
        let last_page = book.pages.and_then(|cp| {
            events
                .last()
                .map(|e| metrics::rescaled_last_page(e.page, e.total_pages, cp))
        });

        entries.push(LibraryEntry {
            unique_pages: book.pages.map(|p| metrics::unique_pages_read(coverage, p)),
            events,
            page_totals,
            capped_secs,
            view_pages,
            last_page,
            book,
            declared_status: None,
            annotations: Vec::new(),
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
                    entry.annotations = meta.annotations;
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
    let promoted = stage_and_promote(source, staging_dir, canonical);
    // The staging dir is ours and holds a whole copy of the database, so
    // clear it however the promotion went. It used to be cleaned only after
    // a successful rename, which stranded a full snapshot in the app data
    // dir on any failure (an interrupted copy, a bad pick, the device
    // unplugged mid-read) until the next import happened to overwrite it.
    let _ = std::fs::remove_dir_all(staging_dir);
    promoted?;

    load_snapshot(canonical, sidecar_dir)
}

/// Snapshot → validate → rename over `canonical`. Split out of [`import`]
/// so every failure path funnels through one staging-dir cleanup.
fn stage_and_promote(source: &Path, staging_dir: &Path, canonical: &Path) -> Result<()> {
    let staged = colophon_core::snapshot(source, staging_dir)
        .context("copying the database (is the device still mounted?)")?;

    // Validate before promoting by fully loading the staged copy: the same
    // load the app would run against it after promotion. A database that
    // opens but cannot be loaded (a missing `numbers` table for the page
    // aggregates, say) is refused while the good snapshot is still
    // untouched. Validation used to stop at the book table, so such a file
    // was renamed over the good snapshot and only then failed.
    load_snapshot(&staged, None)?;

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
    Ok(())
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
    fn readable_start_time_bounds_the_calendar() {
        // The extremes a corrupt/foreign db can carry: chrono maps these to
        // no instant, which used to panic the day/hour bucketing at render.
        assert!(!readable_start_time(i64::MIN));
        assert!(!readable_start_time(-1));
        assert!(!readable_start_time(i64::MAX));
        assert!(!readable_start_time(253_402_300_800)); // year 10000
        // Epoch 0 reads as "never" (fmt::relative_date), not a 1970 day.
        assert!(!readable_start_time(0));
        assert!(readable_start_time(1));
        assert!(readable_start_time(1_788_000_000)); // 2026, comfortably
        assert!(readable_start_time(253_402_300_799)); // 9999-12-31 23:59:59
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
            match (entry.unique_pages, entry.book.pages) {
                (Some(u), Some(p)) => assert!(u <= p.max(1)),
                (None, None) => {}
                got => panic!("unique/pages mismatch: {got:?}"),
            }
        }
        // The source was only ever fs-copied, never opened or written.
        assert_eq!(
            std::fs::metadata(&source).unwrap().modified().unwrap(),
            source_mtime
        );

        let reloaded = load_snapshot(&canonical, None).unwrap();
        assert_eq!(reloaded.entries.len(), snap.entries.len());

        // The panic guard: every event that reaches the render path is
        // inside the calendar window, whatever the db carried.
        assert!(
            snap.entries
                .iter()
                .flat_map(|e| e.events.iter())
                .all(|ev| readable_start_time(ev.start_time))
        );

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
        let matched = snap
            .entries
            .iter()
            .find(|e| {
                e.book
                    .md5
                    .as_deref()
                    .is_some_and(|m| m.eq_ignore_ascii_case(&md5))
            })
            .expect("a book matches the sidecar md5");
        assert_eq!(
            matched.declared_status,
            Some(colophon_core::sidecar::ReadStatus::Complete)
        );
        assert!(matched.is_finished());
        // Royal Assassin's sidecar carries its highlight; markers flow too.
        assert!(!matched.annotations.is_empty());

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
        // The rejected copy must not be left sitting in the app data dir.
        assert!(
            !staging.exists(),
            "staging dir cleaned after a failed import"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn import_refuses_a_db_that_opens_but_does_not_load() {
        // The exact class the full-load validation exists for: book and
        // page_stat_data exist (so the old books()-only validation passed)
        // but the `numbers` tally table is missing, which only the page
        // aggregates need. The import must refuse while the good snapshot
        // is still untouched, not promote and then fail.
        let root = temp_dir("unloadable-import");
        let staging = root.join("staging");
        let canonical = root.join("statistics.sqlite3");
        std::fs::write(&canonical, b"pretend this is a good snapshot").unwrap();

        let partial = root.join("partial.sqlite3");
        let conn = rusqlite::Connection::open(&partial).unwrap();
        conn.execute_batch(
            "CREATE TABLE book (
                 id integer PRIMARY KEY autoincrement,
                 title text, authors text, notes integer, last_open integer,
                 highlights integer, pages integer, series text, language text,
                 md5 text, total_read_time integer, total_read_pages integer
             );
             CREATE TABLE page_stat_data (
                 id_book integer,
                 page integer NOT NULL DEFAULT 0,
                 start_time integer NOT NULL DEFAULT 0,
                 duration integer NOT NULL DEFAULT 0,
                 total_pages integer NOT NULL DEFAULT 0
             );
             INSERT INTO book (title, authors, pages, md5)
                 VALUES ('B', 'A', 100, 'dddd');
             INSERT INTO page_stat_data VALUES (1, 5, 1000, 60, 100);",
        )
        .unwrap();
        drop(conn);

        assert!(import(&partial, &staging, &canonical, None).is_err());
        assert_eq!(
            std::fs::read(&canonical).unwrap(),
            b"pretend this is a good snapshot",
            "the good snapshot must survive a refused import"
        );
        assert!(
            !staging.exists(),
            "staging dir cleaned after a refused import"
        );

        std::fs::remove_dir_all(&root).ok();
    }
}

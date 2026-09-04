//! Integration tests for the `db` layer against a synthetic database
//! built with the real KOReader schema (see `common/mod.rs`).

mod common;

use colophon_core::{StatsDb, metrics, snapshot};
use common::FixtureBook;
use rusqlite::Connection;

#[test]
fn open_refuses_missing_file() {
    let dir = common::TempDir::new();
    let missing = dir.path().join("nope.sqlite3");
    assert!(StatsDb::open(&missing).is_err());
    // Read-only open must not have created the file.
    assert!(!missing.exists());
}

#[test]
fn open_refuses_non_koreader_db() {
    let dir = common::TempDir::new();
    let path = dir.path().join("other.sqlite3");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE unrelated (x);").unwrap();
    drop(conn);
    let err = StatsDb::open(&path).unwrap_err();
    assert!(
        err.to_string()
            .contains("not a KOReader statistics database")
    );
}

#[test]
fn schema_version_round_trips() {
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    let db = StatsDb::open(&path).unwrap();
    assert_eq!(
        db.schema_version().unwrap(),
        colophon_core::EXPECTED_SCHEMA_VERSION
    );
}

#[test]
fn books_reads_rows_and_defaults_nulls() {
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    common::insert_book(
        &path,
        &FixtureBook {
            title: "Novel One",
            authors: "Author One",
            pages: Some(866),
            md5: Some("aaaa0000aaaa0000aaaa0000aaaa0000"),
            last_open: 1_000,
            total_read_time: 36_047,
            total_read_pages: 550,
        },
    );
    // NULL-heavy row, like book id 5 in the real sample (total_read_time
    // NULL, md5 NULL).
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO book (title, authors, pages) VALUES ('junk', 'N/A', 1)",
        [],
    )
    .unwrap();
    drop(conn);

    let db = StatsDb::open(&path).unwrap();
    let books = db.books().unwrap();
    assert_eq!(books.len(), 2);
    assert_eq!(books[0].title, "Novel One");
    assert_eq!(books[0].total_read_time, 36_047);
    assert_eq!(books[1].total_read_time, 0);
    assert_eq!(books[1].last_open, 0);
    assert!(books[1].md5.is_none());
    assert!(books[1].is_junk(300));
    assert!(!books[0].is_junk(300));
}

#[test]
fn books_merge_rows_sharing_an_md5() {
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    // A metadata edit in KOReader leaves two rows for one file: same md5,
    // different title spelling.
    let old_id = common::insert_book(
        &path,
        &FixtureBook {
            title: "novel two",
            md5: Some("bbbb1111bbbb1111bbbb1111bbbb1111"),
            last_open: 100,
            total_read_time: 500,
            total_read_pages: 10,
            ..Default::default()
        },
    );
    let new_id = common::insert_book(
        &path,
        &FixtureBook {
            title: "Novel Two",
            md5: Some("BBBB1111BBBB1111BBBB1111BBBB1111"), // case-insensitive
            last_open: 200,
            total_read_time: 132,
            total_read_pages: 5,
            ..Default::default()
        },
    );
    common::insert_event(&path, old_id, 1, 50, 60, 100);
    common::insert_event(&path, new_id, 2, 500, 60, 100);

    let db = StatsDb::open(&path).unwrap();
    let books = db.books().unwrap();
    assert_eq!(books.len(), 1);
    let book = &books[0];
    // Canonical row is the most recently opened one.
    assert_eq!(book.id, new_id);
    assert_eq!(book.title, "Novel Two");
    assert_eq!(book.total_read_time, 632);
    assert_eq!(book.total_read_pages, 15);
    assert_eq!(book.all_ids.len(), 2);

    // Events come from both merged rows, in time order.
    let events = db.events(book).unwrap();
    assert_eq!(events.len(), 2);
    assert!(events[0].start_time < events[1].start_time);
}

#[test]
fn different_md5s_stay_separate_books() {
    // The real sample has one title twice with different md5s: two files
    // of the same work. Those must NOT merge.
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    common::insert_book(
        &path,
        &FixtureBook {
            title: "Novel Two",
            md5: Some("cccc2222cccc2222cccc2222cccc2222"),
            pages: Some(567),
            ..Default::default()
        },
    );
    common::insert_book(
        &path,
        &FixtureBook {
            title: "Novel Two",
            md5: Some("bbbb1111bbbb1111bbbb1111bbbb1111"),
            pages: Some(644),
            ..Default::default()
        },
    );
    let db = StatsDb::open(&path).unwrap();
    assert_eq!(db.books().unwrap().len(), 2);
}

#[test]
fn rescaled_view_matches_koreader_semantics() {
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    // Book currently has 200 pages; an old event was recorded when the
    // layout had 100. Page 10 of 100 must rescale to pages 19-20 of 200,
    // with the 60 s split 30/30 (KOReader's own view does this).
    let id = common::insert_book(
        &path,
        &FixtureBook {
            pages: Some(200),
            ..Default::default()
        },
    );
    common::insert_event(&path, id, 10, 1_000, 60, 100);

    let db = StatsDb::open(&path).unwrap();
    let books = db.books().unwrap();
    let rescaled = db.rescaled_events(&books[0]).unwrap();
    assert_eq!(rescaled.len(), 2);
    assert_eq!(rescaled[0].page, 19);
    assert_eq!(rescaled[1].page, 20);
    assert_eq!(rescaled[0].duration + rescaled[1].duration, 60);

    // The raw table is untouched by rescaling.
    let raw = db.events(&books[0]).unwrap();
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0].page, 10);
    assert_eq!(raw[0].total_pages, 100);
}

#[test]
fn page_totals_and_last_page_agree_with_the_materialized_view() {
    // The aggregated path (page_totals + rescaled_last_page) must yield
    // exactly what pulling the whole fanned-out view yielded before.
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    // 200-page book; three events, one recorded at the old 100-page layout
    // (so it fans out 1->2 pages) and page 40 read twice (a re-read).
    let id = common::insert_book(
        &path,
        &FixtureBook {
            pages: Some(200),
            ..Default::default()
        },
    );
    common::insert_event(&path, id, 10, 1_000, 60, 100); // -> pages 19,20 (30s each)
    common::insert_event(&path, id, 40, 2_000, 50, 200); // -> page 40, 50s
    common::insert_event(&path, id, 40, 3_000, 20, 200); // -> page 40 again, +20s

    let db = StatsDb::open(&path).unwrap();
    let book = &db.books().unwrap()[0];

    // Reference: derive the same numbers straight from the view rows.
    let rescaled = db.rescaled_events(book).unwrap();
    let (ref_capped, ref_pages) = metrics::capped_seconds(
        rescaled.iter().map(|e| (e.page, e.duration)),
        colophon_core::model::KOREADER_DEFAULT_MAX_SEC,
    );
    let ref_last = rescaled
        .iter()
        .max_by_key(|e| e.start_time)
        .map(|e| e.page)
        .unwrap_or(0);

    // New path: page_totals GROUP BY + rescaled_last_page from raw events.
    let totals = db.page_totals(book).unwrap();
    let (capped, pages) = metrics::capped_seconds(
        totals.iter().map(|p| (p.page, p.secs)),
        colophon_core::model::KOREADER_DEFAULT_MAX_SEC,
    );
    let raw = db.events(book).unwrap();
    let last = raw
        .last()
        .map(|e| metrics::rescaled_last_page(e.page, e.total_pages, book.pages.expect("pages")))
        .unwrap_or(0);

    assert_eq!((capped, pages), (ref_capped, ref_pages));
    assert_eq!(last, ref_last);
    // Page 40's re-read is one page with the two durations summed.
    let p40 = totals.iter().find(|p| p.page == 40).unwrap();
    assert_eq!((p40.secs, p40.reads), (70, 2));
}

#[test]
fn all_events_returns_every_row_in_time_order() {
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    let a = common::insert_book(&path, &FixtureBook::default());
    let b = common::insert_book(
        &path,
        &FixtureBook {
            title: "Other",
            md5: Some("bbbb0000bbbb0000bbbb0000bbbb0000"),
            ..Default::default()
        },
    );
    common::insert_event(&path, b, 1, 300, 30, 100);
    common::insert_event(&path, a, 1, 100, 30, 100);

    let db = StatsDb::open(&path).unwrap();
    let events = db.all_events().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].start_time, 100);
    assert_eq!(events[1].start_time, 300);
}

#[test]
fn snapshot_copies_and_folds_wal() {
    let source_dir = common::TempDir::new();
    let dest_dir = common::TempDir::new();
    let source = common::create_db(source_dir.path());

    // Put the source in WAL mode with unflushed content, like a live
    // device database.
    let conn = Connection::open(&source).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.execute(
        "INSERT INTO book (title, authors, pages, md5) VALUES ('In WAL', 'A', 10, 'cccc')",
        [],
    )
    .unwrap();
    // Keep the connection open so the WAL is not checkpointed by close.
    let copied = snapshot(&source, dest_dir.path().join("sub")).unwrap();
    drop(conn);

    let db = StatsDb::open(&copied).unwrap();
    let books = db.books().unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "In WAL");

    // And the copy is self-contained: no WAL sidecar left behind.
    assert!(!copied.with_extension("sqlite3-wal").exists());
}

// --- Audit 2026-09-04: D1/D2/D3 regression fixtures --------------------------

#[test]
fn d1_two_paginations_do_not_merge_axes() {
    // One md5, two book rows whose page counts differ (300 and 350). The old
    // view-based GROUP BY summed positions from both pagination axes into
    // one bucket; the canonical-axis rescale keeps every event on the
    // canonical (350-page) axis.
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    let old_id = common::insert_book(
        &path,
        &common::FixtureBook {
            title: "Old Title", // a metadata edit creates the second row
            md5: Some("same"),
            pages: Some(300),
            last_open: 1_000,
            ..Default::default()
        },
    );
    let new_id = common::insert_book(
        &path,
        &common::FixtureBook {
            title: "New Title",
            md5: Some("same"),
            pages: Some(350),
            last_open: 2_000, // canonical: most recently opened
            ..Default::default()
        },
    );
    let conn = Connection::open(&path).unwrap();
    // The old row's page 30 was page 30 of 300 = 10% in; on the 350-page
    // axis that is page 35. The new row's page 70 is page 70 of 350.
    conn.execute(
        "INSERT INTO page_stat_data VALUES (?1, 30, 1000, 60, 300)",
        rusqlite::params![old_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO page_stat_data VALUES (?1, 70, 2000, 60, 350)",
        rusqlite::params![new_id],
    )
    .unwrap();
    drop(conn);

    let db = StatsDb::open(&path).unwrap();
    let books = db.books().unwrap();
    assert_eq!(books.len(), 1);
    let book = &books[0];
    assert_eq!(book.all_ids.len(), 2);

    let totals = db.page_totals(book).unwrap();
    let pages: Vec<i64> = totals.iter().map(|p| p.page).collect();
    // The old row's page 30 was 10% into a 300-page axis; rescaled onto the
    // canonical 350-page axis it fans across pages 34-35 (the view's own
    // fan-out arithmetic). No phantom page 30 from the other axis.
    assert_eq!(
        pages,
        vec![34, 35, 70],
        "axes merged or miscounted: {pages:?}"
    );

    // rescaled_events maps the same way, ordered by time.
    let events = db.rescaled_events(book).unwrap();
    let event_pages: Vec<i64> = events.iter().map(|e| e.page).collect();
    assert_eq!(event_pages, vec![34, 35, 70]);
}

#[test]
fn d2_unknown_pages_hides_page_derived_stats() {
    // A NULL-pages book (KOReader's book.pages is nullable): the page-
    // derived aggregates come back empty rather than confident zeros, and
    // the raw events survive for the time-derived stats.
    let dir = common::TempDir::new();
    let path = common::create_db(dir.path());
    let id = common::insert_book(
        &path,
        &common::FixtureBook {
            pages: None,
            total_read_time: 3_600,
            ..Default::default()
        },
    );
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO page_stat_data VALUES (?1, 5, 1000, 60, 200)",
        rusqlite::params![id],
    )
    .unwrap();
    drop(conn);

    let db = StatsDb::open(&path).unwrap();
    let books = db.books().unwrap();
    assert_eq!(books.len(), 1);
    assert!(books[0].pages.is_none());

    // Page-derived: hidden.
    assert!(db.page_totals(&books[0]).unwrap().is_empty());
    assert!(db.rescaled_events(&books[0]).unwrap().is_empty());

    // Time-derived: intact.
    assert_eq!(db.events(&books[0]).unwrap().len(), 1);
}

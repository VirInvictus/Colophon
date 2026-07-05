//! Test fixture builder: creates a synthetic KOReader `statistics.sqlite3`
//! in a temp directory using the *verbatim* schema from the on-device
//! plugin source (`research/koreader-plugin-src/statistics.koplugin/
//! main.lua:410-499`), including the `numbers` tally table and the
//! `page_stat` rescaling view. `*.sqlite3` is gitignored repo-wide, so
//! fixtures are built programmatically instead of committed as binaries.
//!
//! Shared across test binaries; not every helper is used by every binary,
//! so dead-code warnings here are expected noise.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::Connection;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A per-test temp directory under the system temp dir, removed on drop.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "colophon-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&path).expect("creating test temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// Verbatim from KOReader's statistics.koplugin main.lua (createDB).
const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS book
        (
            id integer PRIMARY KEY autoincrement,
            title text,
            authors text,
            notes      integer,
            last_open  integer,
            highlights integer,
            pages      integer,
            series text,
            language text,
            md5 text,
            total_read_time  integer,
            total_read_pages integer
        );
    CREATE UNIQUE INDEX IF NOT EXISTS book_title_authors_md5 ON book(title, authors, md5);
    CREATE TABLE IF NOT EXISTS page_stat_data
        (
            id_book     integer,
            page        integer NOT NULL DEFAULT 0,
            start_time  integer NOT NULL DEFAULT 0,
            duration    integer NOT NULL DEFAULT 0,
            total_pages integer NOT NULL DEFAULT 0,
            UNIQUE (id_book, page, start_time),
            FOREIGN KEY(id_book) REFERENCES book(id)
        );
    CREATE INDEX IF NOT EXISTS page_stat_data_start_time ON page_stat_data(start_time);
    CREATE TABLE IF NOT EXISTS numbers
        (
            number INTEGER PRIMARY KEY
        );
    WITH RECURSIVE counter AS
        (
            SELECT 1 as N UNION ALL
            SELECT N + 1 FROM counter WHERE N < 1000
        )
        INSERT INTO numbers SELECT N AS number FROM counter;
    CREATE VIEW IF NOT EXISTS page_stat AS
        SELECT id_book, first_page + idx - 1 AS page, start_time, duration / (last_page - first_page + 1) AS duration
        FROM (
            SELECT id_book, page, total_pages, pages, start_time, duration,
                ((page - 1) * pages) / total_pages + 1 AS first_page,
                max(((page - 1) * pages) / total_pages + 1, (page * pages) / total_pages) AS last_page,
                idx
            FROM page_stat_data
            JOIN book ON book.id = id_book
            JOIN (SELECT number as idx FROM numbers) AS N ON idx <= (last_page - first_page + 1)
        );
    PRAGMA user_version = 20221111;
";

pub struct FixtureBook<'a> {
    pub title: &'a str,
    pub authors: &'a str,
    pub pages: i64,
    pub md5: Option<&'a str>,
    pub last_open: i64,
    pub total_read_time: i64,
    pub total_read_pages: i64,
}

impl Default for FixtureBook<'_> {
    fn default() -> Self {
        Self {
            title: "A Book",
            authors: "An Author",
            pages: 100,
            md5: Some("00000000000000000000000000000000"),
            last_open: 0,
            total_read_time: 0,
            total_read_pages: 0,
        }
    }
}

/// Creates `statistics.sqlite3` under `dir` with the real schema and no
/// rows, returning its path.
pub fn create_db(dir: &Path) -> PathBuf {
    let path = dir.join("statistics.sqlite3");
    let conn = Connection::open(&path).expect("creating fixture db");
    conn.execute_batch(SCHEMA).expect("applying fixture schema");
    path
}

/// Inserts a book row and returns its id.
pub fn insert_book(path: &Path, book: &FixtureBook) -> i64 {
    let conn = Connection::open(path).expect("opening fixture db");
    conn.execute(
        "INSERT INTO book (title, authors, notes, last_open, highlights, pages,
                           series, language, md5, total_read_time, total_read_pages)
         VALUES (?1, ?2, 0, ?3, 0, ?4, NULL, NULL, ?5, ?6, ?7)",
        rusqlite::params![
            book.title,
            book.authors,
            book.last_open,
            book.pages,
            book.md5,
            book.total_read_time,
            book.total_read_pages,
        ],
    )
    .expect("inserting fixture book");
    conn.last_insert_rowid()
}

/// A tiny deterministic PRNG (xorshift64*). std ships no rng and the crate
/// takes no dependencies; synthetic fixtures must be reproducible so
/// measurements are comparable run to run, so a fixed-seed generator it is.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[lo, hi)`; `hi` must exceed `lo`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next_u64() % (hi - lo) as u64) as i64
    }

    /// True with probability `num`/`den`.
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.next_u64() % den < num
    }
}

/// The realized shape of a generated large fixture, for the measurement
/// harness to report against.
pub struct LargeFixture {
    pub path: PathBuf,
    pub books: usize,
    /// Rows actually written to `page_stat_data` (the raw, un-fanned events).
    pub raw_events: i64,
}

/// Generates a realistic multi-year `statistics.sqlite3` under `dir`:
/// `num_books` books read across `days` calendar days starting at
/// `start_epoch`, with page-turn events at KOReader-plausible durations.
///
/// Some books record events at a `total_pages` below their current page
/// count (simulating font-size changes), which makes the `page_stat` view
/// fan each stored row out across several rescaled pages, the exact
/// blow-up the performance work targets. Deterministic given `seed`.
pub fn generate_large(
    dir: &Path,
    num_books: usize,
    start_epoch: i64,
    days: i64,
    seed: u64,
) -> LargeFixture {
    let path = dir.join("statistics.sqlite3");
    let mut conn = Connection::open(&path).expect("creating large fixture db");
    conn.execute_batch(SCHEMA).expect("applying fixture schema");
    // Bulk load: one connection, one transaction, prepared statements.
    conn.execute_batch("PRAGMA synchronous = OFF; PRAGMA journal_mode = MEMORY;")
        .expect("relaxing durability for the bulk load");

    let mut rng = Rng::new(seed);
    let mut raw_events: i64 = 0;
    let tx = conn.transaction().expect("begin bulk transaction");
    {
        let mut insert_book = tx
            .prepare(
                "INSERT INTO book (title, authors, notes, last_open, highlights, pages,
                                   series, language, md5, total_read_time, total_read_pages)
                 VALUES (?1, ?2, 0, ?3, 0, ?4, NULL, NULL, ?5, ?6, ?7)",
            )
            .expect("prepare book insert");
        let mut insert_event = tx
            .prepare(
                "INSERT INTO page_stat_data (id_book, page, start_time, duration, total_pages)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .expect("prepare event insert");

        for b in 0..num_books {
            let pages = rng.range(80, 1200);
            // Some books record at a lower page count than they now render
            // at, so the page_stat view fans each row out by `factor`.
            let factor = [1, 1, 1, 2, 3][rng.range(0, 5) as usize];
            let recorded_total = (pages / factor).max(1);

            let title = format!("Synthetic Book {b:04}");
            let md5 = format!("{b:032x}");
            let start_day = rng.range(0, days.max(1));
            let span = rng.range(3, 90).min(days - start_day + 1).max(1);

            // Buffer this book's events so the book row (which foreign
            // keys enforce must exist first) can be inserted with the totals
            // derived from them.
            let mut events: Vec<(i64, i64, i64)> = Vec::new(); // (page, start_time, duration)
            let mut cursor = 1i64;
            let mut total_time = 0i64;
            let mut last_open = start_epoch + start_day * 86_400;

            for day in 0..span {
                // Read on ~65% of the days inside the active span.
                if !rng.chance(65, 100) {
                    continue;
                }
                let day_start = start_epoch + (start_day + day) * 86_400;
                // Session starts somewhere in the evening-ish window.
                let mut clock = day_start + rng.range(6, 23) * 3600;
                let session_pages = rng.range(5, 70);
                for _ in 0..session_pages {
                    let duration = rng.range(8, 120);
                    events.push((cursor, clock, duration));
                    total_time += duration;
                    clock += duration;
                    last_open = clock;
                    cursor += 1;
                    if cursor > recorded_total {
                        cursor = 1;
                    }
                }
            }

            insert_book
                .execute(rusqlite::params![
                    title,
                    "Synthetic Author",
                    last_open,
                    pages,
                    md5,
                    total_time,
                    events.len() as i64,
                ])
                .expect("insert synthetic book");
            let book_id = tx.last_insert_rowid();
            for (page, start_time, duration) in events {
                insert_event
                    .execute(rusqlite::params![
                        book_id,
                        page,
                        start_time,
                        duration,
                        recorded_total,
                    ])
                    .expect("insert synthetic event");
                raw_events += 1;
            }
        }
    }
    tx.commit().expect("commit bulk transaction");

    LargeFixture {
        path,
        books: num_books,
        raw_events,
    }
}

/// Inserts one page-turn event.
pub fn insert_event(
    path: &Path,
    book_id: i64,
    page: i64,
    start_time: i64,
    duration: i64,
    total_pages: i64,
) {
    let conn = Connection::open(path).expect("opening fixture db");
    conn.execute(
        "INSERT INTO page_stat_data (id_book, page, start_time, duration, total_pages)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![book_id, page, start_time, duration, total_pages],
    )
    .expect("inserting fixture event");
}

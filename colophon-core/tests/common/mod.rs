//! Test fixture builder: creates a synthetic KOReader `statistics.sqlite3`
//! in a temp directory using the *verbatim* schema from the on-device
//! plugin source (`research/koreader-plugin-src/statistics.koplugin/
//! main.lua:410-499`), including the `numbers` tally table and the
//! `page_stat` rescaling view. `*.sqlite3` is gitignored repo-wide, so
//! fixtures are built programmatically instead of committed as binaries.

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

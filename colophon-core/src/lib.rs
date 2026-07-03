//! Read-only access to a KOReader `statistics.sqlite3` database.
//!
//! Colophon never opens KOReader's live database in place and never writes
//! to it. Callers are expected to copy the file (e.g. from an SSHFS-mounted
//! device) to a local path first; this crate only ever attaches it
//! `?mode=ro`.
//!
//! The real schema (table/column names, page-count-vs-font-size behaviour,
//! how re-reads and multi-device sync land in the data) has not been
//! verified against an upstream source yet. Do not assume the shape below
//! is correct — confirm it against KOReader's
//! `plugins/statistics.koplugin/main.lua` and a real database before
//! building anything on top of it. See `../CLAUDE.md` and `../roadmap.md`
//! Phase 0.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

pub struct StatsDb {
    conn: Connection,
}

impl StatsDb {
    /// Opens a local copy of a KOReader `statistics.sqlite3` file, read-only.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let uri = format!("file:{}?mode=ro", path.as_ref().display());
        let conn = Connection::open_with_flags(
            uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )
        .context("opening koreader statistics db read-only")?;
        Ok(Self { conn })
    }

    /// Lists the tables present in the attached database. Placeholder used
    /// during Phase 0 schema discovery; not a stable API.
    pub fn table_names(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(names)
    }
}

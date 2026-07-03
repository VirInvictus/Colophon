//! Read-only ingestion and metrics for KOReader's `statistics.sqlite3`.
//!
//! Colophon never opens KOReader's live database in place and never
//! writes to a path KOReader owns. Callers copy the file first (see
//! [`db::snapshot`]); this crate opens copies with
//! `SQLITE_OPEN_READ_ONLY` only.
//!
//! Layering:
//! - [`db`] — typed queries over the confirmed schema (RESEARCH.md §1):
//!   md5-merged books, raw `page_stat_data` events, the rescaled
//!   `page_stat` view.
//! - [`metrics`] — pure derived-metric functions implementing `spec.md`'s
//!   normative definitions (sessions, streaks, coverage, capped totals,
//!   speed, completion detection).
//! - [`model`] — the plain types both share.

pub mod db;
pub mod metrics;
pub mod model;

pub use db::{EXPECTED_SCHEMA_VERSION, StatsDb, snapshot};
pub use model::{
    Book, Completion, DayTotal, PageEvent, RescaledEvent, Session, SpeedPoint, Streak, Streaks,
};

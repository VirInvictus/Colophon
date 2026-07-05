//! Performance baseline for a realistic multi-year library (roadmap
//! Phase 4). Ignored by default; run explicitly:
//!
//! ```text
//! cargo test -p colophon-core --test perf -- --ignored --nocapture
//! ```
//!
//! It mirrors what the app actually does today: the load phase copies
//! `loader::load_snapshot` (every book's raw events *and* fanned-out
//! `page_stat` view rows held in memory), and the render phase copies the
//! `stats::overview` hot path (flatten all events, then daily/speed/hourly/
//! session aggregation, once for all-time and once for a 90-day window).
//! The numbers it prints are the baseline the optimization work is judged
//! against; it asserts only that the fixture is genuinely large so a
//! regression to a trivial dataset can't pass silently.

mod common;

use std::hint::black_box;
use std::time::Instant;

use chrono::Utc;
use colophon_core::StatsDb;
use colophon_core::metrics::{self, Bucket};
use colophon_core::model::{
    DEFAULT_SESSION_GAP_SECS, KOREADER_DEFAULT_MAX_SEC, PageEvent, PageTotal, RescaledEvent,
};

/// A value from `/proc/self/status` in kB, or 0 if unavailable (non-Linux).
fn status_kb(field: &str) -> i64 {
    let Ok(text) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(field).filter(|_| line.starts_with(field)) {
            return rest
                .trim_start_matches(':')
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
#[ignore = "performance measurement; run with --ignored --nocapture"]
fn measure_multi_year_load_and_render() {
    // 200 books over four years (~1460 days) starting 2020-09-13.
    let dir = common::TempDir::new();
    let t = Instant::now();
    let fx = common::generate_large(dir.path(), 200, 1_600_000_000, 1460, 0xC0110F0D);
    let gen_ms = ms(t);

    let db = StatsDb::open(&fx.path).expect("open synthetic db");
    let books = db.books().expect("load books");

    // ---- Load phase (current): mirror loader::load_snapshot, which pulls
    // raw events plus the per-page GROUP BY reduction of the page_stat view
    // (Stage 2), never the fanned-out view rows. ----
    let t = Instant::now();
    let mut per_book_events: Vec<Vec<PageEvent>> = Vec::with_capacity(books.len());
    let mut held_totals: Vec<Vec<PageTotal>> = Vec::with_capacity(books.len());
    let mut total_rows: i64 = 0;
    for book in &books {
        let events = db.events(book).expect("events");
        black_box(metrics::coverage(&events));

        let page_totals = db.page_totals(book).expect("page totals");
        black_box(metrics::capped_seconds(
            page_totals.iter().map(|p| (p.page, p.secs)),
            KOREADER_DEFAULT_MAX_SEC,
        ));
        black_box(
            events
                .last()
                .map(|e| metrics::rescaled_last_page(e.page, e.total_pages, book.pages))
                .unwrap_or(0),
        );
        total_rows += page_totals.len() as i64;

        per_book_events.push(events);
        held_totals.push(page_totals);
    }
    let load_ms = ms(t);
    let rss_after_load = status_kb("VmRSS");

    // ---- Baseline comparison: the pre-Stage-2 load, which also
    // materialized the whole fanned-out page_stat view per book. Same work
    // as above but pulling view rows instead of the GROUP BY reduction, so
    // the two load numbers are directly comparable. ----
    let t = Instant::now();
    let mut rescaled_rows: i64 = 0;
    for book in &books {
        let events = db.events(book).expect("events");
        black_box(metrics::coverage(&events));
        let rescaled: Vec<RescaledEvent> = db.rescaled_events(book).expect("rescaled events");
        black_box(metrics::capped_seconds(
            rescaled.iter().map(|e| (e.page, e.duration)),
            KOREADER_DEFAULT_MAX_SEC,
        ));
        black_box(rescaled.iter().max_by_key(|e| e.start_time).map(|e| e.page));
        rescaled_rows += rescaled.len() as i64;
        black_box((events, rescaled));
    }
    let old_load_ms = ms(t);

    // ---- Render phase: mirror stats::overview_base + overview_windowed. ----
    let today = per_book_events
        .iter()
        .flatten()
        .map(|e| e.start_time)
        .max()
        .map(|ts| metrics::local_date(ts, &Utc))
        .expect("some events");

    // The window-independent base: flatten + daily_totals (+ cheap streaks/
    // monthly). Built once per filtered set and cached across toggles.
    let t = Instant::now();
    let all: Vec<PageEvent> = per_book_events.iter().flatten().copied().collect();
    let flatten_ms = ms(t);
    let t = Instant::now();
    let daily = metrics::daily_totals(&all, &Utc);
    let daily_ms = ms(t);
    let base_ms = flatten_ms + daily_ms;
    let day_count = daily.len();

    // A cached window toggle recomputes only the windowed behaviour charts
    // against the cached base; daily_totals/streaks/monthly are reused.
    let toggle = |window_days: Option<i64>| {
        let cutoff = window_days.map(|n| today - chrono::Duration::days(n - 1));
        let windowed: Vec<PageEvent> = match cutoff {
            Some(c) => all
                .iter()
                .copied()
                .filter(|e| metrics::local_date(e.start_time, &Utc) >= c)
                .collect(),
            None => all.clone(),
        };
        black_box(metrics::speed_series(&windowed, &Utc, Bucket::Week));
        black_box(metrics::hourly_profile(&windowed, &Utc));
        black_box(metrics::sessions(&windowed, DEFAULT_SESSION_GAP_SECS));
    };

    let t = Instant::now();
    toggle(None);
    let toggle_all_ms = ms(t);
    let render_all_ms = base_ms + toggle_all_ms;

    let t = Instant::now();
    toggle(Some(90));
    let toggle_90_ms = ms(t);

    // Per-step breakdown (all-time) for context.
    let t = Instant::now();
    black_box(metrics::speed_series(&all, &Utc, Bucket::Week));
    let speed_ms = ms(t);
    let t = Instant::now();
    black_box(metrics::hourly_profile(&all, &Utc));
    let hourly_ms = ms(t);
    let t = Instant::now();
    black_box(metrics::sessions(&all, DEFAULT_SESSION_GAP_SECS));
    let sessions_ms = ms(t);

    let rss_peak = status_kb("VmHWM");
    let fanout = rescaled_rows as f64 / fx.raw_events.max(1) as f64;

    eprintln!("\n=== Colophon perf baseline (synthetic multi-year) ===");
    eprintln!("books                {}", fx.books);
    eprintln!("raw page_stat_data   {}", fx.raw_events);
    eprintln!("page_totals rows     {total_rows}  (one per page, held in memory)");
    eprintln!("page_stat view rows  {rescaled_rows}  ({fanout:.2}x fan-out, no longer held)");
    eprintln!("distinct days        {day_count}");
    eprintln!("---");
    eprintln!("fixture generate     {gen_ms:8.1} ms");
    eprintln!("load (aggregated)    {load_ms:8.1} ms");
    eprintln!("  vs old full load   {old_load_ms:8.1} ms");
    eprintln!(
        "overview first render {render_all_ms:8.1} ms  (base {base_ms:.1} + toggle {toggle_all_ms:.1})"
    );
    eprintln!("cached toggle -> all  {toggle_all_ms:8.1} ms  (base reused)");
    eprintln!("cached toggle -> 90d  {toggle_90_ms:8.1} ms  (base reused)");
    eprintln!("  breakdown (all-time):");
    eprintln!("    flatten          {flatten_ms:8.1} ms  [base]");
    eprintln!("    daily_totals     {daily_ms:8.1} ms  [base]");
    eprintln!("    speed_series     {speed_ms:8.1} ms  [windowed]");
    eprintln!("    hourly_profile   {hourly_ms:8.1} ms  [windowed]");
    eprintln!("    sessions         {sessions_ms:8.1} ms  [windowed]");
    eprintln!("---");
    eprintln!("RSS after load       {} MB", rss_after_load / 1024);
    eprintln!("RSS peak (VmHWM)     {} MB", rss_peak / 1024);
    eprintln!("=====================================================\n");

    // Guard: the fixture must actually be large, or this measures nothing.
    assert!(
        fx.raw_events > 100_000,
        "fixture too small ({} raw events); tune generate_large",
        fx.raw_events
    );
    black_box((per_book_events, held_totals));
}

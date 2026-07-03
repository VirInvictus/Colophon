# Colophon — spec

**Status: locked for v1 (Phase 0 complete, 2026-07-03).** The
`statistics.sqlite3` schema, KOReader's own built-in stats UI, and the four
existing third-party tools have all been read from source; findings and
citations live in `RESEARCH.md`. The widget catalogue below is a
commitment, not a brainstorm.

## Core concept

Colophon is a native GTK4 / libadwaita desktop app that turns KOReader's
reading-statistics database into attractive graphs and a wide variety of
statistic widgets. It is a *viewer*, not a KOReader plugin and not a sync
service — it operates on a local copy of the data.

## Philosophy

- **Read-only, always.** Colophon never opens KOReader's live database file
  in place, and never writes to any file KOReader owns. It works from a
  copy (transferred by the user, e.g. via the SSHFS-mounted `/mnt/Kindle`,
  or a manual file drop).
- **Local-first.** No accounts, no cloud sync, no telemetry, works fully
  offline. On-brand for the rest of the portfolio.
- **Native, not a web dashboard.** The explicit gap this project targets:
  every existing KOReader stats tool Brandon has run into is a web UI or a
  self-hosted Docker service. This is a real GTK4/libadwaita app.
- **Breadth over one fixed report.** KOReader's own in-app statistics screen
  already covers the basics (calendar heatmap, per-book totals). Colophon's
  reason to exist is a *wide variety* of widgets/charts pulling different
  cuts of the same underlying data — the value is in depth and variety, not
  in re-skinning what KOReader already shows.

## Data model (confirmed — see `RESEARCH.md` §1 for full detail)

`statistics.sqlite3` (`koreader/settings/statistics.sqlite3` on device) has
three things that matter: a `book` table (title/authors/series/language/
md5, `pages`, running `total_read_time`/`total_read_pages` totals, plus
`notes`/`highlights` as **counts only**, not content); a raw
`page_stat_data` table (one row per page-turn: book, page, start_time,
duration, and the page count *at that moment*, which is how KOReader copes
with font-size changes shifting pagination); and a `page_stat` view that
rescales historical rows onto the book's current page count for
apples-to-apples charting. Timestamps are unix epoch seconds, no stored
timezone.

## Derived-metric definitions (normative)

These definitions are pinned so Colophon's numbers reconcile with the
device and with each other. Rationale and citations: `RESEARCH.md` §4-§6.

- **Day**: local-timezone calendar day (`date(start_time, 'unixepoch',
  'localtime')` semantics). A configurable "day starts at HH:MM" shift
  (KOReader and KoShelf both offer one) is a later option, off by default.
- **Session**: per-book run of `page_stat_data` rows where each row starts
  no more than 300 s after the previous row's end
  (`start_time > prev.start_time + prev.duration + 300` opens a new one).
  Sessions never span books.
- **Pages read** (in any bucket): distinct (book, page) pairs, never raw
  event counts. Page-turn *events* are a separate, clearly-named metric.
- **Time read**: uncapped `sum(duration)` by default. Where a widget
  mirrors a KOReader screen that uses capped totals (per distinct page,
  clamped to `max_sec` per page), it uses the capped query and says so.
  Durations are already clamped to [min_sec, max_sec] at record time by
  KOReader; Colophon surfaces those settings' values when it can infer
  them but never "un-clamps".
- **Streak**: a day counts if it has any reading. Current streak is alive
  if the last read day is today or yesterday; a gap of two or more days
  zeroes it. Longest streak is the max consecutive-day run.
- **Progress / unique pages read**: interval union on the page axis. Each
  event's page (out of its own recorded `total_pages`) maps to the
  fractional span `[(page-1)/total, page/total]`; merged span length x
  current page count = unique pages read. Immune to re-reads and
  pagination drift.
- **Completion (inferred read-through)**: KoShelf's detection, adopted
  as-is: a progression of events visiting >= 78 % of pages including a
  page in the first 20 % and one in the last 2 %; a jump back to the
  first 5 % starts a new progression only if the remainder would itself
  qualify. Yields start/end dates, time, sessions, pages/hour per
  completion. (The `.sdr` `summary.status` field is the only user-declared
  "finished" flag and is out of scope until sidecars are.)
- **Book identity**: `book.md5`. Rows sharing an md5 (metadata edits) are
  merged at ingest. Same-title/author books with different md5s (two
  files of the same work, confirmed in the sample data) are *grouped for
  display only*, never merged in data.
- **Junk filter**: books below a minimum total read time (default 5
  minutes, configurable) are hidden from library-wide widgets by default;
  they remain queryable.

## Widget catalogue (v1 commitment)

Grounded against what KOReader's own UI shows (`RESEARCH.md` §4), what the
four tools already built (§5), and what nobody has (§8). Three tiers:
build order within a tier is free, tiers are priority order.

### Tier A — the differentiators (why Colophon exists)

Nothing below exists in KOReader or any of the four tools.

1. **Reading-speed trend.** Pages/hour over time (bucketed by
   day/week/month), library baseline with per-book overlay. Uses distinct
   pages / uncapped time per bucket.
2. **When-do-I-read heatmap.** Weekday x hour-of-day grid (7x24), whole
   history or windowed, cell intensity = total time. The aggregate profile
   KOReader only shows one day at a time.
3. **Session analytics.** Session-length histogram, sessions-per-day,
   start-time patterns, records (longest session, most sessions in a day),
   average session by weekday.
4. **Book velocity.** For one book: time spent per page position (via the
   `page_stat` view so the axis is stable), plus pace-per-day through each
   read. Answers "did it drag in the middle".
5. **Per-page activity strip.** KoShelf's per-page grid done natively:
   per-page total duration (sqrt scale, 90th-percentile cap) and read
   count; annotation *count* markers when sidecar data is absent.
6. **Completions timeline.** Inferred read-throughs plotted on a timeline;
   books-finished-per-year/month rollups; per-completion cards (dates,
   calendar span, time, sessions, pages/hour).

### Tier B — expected furniture (table stakes, done well)

7. **Year heatmap calendar.** GitHub-style day grid, quantized intensity
   levels (not continuous alpha), tooltips with time + pages + books.
8. **Streaks.** Current/longest day streak with date ranges.
9. **Library totals.** Total time, unique pages, books touched, active
   days, busiest day/month records. Windowed (30/90/365/all).
10. **Per-book stat cards.** Progress (interval union), total time (both
    capped and uncapped, labelled), days reading, avg time/day, avg
    time/page, est. time left and finish date using KOReader's own math
    (capped avg_time) so the numbers match the device.
11. **Weekday and monthly distribution bars.** Weekday averages normalized
    by weekdays elapsed (not raw sums; KoInsight's mistake), monthly
    totals.

### Tier C — deferred until the data or a dependency justifies it

- Highlight/note content browser (needs `.sdr` ingestion, an `mlua` dep,
  and a real sidecar sample; counts-only until then).
- Vocabulary-builder widgets (Brandon's `vocabulary_builder.sqlite3` is
  empty; revisit if the feature gets used).
- Series/language/author rollups (schema supports it; sample data too
  thin to design against yet).
- Reference-pages normalization across layouts (KoInsight's manual
  canonical page count; only matters with multiple layouts of the same
  book being compared, which the interval-union progress already handles
  for the common case).
- Multi-device merge (no second device exists; KoInsight's
  `(md5, device, page, start_time)` upsert is the reference design).

## Resolved questions (Phase 0)

Everything that was blocking: schema (RESEARCH §1), pagination-drift
handling (§1), KOReader's built-in UI (§4), third-party catalogues (§5),
`.sdr` sidecar location/format/linkage (§7; structure known from source,
one real sample still to be copied when the Kindle is next mounted).

## Stack

Rust 2024, GTK4 / libadwaita, `rusqlite` (read-only opens only). Two-crate
workspace: `colophon-core` (ingestion/querying) and `colophon` (the GTK
shell). Charting approach (native cairo drawing vs. a Rust charting crate)
is an open decision for after Phase 0 — don't lock it in prematurely.

## Non-goals (for now)

- Writing back to KOReader's database or config in any way.
- Cloud sync / multi-device merge logic beyond what's already baked into
  the KOReader data itself.
- Supporting reading-stats formats from other e-reader software (Kobo's
  native firmware stats, Moon+ Reader, etc.) — KOReader only, unless a
  strong case emerges later.

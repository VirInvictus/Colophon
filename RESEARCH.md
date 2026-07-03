# Colophon — research dossier

Phase 0 findings. This is the canonical research record; `spec.md` and
`roadmap.md` should stay in sync with it as understanding deepens, but this
file is where the detail and citations live. Confirmed 2026-07-03 against
Brandon's actual Kindle (`/mnt/Kindle`, SSHFS-mounted), KOReader version
running `statistics.sqlite3` schema `user_version = 20221111`.

## 1. The core statistics database

Path on device: `koreader/settings/statistics.sqlite3` (under the KOReader
install root — on this Kindle that's `/mnt/us/koreader/...`, mounted at
`/mnt/Kindle/koreader/...`). A read-only copy lives at
`research/samples/statistics.sqlite3` in this repo (gitignored).

Source of truth for the schema: the plugin itself,
`koreader/plugins/statistics.koplugin/main.lua` (copied into
`research/koreader-plugin-src/statistics.koplugin/` from the live device —
this is the actual Lua source running on Brandon's Kindle, not upstream
GitHub, though it should match `koreader/koreader` upstream closely). The
schema-creation code is `main.lua:459` (`createDB`) plus the
`STATISTICS_DB_PAGE_STAT_DATA_SCHEMA` / `STATISTICS_DB_PAGE_STAT_VIEW_SCHEMA`
constants just above it. There's also a `migrateToDB` codepath and numbered
`upgradeDBtoNNNNNNNN` migration functions (e.g. `upgradeDBto20201010`) for
older schema versions — worth reading if we ever need to handle a
pre-2022 db, but not relevant for Brandon's own data.

### `book` table

```sql
CREATE TABLE book (
    id integer PRIMARY KEY autoincrement,
    title text,
    authors text,
    notes      integer,   -- COUNT of notes/highlights, not the content itself
    last_open  integer,   -- unix timestamp, seconds
    highlights integer,   -- COUNT of highlights, not the content itself
    pages      integer,   -- page count as KOReader currently renders it
    series text,
    language text,
    md5 text,             -- content hash, used for de-dup / identity
    total_read_time  integer,  -- seconds
    total_read_pages integer   -- cumulative pages "turned", NOT unique pages
);
CREATE UNIQUE INDEX book_title_authors_md5 ON book(title, authors, md5);
```

**`notes`/`highlights` are counts, not content.** Confirmed by column type
(`integer`) and by the live sample (values like `0`, `1`, `2`). The actual
highlight/note *text* lives elsewhere — per-book `.sdr` sidecar metadata
(a Lua table, `metadata.epub.lua` or similar, stored alongside the book or
in a KOReader-managed sidecar dir), not in this database. If Colophon wants
to surface highlight content (not just counts), that's a second, separate
data source to parse — out of scope for the core stats work, flag as a
possible later phase.

### `page_stat_data` table (raw per-page-turn events)

```sql
CREATE TABLE page_stat_data (
    id_book     integer,
    page        integer NOT NULL DEFAULT 0,
    start_time  integer NOT NULL DEFAULT 0,  -- unix timestamp, seconds
    duration    integer NOT NULL DEFAULT 0,  -- seconds spent on this page turn
    total_pages integer NOT NULL DEFAULT 0,  -- book's page count AT THAT MOMENT
    UNIQUE (id_book, page, start_time),
    FOREIGN KEY (id_book) REFERENCES book(id)
);
CREATE INDEX page_stat_data_start_time ON page_stat_data(start_time);
```

One row per page-turn event: which book, which page, when, how long, and
critically **`total_pages` is recorded per-row**, not looked up from `book`
— this is exactly the font-size/pagination-change handling from `spec.md`'s
open questions. Because re-flowing text at a different font size changes
how many "pages" the book has, KOReader stamps the page-count-at-the-time
onto every event, so historical rows stay internally consistent even if
`book.pages` later changes.

### `page_stat` view (the rescaled, comparable version)

```sql
CREATE VIEW page_stat AS
    SELECT id_book, first_page + idx - 1 AS page, start_time,
           duration / (last_page - first_page + 1) AS duration
    FROM (
        SELECT id_book, page, total_pages, pages, start_time, duration,
               ((page - 1) * pages) / total_pages + 1 AS first_page,
               max(((page - 1) * pages) / total_pages + 1,
                   (page * pages) / total_pages) AS last_page,
               idx
        FROM page_stat_data
        JOIN book ON book.id = id_book
        JOIN (SELECT number AS idx FROM numbers) AS N
             ON idx <= (last_page - first_page + 1)
    );
```

This is KOReader's own answer to the pagination-drift problem: it rescales
every historical `page_stat_data` row from whatever `total_pages` it was
recorded against onto the book's *current* `pages` count, splitting/merging
duration proportionally, via a `numbers` helper table (1..1000, a plain
tally table used to fan out one row into several during rescaling). **Use
this view, not the raw table, for anything that needs a stable page axis
across a book's history** (progress-over-time charts, "did it drag in the
middle" velocity views). Use the raw `page_stat_data` table when you want
truly raw session/duration data unaffected by rescaling (e.g. total time
reading, session-length distributions).

### Confirmed from the live sample

- 9 books, 695 `page_stat_data` rows.
- `total_read_time` / `total_read_pages` on `book` are running per-book
  totals KOReader maintains directly — don't need to be derived by summing
  `page_stat_data` (though they should reconcile; useful cross-check).
- Timestamps are plain unix epoch seconds, no timezone stored — convert in
  the local timezone at render time.

### Known gotchas (from reading the migration history, not yet stress-tested)

- Schema has changed over time (`DB_SCHEMA_VERSION`, `PRAGMA user_version`).
  Brandon's device is on `20221111`; don't assume every KOReader install is
  on the same version if this ever needs to handle someone else's export.
- `total_read_pages` is a *cumulative page-turn counter*, not "unique pages
  read" — re-reading pages (flipping back) increments it too. Don't treat
  it as a progress percentage without also consulting `pages`.
- Re-reads: nothing in the schema explicitly flags "this was a re-read" —
  it would show up as more `page_stat_data` rows against the same
  `id_book`/`page` combos at a later `start_time`, spread over a
  disconnected time range. Detecting "book read twice" is a derived
  query/heuristic, not a stored fact.
- Multi-device sync: this schema is purely local to the device it's stored
  on. KOReader's own multi-device story (the "Reading statistics: automatic
  sync" discussion referenced in the KoInsight docs — see §3) is handled by
  the KOInsight-provided sync-server protocol or manual db merge, not
  anything native to `statistics.sqlite3` itself. If Brandon ever reads on
  more than one device, merging two `statistics.sqlite3` files is a real
  problem to solve, not solved by this schema.

## 2. Vocabulary builder database

Path: `koreader/settings/vocabulary_builder.sqlite3`. Sample copied to
`research/samples/vocabulary_builder.sqlite3` (currently **empty** on
Brandon's device — 0 rows — since he hasn't used the vocab/flashcard
feature).

```sql
CREATE TABLE vocabulary (
    word          TEXT NOT NULL UNIQUE PRIMARY KEY,
    title_id      INTEGER,      -- FK-ish to title.id (which book it came from)
    create_time   INTEGER NOT NULL,
    review_time   INTEGER,
    due_time      INTEGER NOT NULL,   -- spaced-repetition scheduling
    review_count  INTEGER NOT NULL DEFAULT 0,
    prev_context  TEXT,          -- surrounding text where the word was looked up
    next_context  TEXT,
    streak_count  INTEGER NOT NULL DEFAULT 0,
    highlight     TEXT
);
CREATE TABLE title (
    id     INTEGER NOT NULL UNIQUE PRIMARY KEY,
    name   TEXT UNIQUE,
    filter INTEGER NOT NULL DEFAULT 1
);
```

Interesting but currently moot for Brandon's own data (empty). Worth a
low-priority widget ("words looked up per book") if he starts using the
feature, but shouldn't block v1.

## 3. Other data sources spotted but not yet pulled

- `koreader/settings/bookinfo_cache.sqlite3` / `PT_bookinfo_cache.sqlite3` —
  large (14 MB / 1.1 MB) thumbnail/metadata cache, almost certainly not
  useful for stats (cover art cache, not reading history).
- `koreader/settings/lookup_history.lua`, `bookshelf.lua`,
  `hardcoversync_settings.lua` — plain Lua settings files, not sqlite.
  `hardcoversync_settings.lua` implies KOReader has some integration with
  Hardcover (the Goodreads-alternative book-tracking site) — worth a look
  if Colophon ever wants to cross-reference external ratings/reviews, but
  not pulled or inspected this pass.
- Per-book `.sdr` sidecar metadata (actual highlight/note *content*, as
  opposed to the `book.notes`/`book.highlights` counts) — not located or
  copied this pass. Needed only if Colophon wants highlight content, not
  just counts.

## 4. KOReader's own built-in stats UI (surveyed 2026-07-03)

Read in full from the on-device plugin source in
`research/koreader-plugin-src/statistics.koplugin/` (main.lua 3242 lines,
calendarview.lua 1591, readerprogress.lua 503). This is what Colophon must
*not* merely re-skin.

### 4.1 Recording semantics (interpretation-critical for any consumer)

- On each page turn, the time since the previous turn becomes that page's
  duration, subject to two user settings (defaults `min_sec = 5`,
  `max_sec = 120`; main.lua:30-31): under `min_sec` the event is
  **discarded entirely**, over `max_sec` it is **clamped to `max_sec`** at
  record time (main.lua:2667-2682). Confirmed in the live sample: every
  row's duration is in [5, 120].
- Suspend/resume: screensaver time is *excised* (the pause duration is
  added to the current page's start timestamp on resume, main.lua:2775-2793),
  not clamped. So a row's `start_time + duration` chain can have clean gaps.
- Flush to DB every 50 page turns and on close/suspend/before every stats
  screen (main.lua:29, 2689).
- KOReader displays two different "total time" numbers for a book:
  *uncapped* (`sum(duration)` over `page_stat`) and *capped* (per distinct
  page, `min(sum(duration), max_sec)`; the `STATISTICS_SQL_BOOK_CAPPED_
  TOTALS_QUERY` at main.lua:41-50). `avg_time` (sec/page, used for the
  time-left estimate) comes from the **capped** totals. Colophon must
  reproduce both or its numbers won't match the device.
- "Today" = since local midnight computed in Lua; "session" = since the
  reader resumed/opened, not a DB concept (main.lua:1042, 2762-2765).
- `book.total_read_time`/`total_read_pages` are uncapped cached totals
  refreshed on flush (main.lua:971-984).
- A "daily timeline starts at" setting can shift the day boundary for the
  day view and (optionally) calendar queries (main.lua:2846, 2884); the
  rest of the UI uses plain local midnight.

### 4.2 What its screens show

Everything is a `KeyValuePage` text list except three real visualizations:

- **Current book / book statistics**: session time/pages, today time/pages,
  total time (capped and uncapped), estimated time left and finish date
  (`(pages_left) * avg_time`, `now + time_left / (read_time/days)`), days
  reading, average per day, average per page, start date, pages read as %
  of current page count, highlight/note counts.
- **Time range statistics**: eight drill-in lists (all books ranked by
  total time; books by week/month; last week / last month / last year by
  day; last year by week; all months), all built on one
  `GROUP BY id_book, page, strftime(bucket)` query family over the
  `page_stat` view (main.lua:1855-1913): "pages" = distinct
  (book, page, bucket).
- **Reading progress** (also the screensaver widget): last-7-days summary
  tiles, per-day horizontal bars normalized to the busiest day of the week
  (readerprogress.lua:196-280), session/today tiles.
- **Calendar view**: month grid; each day cell has a 24-bar hourly
  mini-histogram (bar height = fraction of that hour spent reading,
  main.lua:2837-2880) and up to 3 colored per-book spans that merge across
  consecutive days (calendarview.lua:194-248); colors are book_id mod an
  8-entry palette.
- **Today's timeline / day view**: 24-hour vertical timeline with per-book
  colored spans placed to the second; queries `page_stat_data` **directly**
  (real timestamps matter there); spans of the same book separated by
  ≤ `max(30, min_sec)` s are merged visually (main.lua:2922-3010).

### 4.3 What KOReader does NOT show (gaps Colophon can own)

No streaks; no reading-speed trends (pages/hour over time); no aggregate
time-of-day or weekday profile across the history (only per-day-cell
histograms); no cross-book pace comparison; no series/language rollups
(despite storing both columns); no yearly summaries or books-finished-per-
year (a "finished" state doesn't even exist in this DB; it lives in `.sdr`
sidecars); no session-length analysis; and essentially no charts beyond
the three above.

## 5. Existing third-party tools (read in depth 2026-07-03)

All four in `~/.gitrepos/.studyrepos/` (reference only, not Brandon's
code). Summary of each tool's metric catalogue and the load-bearing
implementation choices; line references are into those clones.

### 5.1 KoInsight (`GeorgeSG/KoInsight`, TS web dashboard, MIT)

Re-imports KOReader data into its own SQLite (books keyed by `md5`, page
stats keyed `(book_md5, device_id, page, start_time)`), then computes
everything in JS. Notable:

- **Reference pages** is its flagship idea: the user sets a canonical page
  count per book, and every `page_stat` row is mapped onto that axis. Each
  row `page` (out of its own `total_pages`) becomes the fractional interval
  `[(page-1)*ref/total, page*ref/total]`; interval union (merged, then
  summed; `apps/server/src/utils/ranges.ts`) gives **unique pages read**
  and a percent-complete immune to both re-reads and pagination drift.
  This is the single best derived-metric idea in any of the four tools.
- Multi-device: same-device re-syncs are idempotent (upsert on the 4-tuple)
  but genuinely concurrent reads on two devices double-count; it doesn't
  try to solve that.
- Charts (Recharts): GitHub-style dot-trail day heatmap (intensity relative
  to the single best day ever), weekly area chart, weekday bar chart
  (raw sums, not normalized), monthly bar chart, per-book donut + calendar.
- Conspicuously absent: sessions, streaks, reading speed, any time-of-day
  analysis, finished-state, year-in-review.

### 5.2 KoShelf (`paviro/KoShelf`, Rust + React static-ish site, EUPL)

The most sophisticated of the four, and the closest in spirit to a "real"
stats engine. Rust ingest → own SQLite → axum API → hand-rolled charts.

- **Copies the DB to a temp snapshot before opening** (WAL-safe), then
  opens `?mode=ro`, and reads the **`page_stat` view**, not the raw table
  (`src/source/koreader/database.rs`).
- **Dedupes `book` rows sharing an md5** (KOReader's unique index is
  (title, authors, md5), so editing a book's metadata creates a second row
  for the same file): canonical row = max(last_open, id), totals summed,
  page stats remapped (`database.rs:146-206`).
- **Session = per-book cluster of page stats where each row starts ≤ 300 s
  after the previous row's end** (`compute/sessions.rs:6`). Sessions never
  span books. Derived: count, average, longest, per-bucket series.
- **Completion detection**, its flagship idea: a "read-through" is a
  progression of page stats visiting ≥ 78 % of pages including a page in
  the first 20 % and one in the last 2 %; backwards jumps to the first 5 %
  split a new progression only if the remainder would itself form a valid
  completion (`compute/completion_detection.rs`). Yields per-completion
  start/end dates, reading time, session count, and pages/hour. This is
  how it gets books-finished-per-year out of a DB with no finished flag.
- **Streaks**: consecutive-calendar-day runs; current streak alive iff the
  last read day is today or yesterday.
- **Configurable logical day start** (`--day-start-time HH:MM`; reading at
  02:00 can count as the previous day) and timezone (`time_config.rs`).
- **Per-page activity grid** per book: per-page total duration + read
  count, sqrt height scaling capped at the 90th percentile, with
  highlight/note/bookmark markers at their pages, optionally filtered to
  one completion. The most underexplored visualization in the space.
- Also: yearly recap (share images), month calendar with multi-day event
  spans, ISO-week stats, min-pages/min-time noise filters, "pagemap"
  synthetic-page scaling when sidecars carry it.

### 5.3 Kodashboard (`Yuchen971/Kodashboard`, on-device plugin + web SPA)

Queries `page_stat_data` directly (not the view) with
`date(start_time,'unixepoch','localtime')` day bucketing throughout.

- Calls every raw row a "session" (no gap threshold anywhere) and counts
  page-turn *events* as "pages"; a caution, not a model to copy.
- Streaks: same today-or-yesterday grace rule, implemented three separate
  times (server + client + per-window).
- Charts: SVG area trend, monthly bars, weekday-average bars, 24-hour
  activity pills, top-books meters, month heat calendar (continuous 0-1
  intensity normalized to the 90-day max), per-book 13-week Monday-aligned
  heatmap that also marks annotation-only days, milestones timeline, and an
  "insights" prose card (pace up/down/steady = second half vs first half of
  the window at ±15 %).
- Reads highlight *content* from `.sdr` sidecars via raw Lua `dofile`,
  matching sidecar `partial_md5_checksum` against `book.md5`, with a fuzzy
  title/author/pages scorer as fallback.
- Trusts `book.total_read_time`/`total_read_pages` for all-time totals.

### 5.4 readingstreak.koplugin (`advokatb`, on-device streak plugin)

Streaks from its own `reading_history` array of "YYYY-MM-DD" strings, not
live DB queries; the DB is only used for one-time import and weekly-time
display. The algorithm details are the reference for Colophon's streak
widget:

- A day counts once any page turn happens (optional page/time thresholds,
  both default 0). Current streak alive iff last read day is today or
  yesterday; a gap of ≥ 2 days zeroes it; longest = max consecutive run.
  Date math via Julian day numbers (DST-safe).
- Its import query is the correct "distinct pages per day" shape:
  `GROUP BY date, id_book, page` inside, then `GROUP BY date` outside, over
  the `page_stat` view in localtime (`statistics_importer.lua:32-46`).
- Also tracks week streaks (Monday-start, hand-rolled week numbering) and
  a binary month calendar (day read / not read).

## 6. Conventions Colophon adopts (converged across tools)

| Concern | Convention | Source |
|---|---|---|
| Day bucketing | `date(start_time,'unixepoch','localtime')`, i.e. local midnight | all tools + KOReader itself |
| Session | per-book; new session when a row starts > 300 s after previous row's end | KoShelf (only real implementation) |
| Streak | day counts if any reading; current streak alive if last day is today **or** yesterday; ≥ 2-day gap → 0 | readingstreak, Kodashboard, KoShelf (independent convergence) |
| "Pages read" | distinct (book, page) per bucket, never raw event count | KOReader, readingstreak |
| Book identity | `book.md5` (KOReader's partial MD5); merge `book` rows sharing an md5 | KoShelf, KoInsight, Kodashboard |
| Page axis | `page_stat` view for anything page-positional; `page_stat_data` for real timestamps (timelines) | KOReader itself |
| Device parity | reproduce both capped and uncapped totals; label which is shown | KOReader (§4.1) |
| Progress % | interval-union of fractional page spans (unique pages read) | KoInsight |
| Completion | 78 % / first-20 % / last-2 % progression detection | KoShelf |

Junk filtering is Colophon's own addition: the live sample contains plugin
READMEs and the quickstart guide as "books" (§1); a minimum-total-read-time
display filter (KOReader's own purge uses < N minutes) handles it.

Note the *Jingo* case in the sample: two `book` rows with the same
title/authors but different md5s and page counts. That is two different
*files* (e.g. a re-download), not the metadata-edit duplication KoShelf's
md5-dedup solves. Grouping same-title/author books for display is a
Colophon design decision (group in UI, never merge in data).

## 7. The `.sdr` sidecar question (answered from source, sample pending)

Structure fully documented from KoShelf's parser + Kodashboard's loader;
no live sample copied yet (Kindle wasn't mounted this pass; grab
`<book>.sdr/metadata.epub.lua` for one highlighted book next time it is).

- Location: `<book dir>/<stem>.sdr/metadata.<ext>.lua` next to the book by
  default (KOReader can be configured for central `docsettings/` or
  hash-named `hashdocsettings/` dirs instead).
- Format: a Lua chunk `return`ing a table. Keys of interest:
  `annotations` (array of `{chapter, datetime, pageno, pos0, pos1, text,
  note, color, drawer}`; no `drawer` ⇒ bookmark, `note` present ⇒ note),
  `partial_md5_checksum` (**the join key to `book.md5`**),
  `percent_finished` (0-1), `summary` (`status` reading/complete/abandoned,
  `rating` stars, `note` review text, `modified` date), `doc_props`,
  `doc_pages`, plus typesetting state (font, margins, pagemap fields).
- Parsing in Rust: KoShelf evaluates the chunk in a sandboxed `mlua` VM
  (`Lua::new_with(StdLib::NONE, ...)`), lossily fixing invalid UTF-8 first.
  Clean, safe, directly reusable pattern, but it means an `mlua` dependency
  when this comes in scope (ask first, per house rules).
- `summary.status` is also the only source of a user-declared
  "finished" state anywhere in KOReader; the stats DB has none.

## 8. What's genuinely underexplored (Colophon's opening)

Across KOReader's UI and all four tools, nobody ships:

1. **Reading-speed analytics.** Pages/hour over time, per-book vs library
   baseline, speed by time of day. (KoShelf computes a single pages/hour
   per completion; that's the entire field.)
2. **Session-length analysis.** KoShelf counts/averages sessions; nobody
   shows a session-length distribution, session start-time patterns, or
   long-session records.
3. **Aggregate time-of-day × weekday profile.** "When do I read" as a
   7×24 heatmap over the whole history. KOReader has per-day cell
   histograms; Kodashboard has a flat 24-hour profile; nobody crosses them.
4. **Per-book velocity narrative.** "Did it drag in the middle": time per
   page position across a book (the `page_stat` view exists precisely to
   make this valid), or pace-per-day through a single read.
5. **Cross-book comparison.** Ranked pace, session habits, or calendar
   span per book; series/language/author rollups.
6. **A native desktop presentation.** Every one of these is a web page.

Also worth stealing but not novel: KoShelf's completion detection and
per-page activity grid, KoInsight's interval-union progress, the standard
streak/calendar-heatmap set.

## 9. Status for `spec.md` / `roadmap.md`

- Schema (§1), granularity/font-size handling (§1), KOReader's own UI
  (§4), the four tools' catalogues (§5), and the `.sdr` structure (§7) are
  all answered. Phase 0's research goals are met.
- `spec.md`'s widget list is now locked against §4.3/§6/§8 (done in the
  same pass as this update).
- Remaining loose ends, none blocking: copy one real `.sdr` sample when
  the Kindle is next mounted; multi-device merge stays out of scope until
  a second device exists (KoInsight's 4-tuple upsert is the reference if
  it ever does).

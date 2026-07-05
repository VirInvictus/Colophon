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

### 5.5 Tome (`bndct-devops/tome`, FastAPI + React self-hosted server, AGPL-3.0)

Added 2026-07-05 (a fifth tool, read after the original four). The most
feature-complete of the set, but it is a full self-hosted library *server*
(Docker, web UI, OPDS, SSO, Hardcover sync, a custom KOReader plugin), i.e.
exactly the category Colophon exists to avoid. It matters here only as a
metric-idea source. The important finding first:

**It does NOT read richer reading telemetry than Colophon.** Tome has two
reading-data sources: live `ReadingSession` rows POSTed by its own TomeSync
KOReader plugin, and imported `statistics.sqlite3` `page_stat_data` (byte
for byte Colophon's source). Its reconciliation layer makes the imported
page-stats **win** for any book that has them and **discards** the live
device sessions for that book to avoid double-counting
(`reconciled_reading.py:8-12,61-75`); the plugin history import is
explicitly "reading time and pages only, never read/unread status"
(`ko_stats_import.py:31-35`). The plugin adds annotation *content*
(xPointer anchor, text, note, chapter, colour), a resume-position CFI, and
a user star-rating synced both ways, but no time signal beyond
`page_stat_data`. So for time and pages, Tome and Colophon see the same
data. Its live sessions carry `progress_start`/`progress_end`/`pages_turned`/
`device`, but Colophon reconstructs the equivalents by gap-clustering
page-stat rows exactly as Tome itself does for imported books
(`reconciled_reading._cluster_rows`, 1800 s gap, 10 s min).

The genuinely-new *statistical axes* Tome has that are absent from
`statistics.sqlite3` are three: **word counts** (parsed from the EPUB
files, not from KOReader), **user ratings** (KOReader `.sdr` sidecars),
and **library catalogue metadata** (book-type/"genre", language,
`added_at`, whole-library TBR, from Tome's own DB). Everything Tome
computes divides cleanly along that line, which is what makes the delta
easy to state.

**Re-implementations (nothing new for Colophon):** headline totals, streaks
(identical walk-back-from-today logic, `streaks.py:17-39`), 365-day year
heatmap, hour×weekday 168-cell heatmap, monthly bars, session histogram,
pace in pages/min, series completion section, per-page reading-intensity
curve (same fraction-bin + interval-union as Colophon), re-read detection.
One reference detail worth keeping: Tome's re-read heuristic groups by
`(book, total_pages, page)` so a re-pagination cannot fake a revisit, ranks
books by revisited-page count, and drops books with `< 3` revisited pages
as noise (`reading_stats.py:920-958`). Also a cheap correctness nicety: it
buckets every day-identity metric with a **4 a.m. logical-day rollover** (a
01:30 session counts to the prior day), the one exception being the
hour-of-day heatmap which uses the raw offset (`reading_day.py:1-13`). This
matches KoShelf's `--day-start-time` idea.

**New, and feasible from `statistics.sqlite3` alone** (needs only KOReader's
`book` + `page_stat`; no plugin, no word counts, no sidecars):

- **Author affinity**: reading time and finished-book count rolled up per
  author, top N (`stats.py:515-542`). A whole dimension (author) Colophon's
  widget list omits; `book.authors` is right there. Near-zero cost.
- **Personal records**: longest single session, biggest reading day by
  time, most pages in a day (`stats.py:747-764`). Max over the structures
  Colophon already builds.
- **Per-book finish estimate + momentum**: seconds-remaining =
  `total_seconds / progress * (1 - progress)` (`reading_stats.py:389-397`);
  "momentum" = last 7 reading-days vs the prior 7, direction + % delta
  (`:330-362`); a days-to-finish endpoint with high/medium/low confidence
  from evidence count (`stats.py:1018-1158`), deliberately not subtracting
  the first active day from the pace denominator (`:1113-1117`).
- **Completion / abandonment rate**: started → finished % (`stats.py:544-558`).
  Tome uses an explicit status; Colophon would derive "started" from any
  dwell and "finished" from its own 78%/last-2% completion heuristic
  (honest caveat: heuristic, not user-declared).
- **Year-in-review card**: a Wrapped-style composite tile: books finished,
  hours, longest streak, sessions, most-active month, top genre
  (`stats.py:330-375`). Pure re-aggregation of numbers Colophon already has
  (drop top-genre, which needs catalogue metadata). Presentation novelty.
- **Period-over-period comparison**: current window vs the immediately
  preceding equal-length window, % change, returning null (not a fake ∞%)
  when the prior window is empty (`stats.py:309-328`).
- **Forgotten / stale books**: books in "reading" state untouched 30+ days
  (`home.py:264-294`). Cheap home-rail widget.
- **Reading goals**: `{books|minutes|pages}_per_{day|week|month|year}`,
  with a prorated "on-pace" line for month/year and `days_hit_this_week`
  for daily goals (`goals.py`). Colophon *deliberately* declined goals, so
  this is an available-but-declined idea, not a gap; re-raise only if wanted.

**New, but requires word counts (the single biggest capability delta):**
Tome parses the **EPUB file itself** for word counts, not KOReader.
`count_words_epub` opens the EPUB via `ebooklib` and, on any parse failure,
falls back to reading the zip directly with stdlib `zipfile`
(`metadata.py:60-103`): strip `<script>`/`<style>`, strip tags, count a
Unicode word regex over every spine document, summed and cached on the
book; a cancellable background job backfills the library with a
byte-weighted ETA (`word_count_job.py`). This unlocks
(`stats.py:825-913`): **words read** (lifetime + by year), **true WPM**
(`words × 60 / read-seconds`, with a 300 s floor below which pace is
dismissed as noise; pagination-independent, unlike Colophon's pages/hour),
and a **book-length distribution** (mean/median/longest + fixed buckets
`<50k / 50-100k / 100-150k / 150-250k / 250k+`, and avg length by
finish-year). **Feasibility: NOT from `statistics.sqlite3`.** It needs the
actual book files plus an EPUB word counter, and Colophon's contract today
is stats-DB-only with no library-file access in the pipeline. The stdlib
zip fallback shows it is achievable without a heavy dep, but it is a
deliberate scope decision (touch library files? add or replicate an EPUB
parser? `ebooklib` is third-party) to make explicitly, not slip in.

**New, but requires catalogue metadata Colophon cannot get** (all blocked
by data, not choice; KOReader's DB only knows books you *opened*, so it has
no book-type, language, `added_at`, or owned-but-unread): genre/book-type
time and genre-over-time stacks (`stats.py:206-214,430-458`; note it is
book-*type* manga/novel/comic, not literary genre, so the README's "genre
trends" is narrower than it sounds), time by language (`:807-823`), TBR /
library completion (`:766-805`), library-growth timeline (`:613-648`),
pace by file format (`:584-611`).

**New, but requires ratings** (KOReader stores the star rating/review in the
per-book `.sdr` sidecar, not the stats DB): a full ratings block, i.e.
half-star distribution, average, rating-over-time trend, per-series ratings
(`stats.py:650-732`). Reachable only if Colophon starts ingesting `.sdr`
sidecars (the sample §7 already wants to grab); the by-category facet stays
blocked even then.

**Reading DNA (5 traits) vs Colophon's 3-trait reading personality.**
`reading_dna.py` scores five 0-100 axes and names an archetype (noun from
the most-extreme axis + modifier from the second, forced to be different
axes so it cannot self-contradict; an axis must sit ≥ 12 from neutral-50 to
be "named", `:63,174-203`):

| Trait | Computation | Feasible for Colophon |
|---|---|---|
| Time (early-bird↔night-owl) | circular mean of read-hour over 365 d, 4 a.m. boundary (`:89-100`) | YES (page-stat timestamps) |
| Rhythm (sporadic↔consistent) | active-day density over last 120 d, gated to ≥ 14 days history (`:102-113`) | YES (active-day set) |
| Variety (focused↔eclectic) | `1 − HHI` (Herfindahl index) across authors **and** book-types, averaged (`:152-172`) | PARTIAL (author half works; type half blocked) |
| Length (short↔long) | median finished-book **word count**, 30k→150k calibration (`:128-133`) | NO (needs word counts) |
| Pace (savorer↔speed-demon) | **WPM**, 140→360 calibration, ≥ 300 s books only (`:135-150`) | NO (needs word counts) |

So vs Colophon's three traits, the new axes are Length and Pace (both
word-count-gated) and an HHI-based Variety (author-feasible). Time and
Rhythm are territory Colophon already occupies. The **HHI diversity index**
and the tone-calibrated "flatter the low pole as much as the high pole"
archetype naming are worth borrowing as *technique* even where the axes
overlap.

**Explicitly out of scope for Colophon** from Tome: release detection
(polls Hardcover's GraphQL API for new series volumes; external network,
against the local-first/offline rule) and `book_progress` write paths
(Colophon is read-only; its one transferable idea, "completion is sticky,
re-reads don't un-finish a book", already matches Colophon's completion
detection).

### 5.6 Residual sweep of the first four tools (2026-07-05)

Before deleting the reference clones, each of the original four was re-read
against its §5 summary to drain anything the first pass missed. Two of the
findings are correctness checks that Colophon **already passes**, which is
worth recording as validation:

- **WAL snapshot must copy the `-wal` and `-shm` companions**, not just the
  `.sqlite3`; copying only the main file while KOReader has uncheckpointed
  pages in the WAL reads a stale snapshot and silently drops the newest
  events (KoShelf `src/source/sqlite_snapshot.rs:22-46`). Colophon's
  `snapshot()` does copy both sidecars then checkpoints
  (`colophon-core/src/db.rs:242-282`). Confirmed correct.
- **md5-merge must take `max()`, not `sum()`, for `notes`/`highlights`**:
  those are absolute counts, so summing across md5-duplicate rows
  double-counts (KoShelf only sums `total_read_time`/`total_read_pages`,
  `database.rs:171-184`). Colophon already uses `.max()`
  (`colophon-core/src/db.rs:231-232`). Confirmed correct. (The §5.2 line
  "totals summed" was imprecise; the counts are maxed.)

New reference material the first pass missed:

- **Junk / noise filters at the row level.** KoInsight drops any `page_stat`
  row with `duration <= 0`, `total_pages <= 0`, or non-finite values
  (`upload-service.ts:51-59`) — a data-quality guard independent of
  Colophon's display-time min-read-time filter. KoShelf's noise filter is
  coarser and per-`(book, logical_date)`: a day-bucket survives if it clears
  `min_time_per_day` (default `30s`) or `min_pages_per_day` (unset), and a
  failing bucket drops *all* its rows; "pages" there means row count, not
  distinct pages (`calculator.rs:332-388`, `cli.rs:131`). KoShelf also
  prefilters completion detection to `duration > 0` rows
  (`completion_detection.rs:172`).
- **`book.pages` is not authoritative for current pagination.** After a
  reflow it goes stale and disagrees with the `total_pages` stamped on
  recent `page_stat_data` rows; trust the freshest row's `total_pages`
  (KoInsight `db_reader.lua:63-80`). Across devices, use `max(total_pages)`
  when no user reference count is set (`books-service.ts:10-12`). A
  `max(page - 1, 0)` clamp guards the page-0 off-by-one in interval mapping
  (`books-service.ts:44`).
- **Synthetic-pagination scaling: accumulate as float, round once per
  bucket.** When a sidecar carries a pagemap, KoShelf scales by
  `pagemap_doc_pages / book.pages` but returns the raw f64 so callers sum
  scaled pages as floats and round only once per day/month bucket, avoiding
  per-row rounding drift; non-finite/≤0 factors are rejected
  (`scaling.rs:27-100`).
- **Completion restart-split is a two-threshold rule, not one.** The §5.2
  "backwards jump to first 5%" fires only when the current page ≤ 5%
  **and** the previous page was > 20% **and** the progression already holds
  an early page **and** the remainder would itself complete
  (`completion_detection.rs:259-292`); defaults 0.78 / 0.20 / 0.02.
- **Day-start offset math.** The logical-day start is *subtracted* from the
  localized datetime before taking the date, so 02:00 with a 04:00 start
  counts as the previous day (KoShelf `time_config.rs:64-75`; Tome's
  `reading_day.py` does the same at 04:00). DST fall-back ambiguity resolves
  to the earliest instant (`time_config.rs:111-120`). readingstreak, by
  contrast, has **no** offset (bare local midnight, `main.lua:147-148`) — a
  differentiator if Colophon wants the offset.
- **DST-safe date arithmetic (reusable primitive).** readingstreak does all
  streak day-diffs via the Fliegel–Van Flandern Julian-day formula on parsed
  Y/M/D integers, never `os.time`, so it is immune to DST/timezone
  (`main.lua:151-166`). Its *weekly* window, by contrast, uses
  `days * 86400` second math on `os.time` and is DST-fragile
  (`time_stats.lua:36,47`) — a cautionary inconsistency. Its week numbering
  is hand-rolled and **diverges from ISO 8601** (no W53/W00 or year-boundary
  handling, `main.lua:168-180`); do not copy it for week streaks.
- **Milestone tiers, and how thin they are.** readingstreak's only
  achievement ladder is 7 / 30 / 100 / 365 days plus a configurable
  `streak_goal` (default 7) congratulated on exact equality
  (`main.lua:238-251`, `streak_calculator.lua:216`). Kodashboard's
  "milestones timeline" is just three fixed points (first open, first
  annotation, last open; `app.js:2160-2230`) — thinner than the §5.3
  summary implied, not a rich ladder.
- **Idle cap.** readingstreak clamps inter-page-turn gaps to
  `MAX_TRACKED_INTERVAL = 45 min` before summing its own live duration
  (`daily_progress.lua:8,67`), mirroring KOReader's idle timeout. A
  reference constant if Colophon ever computes duration from raw gaps rather
  than trusting `page_stat.duration`.
- **Same-title dedup tiebreak.** Kodashboard ranks duplicate display records
  by `last_open`, then a quality score (`md5 +20, cover +12, known-author
  +10, pages +8, percent +6, highlights +4`; `app.js:945-967`). Colophon's
  same-title grouping (§6) could borrow this ordering to pick which file's
  metadata to show.

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

Note the duplicate-title case in the sample: two `book` rows with the same
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

### 7.1 Annotation placement — the parked "activity-strip markers" question, answered

The Phase 3 item "annotation markers on the activity strip (needs
sidecars)" has a clear answer, converged across three tools:

- **Annotations suffer the same pagination drift as page stats, and take
  the same fix.** An annotation's `pageno` becomes wrong after a reflow, so
  KOReader (and KoInsight, mirroring it) stamps `total_pages` onto the
  annotation at creation time; placing a marker means rescaling its page
  onto the current axis exactly like the `page_stat` view rescales dwell,
  **not** using the raw `pageno` (KoInsight
  `plugins/koinsight.koplugin/annotation_reader.lua:93-105`; falls back to
  `doc_pages` when absent). So Colophon's activity strip already has the
  machinery — its per-page interval rescale — and annotation markers reuse
  it; naive `pageno` positioning would misplace them on any reflowed book.
- **Three-way type classification, agreed by all three sidecar readers.**
  bookmark = no `drawer` field; highlight = has `drawer`/`text`; note = has
  a `note` field. Render precedence: Note if `note` present, else Highlight
  if `text` present, else Bookmark (KoShelf
  `src/shelf/library/page_activity.rs:131-137`,
  `models/koreader_metadata.rs:89-101`; KoInsight
  `annotations-repository.ts:206-213`; Kodashboard `dataloader.lua:399-409`).
- **`percent_finished` is an authoritative completion fraction** KOReader
  writes to the sidecar directly, independent of Colophon's interval-union
  detection — a cross-check, and a finished-state signal for books the
  heuristic is unsure about.

### 7.2 Sidecar parsing hardening (when Colophon takes the `mlua` dep)

KoShelf's parser is the reference for doing this safely:

- Sandbox: `Lua::new_with(StdLib::NONE, ...)` (no `os`/`io`/`require`, so a
  malicious sidecar can't escape) plus `.set_mode(ChunkMode::Text)` to
  refuse precompiled bytecode (`src/source/koreader/lua_parser.rs:25,50`).
- **UTF-8-lossy fallback**: KOReader can truncate highlight text mid
  multibyte character; on `from_utf8` failure fall back to
  `from_utf8_lossy` rather than dropping the whole file
  (`lua_parser.rs:36-45`).
- Flags like hyphenation/font are stored inconsistently as bool-or-0/1-int;
  coerce both (`lua_parser.rs:277-321`).
- **Hidden flows** ("handmade flows": user-marked appendices/front matter
  KOReader excludes from progress): compute the hidden page ranges from
  `handmade_flow_points` and subtract them from the page count *before*
  completion detection, or completion % is understated and the 78% gate is
  unreachable (KoShelf `models/koreader_metadata.rs:34-66`,
  `types.rs:167-187`). Sidecar-gated, but necessary for correct completion
  on any book with a hidden appendix.
- **Fuzzy sidecar↔DB matching**, when the md5 join fails: first sanitize
  the KOReader title (normalize CJK full-width punctuation to ASCII, strip
  z-library / `1lib.sk` noise tokens and `(…)`/`[…]` parentheticals, smart
  quotes, dashes — Kodashboard `dataloader.lua:59-90`), then a weighted
  scorer (title exact +100 / substring +60, author exact +30, pages +20;
  md5 short-circuits to 999) with an **accept gate of score ≥ 40**
  (`dataloader.lua:92-131,591`). The join key when md5 *is* present is the
  sidecar's `partial_md5_checksum`.

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

### 8.1 Tome-sourced steal-list (2026-07-05, full detail in §5.5)

Ranked by value, tagged by data feasibility. The first bucket needs only
`statistics.sqlite3`, which Colophon already reads, so it is directly
actionable:

1. **Author affinity** (time + finished per author) — a dimension the
   widget list omits; `book.authors` is already loaded. Cheapest win.
2. **Personal records** (longest session, biggest day, most pages/day) —
   max over structures Colophon already builds. High delight per line.
3. **Per-book finish estimate + reading momentum** (7d-vs-prior-7d
   trend, days-to-finish with confidence) — extends the velocity work.
4. **Completion / abandonment rate**: started→finished %, using
   Colophon's existing completion heuristic for "finished".
5. **Year-in-review card**, **period-over-period % delta**, **forgotten
   books (reading, untouched 30+ days)**: all trivial re-aggregations.

Bigger, off-contract: a **word-count axis** (words-read, true WPM,
book-length distribution, and it unlocks two more Reading-DNA traits) is
the single largest capability gap, but it needs the EPUB files and a word
counter, which Colophon's stats-DB-only pipeline does not touch today. A
deliberate scope call, not a slip-in (see §5.5). Borrow the **HHI
diversity index** for a Variety trait regardless (author half is
stats-DB-feasible). **Reading goals** are available from the stats DB but
were deliberately declined; re-raise only on request. Genre/language/TBR
metrics and the ratings block stay blocked on catalogue metadata and
`.sdr` sidecars respectively.

## 9. Status for `spec.md` / `roadmap.md`

- Schema (§1), granularity/font-size handling (§1), KOReader's own UI
  (§4), the tools' catalogues (§5, now five tools including Tome in §5.5),
  and the `.sdr` structure (§7) are all answered. Phase 0's research goals
  are met.
- `spec.md`'s widget list is now locked against §4.3/§6/§8 (done in the
  same pass as this update).
- **2026-07-05 additions:** Tome (`bndct-devops/tome`) read and dossiered
  (§5.5) with a ranked steal-list (§8.1); the original four re-swept for
  residual value (§5.6); the parked Phase 3 "annotation markers on the
  activity strip" question answered (§7.1 — reuse the existing page rescale,
  three-way type classification). The five reference clones under
  `~/.gitrepos/.studyrepos/` were **deleted after this pass**; everything of
  value is captured here, and each is re-clonable from its upstream if
  needed (KoInsight `GeorgeSG/KoInsight`, KoShelf `paviro/KoShelf`,
  Kodashboard `Yuchen971/Kodashboard`, readingstreak `advokatb/…`, Tome
  `bndct-devops/tome`).
- Remaining loose ends, none blocking: copy one real `.sdr` sample when
  the Kindle is next mounted; multi-device merge stays out of scope until
  a second device exists (KoInsight's 4-tuple upsert is the reference if
  it ever does).

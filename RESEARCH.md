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

## 4. Existing third-party tools (cloned for study, not yet deeply read)

Shallow-cloned into `~/.gitrepos/.studyrepos/` (outside this repo, reference
only, not Brandon's code):

- **`GeorgeSG/KoInsight`** — the main one Brandon asked about by name. Web
  dashboard (upload a `statistics.sqlite3` or sync a plugin live), has grown
  a "Reference Pages" feature to normalize progress across devices/layouts,
  plus annotation (highlights/notes) sync. This is exactly the
  web-dashboard pattern Colophon exists to avoid *using*, but its feature
  set is the best map of "what's worth showing" — read its dashboard
  components / API layer for chart and metric ideas before designing
  Colophon's widget catalogue.
- **`paviro/KoShelf`** — combines notes/highlights with statistics into one
  dashboard. Relevant for the highlight-content angle (§1, §3) since it
  presumably already solved reading `.sdr` sidecars.
- **`Yuchen971/Kodashboard`** — a KOReader-side plugin serving a local web
  dashboard (library, stats, calendar activity, highlights). Different
  approach (runs on-device) but likely queries the same schema documented
  above; useful as a second implementation to cross-check query shapes
  against.
- **`advokatb/readingstreak.koplugin`** — small, focused KOReader plugin
  just for streak tracking. Good, minimal reference for one specific derived
  metric (streaks) without wading through a full dashboard app.

**Not yet done, still open for the next research pass:** actually reading
these four repos' source for (a) what KOReader's own built-in statistics
screen shows natively (so Colophon doesn't duplicate it) and (b) the full
list of derived metrics/charts they've already built, to sanity-check or
extend the widget brainstorm in `spec.md`. This pass only confirmed the
underlying schema and got the tools cloned locally.

## 5. Immediate implications for `spec.md` / `roadmap.md`

- `spec.md` open question #1 (real schema) is now answered — see §1 above.
  Update the spec to drop the "unconfirmed" framing and cite this document.
- Open question #2 (data granularity / font-size handling) is answered:
  `page_stat_data.total_pages` per-row plus the `page_stat` rescaling view.
- Still genuinely open: KOReader's built-in stats UI survey, the deeper
  read of the four cloned tools, and the `.sdr` highlight-content question.

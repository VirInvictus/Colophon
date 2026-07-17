# Colophon — spec

**Status: locked for v1 (Phase 0 complete, 2026-07-03).** The
`statistics.sqlite3` schema, KOReader's own built-in stats UI, and the four
existing third-party tools have all been read from source; findings and
citations live in `RESEARCH.md`. The widget catalogue below is a
commitment, not a brainstorm.

## Core concept

Colophon is a native GTK4 desktop app that turns KOReader's
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
  self-hosted Docker service. This is a real native GTK4 app.
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
- **Read-span coverage (positional)**: interval union on the page axis.
  Each event's page (out of its own recorded `total_pages`) maps to the
  fractional span `[(page-1)/total, page/total]`; the merged set of spans
  is the *positional* record of which parts of the book were read in
  KOReader. Immune to re-reads and pagination drift.
- **Unique pages read**: total merged span length x current page count.
  This is a *coverage* measure ("how much of the book KOReader logged"),
  not a progress measure. It under-counts any book partly read outside
  KOReader (e.g. before a mid-book KOReader install), where a leading
  span is simply absent.
- **Furthest position reached**: the maximum span upper bound, i.e. the
  deepest fractional position any event reached. This is the *progress*
  measure ("how far through the book you got"), and unlike coverage it is
  unaffected by an unlogged leading gap. For a book read to its final
  page it is 1.0 even when coverage is well below that.
- **Reached the end / finished**: the sidecar's user-declared
  `summary.status` is authoritative when the user has provided the book's
  sidecar; otherwise the inferred furthest position `>= 0.98` (the last-2 % endpoint the completion detector
  also uses). A book can be declared finished without the device having
  logged the end, and vice versa. This single reconciled value
  (`LibraryEntry::is_finished`) drives the per-book "Finished" marker and
  every finished count (series, author, recap, completion rate).
- **User-provided files (the app never reads the device)**: Colophon only
  ever sees files the user hands it, exactly as it does the stats database
  (which the user imports). It does not scan or read anything on the device.
  Any statistic that needs a file the user has not provided simply does not
  show until they add it. This is a deliberate boundary, not a limitation.
- **`.sdr` sidecars, per book**: the user adds a book's `metadata.*.lua`
  sidecar from that book's own page. Colophon checks its
  `partial_md5_checksum` matches the book (rejecting another book's file),
  copies it into its own cache (`<data>/sidecars/<book-md5>.lua`), and parses
  it in a sandboxed Lua VM (no stdlib, text chunks only, UTF-8 repaired
  lossily). On load each book looks up its own cached sidecar by md5; a book
  with none keeps the inferred stats. The cache is app-owned and the sidecar
  is only ever read.
- **Per-book progress display**: a positional span bar drawing the
  read-span coverage on the `[0, 1]` page axis (read regions filled,
  unlogged gaps empty) with a marker at the furthest position, plus the
  Finished marker when the end was reached. It shows *where* reading was
  logged, not a single left-anchored fraction, so a book read from the
  middle onward reads honestly rather than as "partly done".
- **Completion (inferred read-through)**: KoShelf's detection, adopted
  as-is: a progression of events visiting >= 78 % of pages including a
  page in the first 20 % and one in the last 2 %; a jump back to the
  first 5 % starts a new progression only if the remainder would itself
  qualify. Yields start/end dates, time, sessions, pages/hour per
  completion.
- **Finished-books timeline**: each finished work (per the reconciled
  finished rule above) paired with a finish date: the end of its most recent
  detected read-through when there is one, otherwise its last reading day, so
  a book finished off the device and known only from its sidecar still gets
  placed. Files of one work collapse to the most recent finish; ordered
  most-recent first. Whole-history.
- **Annotation markers**: highlights, notes, and bookmarks from a provided
  `.sdr` sidecar, placed on the per-book activity strip at each annotation's
  fractional position through the book (its `pageno` over the sidecar's own
  page count, so a marker lands correctly under any current pagination, the
  same rescale idea as `page_stat`). Kind by KOReader's rule: a bookmark has
  no drawer, a note carries a `note`, otherwise a highlight. Absent a
  sidecar for the book, no markers.
- **Book identity**: `book.md5`. Rows sharing an md5 (metadata edits) are
  merged at ingest. Same-title/author books with different md5s (two
  files of the same work, confirmed in the sample data) are *grouped for
  display only*, never merged in data.
- **Junk filter**: books below a minimum total read time (default 5
  minutes, configurable) are hidden from library-wide widgets by default;
  they remain queryable.
- **Rollups (series, author)**: whole-library groupings, window-independent
  (they use `book.total_read_time`, KOReader's cached all-time per-book
  total, so a time-window selection never touches them). *Series* groups by
  the Calibre-style `series` field (`"Name #index"` parsed to `"Name"`; the
  empty and `"N/A"` placeholders are skipped). *Author affinity* groups by
  the `authors` string as KOReader stores it (one field; not split into
  co-authors). In both, files of one work (same title within a group) count
  once toward the book and finished counts; "finished" is the inferred
  furthest-position >= 0.98 (the declared `.sdr` flag once sidecars are in
  scope). Each rollup sums read time and ranks its entries: series
  most-recently-read first, author affinity by total time (top authors
  first).
- **Reader profile**: a synthesis of already-defined behaviour metrics into
  named traits (no new data; classification only). Three are computed over
  the selected window. *Chronotype* from the hour-of-day totals' peak hour:
  early bird (05–10), daytime (11–16), evening (17–20), night owl (21–04).
  *Session style* from the **time-weighted typical** session (the length at
  or below which half of total reading time accumulates, not the plain
  count-median): marathoner (>= 45 min), sipper (<= 10 min), otherwise
  steady. Time-weighting is deliberate: a pile of tiny sessions (device
  tinkering, quick lookups) drags the count-median down and would mislabel a
  real reader a sipper, but holds little actual time, so it barely moves the
  typical. *Weekly rhythm* from the mean
  weekday seconds: weekend reader when the Sat/Sun mean is >= 1.3x the
  Mon–Fri mean, weekday reader when <= 0.77x, otherwise all-week. A fourth
  trait, *Variety*, is whole-library instead (author identity does not
  window meaningfully): the author-diversity index `1 - HHI`, where HHI is
  the sum over authors of each author's share of read time squared. Focused
  reader when `1 - HHI <= 0.45` (a few authors dominate), eclectic reader
  when `>= 0.72` (spread widely), otherwise varied; suppressed below three
  distinct authors. The three window traits require a minimum of reading to
  be meaningful; below it the whole profile is suppressed.
- **Records (all-time)**: whole-history bests, independent of the time
  window: longest single session (per the session definition above), biggest
  reading day by time, and most distinct pages read in a day. Each carries
  the date it happened. Shown only once there is any reading.
- **Forgotten books**: books with logged reading that are not finished
  (furthest position < 0.98) and whose most recent reading day is more than
  30 days before today, ordered most-neglected first. Files of one work
  collapse to one entry: the work is excluded if any file reached the end,
  and dated by its most recently read file (so reading one copy recently
  keeps the work off the list).
- **Period-over-period**: for a finite selected window, the current window's
  total time against the immediately preceding equal-length window, as a
  signed percentage. Omitted for the all-time view and whenever the previous
  window had no reading (so it never reports an infinite jump from zero).
- **Recap**: a whole-history composite, independent of the window: books
  finished (distinct finished works), total time, longest streak, session
  count, and the most-active calendar month. Because it is always all-time,
  it stays meaningful (and unchanged) when a shorter window is selected.
- **Completion rate**: finished works over started works, both counted
  distinct by title (files of one work count once), whole-history. A started
  work is any with logged reading.
- **Reading momentum (per book)**: a book's total time over the last 7 days
  against the 7 days before it, shown only when the book was read in the last
  7. Picking up at >= 1.15x (or from no prior reading), slowing down at
  <= 0.85x, otherwise holding steady. It describes current pace, so a book
  set aside simply shows nothing.
- **Estimate confidence**: how far to trust the time-left and finish-date
  estimate, from the number of distinct reading days behind the pace: high
  from 7 days, medium from 3, low below. Only present when there is an
  estimate.
- **Speed by hour of day**: reading speed resolved by local clock hour
  (0–23): for each hour, the distinct (book, page) pages read during it over
  the uncapped seconds spent in it, as pages/hour. Exactly the distinct-pages
  / uncapped-time rule of the speed trend, bucketed by hour of day instead of
  by date, and (like the trend) scoped to the selected window. An event's
  whole duration is attributed to its start hour (same rule as the
  when-do-I-read heatmap). Hours with no reading are empty. Library-wide (junk
  filter applied). It answers whether pace changes across the day, not just
  where the reading falls.
- **Cumulative reading curve**: a whole-history running total of reading
  time, one point per active day, from the first reading day onward, plotted
  as a monotonic series. Uses the default uncapped "Time read". Because it is
  an odometer that starts at zero, it is window-independent (a shorter window
  never rebases it) and library-wide (junk filter applied). It shows the
  long-run shape of a reading habit: pushes as steep stretches, lulls as
  plateaus.

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
   count, with annotation position markers drawn from a provided sidecar.
6. **Completions timeline.** Inferred read-throughs plotted on a timeline;
   books-finished-per-year/month rollups; per-completion cards (dates,
   calendar span, time, sessions, pages/hour).
7. **Reader profile.** The three synthesised traits (chronotype, session
   style, weekly rhythm) as a compact "reading personality" on the
   overview, each a plain-language label plus the number behind it. A
   narrative read of the same behaviour data the charts show.
8. **Speed by hour of day.** A 24-bar companion to the speed trend:
   pages/hour by clock hour, windowed. Where the when-do-I-read heatmap
   shows *when* reading happens, this shows *how fast*, so the two read
   together (e.g. a night owl who slows after 21:00).
9. **Cumulative reading curve.** A whole-history "odometer": total time
   read accumulated across every active day, one monotonic line. Pushes
   show up as steep stretches, lulls as plateaus; it only gets richer as
   history banks.

### Tier B — expected furniture (table stakes, done well)

7. **Year heatmap calendar.** GitHub-style day grid, quantized intensity
   levels (not continuous alpha), tooltips with time + pages + books.
8. **Streaks.** Current/longest day streak with date ranges.
9. **Library totals.** Total time, unique pages, books touched, active
   days, busiest day/month records. Windowed (30/90/365/all).
10. **Per-book stat cards.** A positional progress span bar (read regions
    filled, unlogged gaps empty, furthest-position marker, Finished marker
    when the end was reached), total time (both capped and uncapped,
    labelled), days reading, avg time/day, avg time/page, est. time left
    and finish date using KOReader's own math (capped avg_time) so the
    numbers match the device, the estimate's confidence, and a reading-
    momentum read when the book is currently active.
11. **Weekday and monthly distribution bars.** Weekday averages normalized
    by weekdays elapsed (not raw sums; KoInsight's mistake), monthly
    totals.
12. **Rollups.** Series composition and author affinity as whole-library
    lists (books, finished count, total time), ranked; each hidden when the
    library carries none of that metadata.
13. **Records and set-aside.** All-time bests (longest session, biggest day,
    most pages in a day, each dated) and a list of unfinished books untouched
    for over a month, most-neglected first. Both whole-history, from data
    already loaded.
14. **Recap and trend.** A whole-history recap card (books finished,
    completion rate, total time, longest streak, sessions, most-active
    month) and, on the windowed total time, a period-over-period change
    against the previous equal-length window.

### Tier C — deferred until the data or a dependency justifies it

- Highlight/note content browser. The `.sdr` sidecar parser and its `mlua`
  dependency landed with the finished-status reconciliation (v0.15.0); this
  now needs only the `annotations` array read out and a browser UI (counts
  only until then).
- Vocabulary-builder widgets (Brandon's `vocabulary_builder.sqlite3` is
  empty; revisit if the feature gets used).
- Language rollups (schema supports it; Brandon's library is
  single-language, so it would render dull — revisit if that changes).
  Series and author rollups have shipped (overview Series section and
  Author affinity), promoted out of this tier.
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

Rust 2024, plain GTK4, `rusqlite` (read-only opens only). Two-crate
workspace: `colophon-core` (ingestion/querying) and `colophon` (the GTK
shell). Charts are native cairo drawing (decided Phase 3; no charting
crate). Phase 6 (shipped as v2.0.0, 2026-07-10) removed libadwaita per
"Design language" below; GTK4 stays.

## Design language (Phase 6 — Hyprland-native; decisions locked 2026-07-09)

Colophon's shell targets a tiling, keyboard-first desktop. GTK4 stays;
libadwaita goes. The look is flat, square, and hard-edged: 1px borders,
no rounded corners, no shadows, denser spacing than GNOME HIG. The eight
palettes are unchanged; the app's own generated stylesheet (grown from
`theme.rs`) is the single styling authority once the adwaita sheet is gone.
The app must keep working under GNOME unchanged in behaviour; only the
look stops being GNOME's.

Decisions (Brandon, 2026-07-09):

- **Decoration posture: slim flat toolbar, no window buttons.** A thin,
  flat bar carries the title and the Import / Refresh / primary-menu
  buttons, separated from content by a 1px rule. No close / minimize /
  maximize buttons anywhere: Hyprland owns window management, and
  `Ctrl+Q` (already bound) plus the compositor's own binds cover closing
  on any desktop.
- **Layout: `GtkPaned` with a manual sidebar toggle.** The
  `AdwNavigationSplitView` adaptive collapse is replaced by a plain paned
  layout; a keybind (`F9`) shows/hides the library sidebar and the paned
  position persists in GSettings. The app never reshuffles its own layout
  on resize; narrow tiles are the user's call.
- **Follow-system dark/light survives via the portal.** Theme resolution
  reads `org.freedesktop.portal.Settings` (the
  `org.freedesktop.appearance` `color-scheme` key) directly over D-Bus
  through gio; no new dependency. Fixed themes force their own polarity
  exactly as today. Runtime note for non-GNOME sessions: a settings
  portal backend (`xdg-desktop-portal-hyprland` or `-gtk`) must be
  running for Follow-system to resolve; without one it degrades to the
  dark default, never a failure.

## Device auto-pull (decided 2026-07-09)

The read-access principle, restated to cover automation: **Colophon reads
only paths the user has explicitly given it**, and keeps them fresh when
they appear. Two kinds of path qualify: the remembered statistics-db
source path (GSettings `source-path`, set by every validated import) and
each attached sidecar's remembered origin path (recorded when the user
adds the file). Nothing is ever scanned or discovered; a book whose
sidecar was never attached stays sidecar-less until the user adds one.

Behaviour:

- **On startup** and **whenever a filesystem mount change makes the
  remembered source path readable** (the Kindle gets plugged in),
  Colophon automatically re-imports through the existing pipeline
  (staging → validate → promote), so a bad or half-written file can
  never clobber the good snapshot. The usual import toast reports it.
- **Attached sidecars ride along**: before the re-import, each cached
  sidecar with a readable origin is re-copied, re-verified by the same
  md5 join used at attach time. A failed or missing origin is skipped
  silently and the cache keeps its last good copy.
- Everything stays read-only toward the device: Colophon copies, never
  opens in place, never writes to a device path.
- Mount detection watches the kernel mount table (`gio`'s Unix mount
  monitor); no polling, no daemon, no new dependency.

## Non-goals (for now)

- Writing back to KOReader's database or config in any way.
- Cloud sync / multi-device merge logic beyond what's already baked into
  the KOReader data itself.
- Supporting reading-stats formats from other e-reader software (Kobo's
  native firmware stats, Moon+ Reader, etc.) — KOReader only, unless a
  strong case emerges later.

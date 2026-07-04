# Colophon — roadmap

## Phase 0 — Research (blocking, do this before writing UI code)

**This phase is mandatory before the spec can be locked or any real feature
work starts.** See `CLAUDE.md` for the full brief and `RESEARCH.md` for
findings so far.

- [x] Pin down KOReader's real `statistics.sqlite3` schema from source.
      Done 2026-07-03 straight from the device's own plugin source
      (`research/koreader-plugin-src/statistics.koplugin/main.lua`) — see
      `RESEARCH.md` §1 for the full schema, the `page_stat` rescaling view,
      and the font-size/pagination handling.
- [x] Get a real sample database. Copied from `/mnt/Kindle` 2026-07-03:
      `research/samples/statistics.sqlite3` (9 books, 695 page-stat rows)
      and `research/samples/vocabulary_builder.sqlite3` (empty — feature
      unused so far). Both gitignored.
- [x] Clone existing third-party KOReader stats tools for reference into
      `~/.gitrepos/.studyrepos/`: `KoInsight`, `KoShelf`, `Kodashboard`,
      `readingstreak.koplugin`. See `RESEARCH.md` §4.
- [x] Actually read those four tools' source for (a) what KOReader's own
      built-in stats screen already shows and (b) their full derived-metric
      catalogues. Done 2026-07-03: KOReader's UI surveyed from the plugin
      source (`RESEARCH.md` §4), all four tools catalogued (§5), converged
      conventions extracted (§6), underexplored territory mapped (§8).
- [x] Track down per-book `.sdr` sidecar metadata. Structure, location,
      and the `partial_md5_checksum` ↔ `book.md5` linkage fully documented
      from KoShelf/Kodashboard source (`RESEARCH.md` §7). One real sample
      file still to be copied next time the Kindle is mounted (wasn't,
      this pass); a nice-to-have, not blocking. Content stays Tier C.
- [x] Update `spec.md`'s widget/chart list from a brainstorm to a
      commitment. Done 2026-07-03: normative derived-metric definitions
      plus a three-tier catalogue (differentiators / table stakes /
      deferred), each item checked against KOReader's UI and the four
      tools.

## Phase 1 — Ingestion core (complete, v0.1.0, 2026-07-03)

- [x] Lock the real schema into `colophon-core` (the placeholder
      `table_names()` probe is gone; `db.rs` speaks the confirmed schema).
- [x] Typed query layer over the confirmed schema: books (md5-merged,
      junk-filter helper), raw events, and the `page_stat` view.
- [x] Derived-metric layer implementing `spec.md`'s normative definitions:
      sessions (300 s gap), day buckets + daily totals, streaks
      (today-or-yesterday rule), distinct-pages-read, capped and uncapped
      totals, interval-union coverage/progress, reading-speed series
      (day/week/month), completion detection (78 % / 20 % / 2 % with the
      restart-split heuristic). All pure functions, timezone-generic.
- [x] `db::snapshot()`: plain filesystem copy of the db (+ WAL/SHM
      sidecars), then the *copy* is checkpointed; no SQLite connection
      ever touches the source. `/mnt/Kindle` auto-detect deferred to the
      app layer (Phase 2+).
- [x] Test fixtures built programmatically (verbatim KOReader DDL,
      including the `numbers` table and `page_stat` view, in a std-only
      temp dir). 42 tests: 32 unit, 9 fixture-integration, plus a
      live-sample reconciliation test that runs against the gitignored
      Kindle copy when present and skips cleanly when not.

## Phase 2 — App shell (complete, v0.2.0, 2026-07-03)

- [x] Real `adw::ApplicationWindow` layout: `NavigationSplitView` sidebar
      (library) + content pane (Phase 3's detail slot), per-pane
      `ToolbarView`/`HeaderBar`, breakpoint collapse, toast overlay.
      Composite templates in the Viaduct house shape.
- [x] Database loading flow, stricter than planned: imports *always*
      snapshot (staging dir → validate → promote), so no user-chosen file
      is ever opened in place and no heuristic is needed; a bad pick can't
      clobber the good snapshot. Source path remembered in GSettings for
      Refresh (Ctrl+R / F5); startup auto-opens the canonical copy.
- [x] Schema-version guard: `adw::Banner` warning on unfamiliar
      `user_version`, never a refusal.
- [x] Ingestion → UI proven end to end: library list with title, author,
      total time, interval-union unique pages, relative last-open; data
      loads off the main thread (`gio::spawn_blocking`, no tokio).
- [x] Junk filter as a stateful window action in the primary menu
      (default on, 5-minute threshold), GSettings-persisted, live
      refilter on change.
- [x] Same-title/author grouping (the *Jingo* case): header row + inset
      member rows disambiguated by page count and short md5; display-only,
      data never merged.
- [x] Kanagawa Dragon theming: full Dragon sheet on dark via adw 1.6+ CSS
      variables, accent-only sheet on light (Dragon has no light variant),
      swapped live on the system preference; palette exported as consts
      for Phase 3 chart ramps.

Verified against the real sample: 53 tests (11 app-side), plus a headless
smoke run with screenshots (empty state, filtered/unfiltered library,
live junk-toggle via gsettings).

## Phase 3 — Widget variety

The catalogue is locked in `spec.md`; this phase builds it. Definitions
land in `spec.md` first, `colophon-core` grows the queries second, the
widget renders third.

UI shape (Brandon, 2026-07-03): the sidebar gets an **"All Books"** entry
above the book list. Selecting it shows library-wide widgets in the
content pane; selecting a book shows that book's stats. Both surfaces
grow widgets over the phase.

- [x] "All Books" overview surface (v0.3.0): totals tiles (time, pages,
      books, active days, busiest day), streak tiles with date ranges,
      year heatmap, weekday averages. Respects the junk filter (recomputes
      live on toggle). Time-window selector (30/90/365/all) still to come.
- [x] Per-book surface, first cut (v0.3.0): the Tier B stat card set
      (capped total labelled "as shown on device" with uncapped alongside,
      days reading, averages, sessions, KOReader-math time-left and
      finish-date estimates, interval-union progress bar). The Tier A
      per-book charts (velocity, page activity) are still open below.

Charting decision (first, it gates everything):

- [x] **Decided: custom cairo drawing on `GtkDrawingArea`, no charting
      crate.** Validated 2026-07-03 by building the year heatmap and the
      weekday bar chart as production widgets (`colophon/src/charts/`):
      both shapes came out clean, theme-reactive, and dependency-free
      (cairo toy text for short labels keeps pangocairo out too).
- [x] Shared chart scaffolding (`charts/mod.rs`): Kanagawa ramps for
      light/dark, KoShelf-style discrete intensity quantizer (explicitly
      not Kodashboard's continuous alpha), tooltip plumbing, dark-notify
      redraw wiring.

Tier A widgets (the differentiators; nobody ships these):

- [x] Reading-speed trend, library-wide (v0.4.0): pages/hour on a cairo
      line/area chart, daily buckets under ~10 weeks of history and
      weekly past that, nearest-point tooltips.
      - [x] Per-book overlay (v0.5.0): the book page shows its own trend
            with the library baseline muted behind it, same bucket so
            the series stay commensurable.
- [x] When-do-I-read heatmap (v0.4.0): weekday × hour grid over the
      whole history (`metrics::hourly_profile`, attribution by
      `start_time` like KOReader's calendar histograms), per-cell
      tooltips.
      - [x] Time-window filtering (v0.5.0, via the overview window
            selector).
- [x] Session analytics, first cut (v0.4.0): session-length histogram
      (<5m … >2h buckets) + caption records (count, median, longest with
      date).
      - [x] Sessions per active day + starts-by-hour chart (v0.5.0).
- [x] Per-page activity strip (v0.4.0): per-page total time and read
      count from the rescaled view, sqrt scaling with a 90th-percentile
      cap (KoShelf's numbers), pixel binning for long books, per-range
      tooltips. This is also the "did it drag in the middle" velocity
      view (Tier A #4's page axis).
      - [ ] Annotation count markers.
- [x] Book velocity, remaining piece (v0.5.0): read-through cards now
      carry pages/day over the calendar span (the page axis was already
      covered by the activity strip).
- [x] Per-completion cards (v0.4.0): inferred read-throughs on the book
      page (dates, time, sessions, pages/hour, coverage), hidden for
      books with none.
      - [ ] Overview completions timeline / books finished per
            year/month (waiting on data that actually contains finished
            books to design against).

Tier B widgets (expected furniture, done correctly):

- [x] Year heatmap calendar (v0.3.0): GitHub-style Monday-start grid,
      quantized levels, per-day tooltips (date, time, pages); the grid
      shrinks for young histories instead of rendering a year of blanks.
- [x] Streak tiles (v0.3.0): current/longest with date ranges
      (`metrics::streaks`).
- [x] Library totals windowing (v0.5.0): 30/90/365/all-time selector on
      the overview, scoping the totals tiles and the behaviour charts
      (hourly, speed, sessions, weekday) while streaks, the year
      heatmap, and monthly stay whole-history. Windows are calendar
      windows, not "last N days that had data" (Kodashboard's KPI bug;
      noted in RESEARCH §5.3).
- [x] Per-book stat cards with device parity (v0.3.0): capped total
      labelled "as shown on device", uncapped alongside, avg time/page
      and time-left/finish-date estimates using KOReader's own capped
      `avg_time` math so Colophon never contradicts the Kindle.
- [x] Weekday distribution (v0.3.0): averages normalized by weekdays
      elapsed (KoInsight's raw-sum skew is the anti-pattern).
- [x] Monthly distribution (v0.4.0): totals with empty months rendered,
      not skipped; January labels carry the year.

## Phase 4 — Polish & packaging

- [ ] Icon pass (replace the placeholder `logo.svg`).
- [ ] Performance pass on a realistic future db: the current sample is
      695 rows, but years of reading produce hundreds of thousands of
      `page_stat_data` rows and the `page_stat` view fans rows out up to
      1000× via the `numbers` join. Widgets must aggregate in SQL against
      the indexed `start_time`, not slurp the view into memory per
      render. Generate a synthetic multi-year fixture to measure.
- [ ] Meson wrapper + desktop entry + AppStream metainfo + Flatpak
      manifest, matching the Atrium/Conservatory/Viaduct pattern.
- [ ] `VERSION` → `1.0.0`.

## Phase 5 — Post-1.0 candidates (each needs its own go/no-go)

Ordered roughly by likelihood. None are commitments.

- [ ] `.sdr` highlight/note *content*: sandboxed-Lua parsing (KoShelf's
      `mlua` + `StdLib::NONE` pattern), joined via `partial_md5_checksum`
      = `book.md5`. Unlocks a highlight browser, annotation markers with
      text, and the sidecar `summary.status` user-declared finished flag
      to cross-check inferred completions. Gated on: a real sidecar
      sample from the Kindle, and the `mlua` dependency ask.
- [ ] Vocabulary-builder widgets ("words looked up per book", lookup
      timeline). Schema already documented (RESEARCH §2); Brandon's
      `vocabulary_builder.sqlite3` is empty, so parked until the feature
      sees real use.
- [ ] "Day starts at HH:MM" shift for night owls (KOReader and KoShelf
      both offer one; Colophon's day bucketing is already
      timezone-generic, so this is a small `TimeConfig`-style addition).
- [ ] Multi-device merge, only if a second KOReader device ever exists:
      KoInsight's `(md5, device, page, start_time)` upsert is the
      reference design; same-device re-imports are naturally idempotent
      thanks to the schema's `UNIQUE (id_book, page, start_time)`.
- [ ] Reference pages (manual canonical page count per book, KoInsight's
      feature): only if cross-layout comparison starts to matter beyond
      what interval-union progress already absorbs.
- [ ] Hardcover cross-reference: `hardcoversync_settings.lua` on the
      device implies a Hardcover plugin is in use; a read-only "also on
      Hardcover" link-out is the most this should ever be (local-first
      rule).

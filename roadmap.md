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
- [x] Get a real sample database. Copied from `/mnt/Kindle` 2026-07-03,
      refreshed 2026-07-05: `research/samples/statistics.sqlite3` (9 books,
      750 page-stat rows as of the refresh, latest event 2026-07-05; Royal
      Assassin now carries its 1 highlight) and
      `research/samples/vocabulary_builder.sqlite3` (still empty — feature
      unused so far). Both gitignored.
- [x] Clone existing third-party KOReader stats tools for reference into
      `~/.gitrepos/.studyrepos/`: `KoInsight`, `KoShelf`, `Kodashboard`,
      `readingstreak.koplugin`. See `RESEARCH.md` §4.
- [x] Actually read those four tools' source for (a) what KOReader's own
      built-in stats screen already shows and (b) their full derived-metric
      catalogues. Done 2026-07-03: KOReader's UI surveyed from the plugin
      source (`RESEARCH.md` §4), all four tools catalogued (§5), converged
      conventions extracted (§6), underexplored territory mapped (§8).
- [x] Fifth tool researched 2026-07-05: Tome (`bndct-devops/tome`, a
      self-hosted FastAPI+React server). Full dossier `RESEARCH.md` §5.5,
      ranked steal-list §8.1. The original four re-swept for residual value
      (§5.6). All five reference clones then deleted (re-clonable; upstreams
      in §9). Key delta: a word-count axis and a handful of cheap
      stats-DB-feasible cards (author affinity, personal records, finish
      estimates) — now phased below.
- [x] Track down per-book `.sdr` sidecar metadata. Structure, location,
      and the `partial_md5_checksum` ↔ `book.md5` linkage fully documented
      from KoShelf/Kodashboard source (`RESEARCH.md` §7). **Real sample
      copied 2026-07-05** from `/mnt/Kindle`: `research/samples/Royal
      Assassin - Robin Hobb (1705).sdr/metadata.epub.lua` (a finished book
      with a highlight: `summary.status="complete"`, `percent_finished=1`,
      `partial_md5_checksum` present) plus the Jingo dup-title sidecar. Both
      gitignored (real book data). Content stays Tier C.
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
- [x] Same-title/author grouping (two files of one work): header row + inset
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
      - [x] Annotation markers (v0.19.0). Highlights/notes/bookmarks from a
            provided `.sdr` sidecar, drawn on the activity strip at each
            annotation's fractional position through the book (its `pageno`
            over the sidecar's own page count, rescaled onto the current
            axis, never raw `pageno`; RESEARCH §7.1). Three-way
            bookmark/highlight/note classification in the parser; highlights
            and notes accent-coloured, bookmarks muted. Books without a
            provided sidecar show none. `colophon-core::sidecar::Annotation`.
- [x] Book velocity, remaining piece (v0.5.0): read-through cards now
      carry pages/day over the calendar span (the page axis was already
      covered by the activity strip).
- [x] Per-completion cards (v0.4.0): inferred read-throughs on the book
      page (dates, time, sessions, pages/hour, coverage), hidden for
      books with none.
      - [x] Overview completions timeline / books finished per
            year/month (v0.18.0). A "Finished books" section listing each
            finished work by finish date (detected read-through end, else
            last reading day so a sidecar-only finish still places), time
            per book, most recent first. `stats::finished_timeline`.

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

- [x] Icon pass. A colophon press-mark: a Kanagawa-Dragon gold "C" with a
      copper fleuron (the end-of-book flourish a colophon is) in its mouth,
      pure vector paths so it depends on no font. Ships as the scalable app
      icon, a monochrome symbolic variant, and `logo.svg`.
- [x] Performance pass on a realistic future db (v0.6.0). A deterministic
      synthetic fixture (200 books, four years, 222k `page_stat_data`
      rows) and an ignored measurement harness (`colophon-core/tests/
      perf.rs`) gave the baseline. Two findings drove the work: the
      `page_stat` view was materialized whole (367k fanned-out rows held
      in memory), and the overview re-ran its whole-history aggregation on
      every window toggle. Fixes: (a) the view is consumed as a per-page
      `GROUP BY` reduction (`StatsDb::page_totals`) plus a Rust rescale
      for the last page (`metrics::rescaled_last_page`), never the
      fanned-out rows, parity-locked against the old path — resident set
      27 MB → 19 MB; (b) the overview caches its window-independent
      aggregates (`stats::OverviewBase`) and recomputes only the windowed
      charts on a window toggle — narrowing a window 20 ms → 3 ms,
      all-time 44 ms → 23 ms. Timezone/DST math stayed in chrono (no SQL
      `localtime`), so the metric functions and their tests are untouched.
      Load time is unchanged (both paths compute the view once); the win
      is memory and per-interaction render cost.
- [x] Meson wrapper + desktop entry + AppStream metainfo + Flatpak
      manifest, matching the Atrium/Conservatory/Viaduct pattern. Top-level
      `meson.build` orchestrates `cargo build --release` and installs the
      GNOME-shaped layout (binary, gschema + compiled cache, `.desktop`,
      `.metainfo.xml`, hicolor icon); the gschema moved from `colophon/data`
      to a shared top-level `data/` (dev-run schema shim in `main.rs` and
      `build.rs` updated to match). `org.virinvictus.Colophon.json` builds
      it against the GNOME 49 runtime with a read-only host sandbox
      (`--filesystem=host:ro`, fitting the read-only ethos so Refresh can
      re-read a device path). Verified end to end: `meson setup/compile/
      install` to a temp prefix lays the tree out correctly, and
      `desktop-file-validate` + `appstreamcli validate` pass. The installed
      icon is still the placeholder `logo.svg` pending the icon pass.
- [x] `VERSION` → `1.0.0` (2026-07-05). Phases 4.5 and 4.6 all shipped, so
      the milestone is earned; the spec is fully built.

## Phase 4.5 — Make it a joy (Brandon's expansion, 2026-07-05)

Colophon reframed from "ship the spec" to "a cool, useful, evolving
reading-stats companion." Four features greenlit, sequenced; every new
widget's metric still lands in `spec.md` first. 1.0 waits until these are
in.

- [x] **Honest per-book progress** (v0.7.0). The progress bar was
      interval-union coverage shown as a left-anchored fraction, so a book
      read partly outside KOReader (Brandon's own case: jailbroke a Kindle
      mid-read, KOReader logged only ~29 %→100 %) looked half-done though
      it was finished. Now a positional span bar (`charts/span_bar.rs`,
      `stats::progress`, core `coverage_spans` + `furthest_position`) draws
      where reading was logged with a furthest-position marker, plus a
      Finished marker at furthest ≥ 0.98. The `.sdr` declared-finished flag
      will override the inference once sidecars are in scope.
- [x] **Themes** (v0.8.0). Eight palettes (Kanagawa Dragon/Wave/Lotus,
      Gruvbox Dark/Light, Nord, Rosé Pine, Solarized Light) plus a
      Follow-system mode. One `Theme` drives both the generated adwaita CSS
      and the chart colours; a Preferences dialog (Ctrl+comma) switches
      live. The two static CSS sheets are gone; new GSettings `theme` key.
- [x] **Mine today's data for new cards.** No new deps.
      - [x] Reader-profile (v0.9.0): "Reading personality" on the overview,
            three synthesised traits (chronotype, session style, weekly
            rhythm) classified from the existing hourly/session/weekday data.
            (v0.16.0: session style now classifies on a time-weighted typical
            session, not the plain median, so a pile of tiny device-tinkering
            sessions no longer mislabels a steady reader a "Sipper".)
      - [x] Series aggregation (v0.10.0): overview "Series" section grouping
            books by the `series` field ("Name #index" parsed, "N/A"
            skipped, files of one work deduped), with finished counts.
      - [x] Re-read detection (v0.10.0): "Pages revisited" per-book stat
            from `page_totals` (reads > 1).
      - [ ] Language breakdown deferred: Brandon's library is single-language
            ("en"), so it would render dull; revisit if that changes.
- [x] **Completions / year timeline** (v0.18.0). A "Finished books" section
      on the overview: each finished work by finish date, most recent first,
      with per-book time. Finish date is a detected read-through's end, else
      the last reading day (so a sidecar-only finish still places). Grows
      into a year-over-year record as Brandon finishes more.
      `stats::finished_timeline`.
- [x] **`.sdr` finished-flag reconciliation** (v0.15.0). Brandon chose the
      sandboxed `mlua` route over a stdlib parser. `colophon-core::sidecar`
      parses each `metadata.*.lua` in a locked-down Lua VM (`StdLib::NONE`,
      text-only, UTF-8-lossy) and joins `partial_md5_checksum` to
      `book.md5`; the declared `summary.status` becomes authoritative through
      the single `LibraryEntry::is_finished` used by every finished count and
      marker. The book page shows the device's status. Verified by a
      round-trip test that reconciles the real Royal Assassin sidecar
      (`status="complete"`) against the live stats DB. Reading the
      `annotations` array (highlight content, annotation markers) is the
      remaining sidecar work, now that the parser and dep are in place.
      - **v0.17.0 rework:** discovery changed from a device-folder scan (the
        original "KOReader library folder" setting, now removed) to
        per-book, user-provided files, matching how the stats DB is imported.
        Each book's page has an "Add file" action; the sidecar is verified
        against the book's md5, copied into an app-owned cache
        (`<data>/sidecars/<md5>.lua`), and looked up per book on load.
        General principle: Colophon never reads the device; any stat needing
        a file the user has not provided stays hidden until they add it.

**Data-provisioning errands:** the `.sdr` sidecar sample and a fresh
`statistics.sqlite3` were pulled 2026-07-05 (750 events). Standing errand,
not urgent: re-import `statistics.sqlite3` after finishing each book so the
finished-books features (completions timeline, year rollups,
estimate-accuracy) grow richer over time.

## Phase 4.6 — Tome-sourced cards (research 2026-07-05, `RESEARCH.md` §8.1)

The Tome dossier surfaced a cluster of cards that are **feasible from the
`statistics.sqlite3` Colophon already reads**: no new deps, no library-file
access, no sidecars. They fit the "make it a joy" reframing (more useful
cards from data already in hand) and slot ahead of 1.0. Each metric's
normative definition lands in `spec.md` first, per the standing rule; the
pure aggregation goes in `colophon-core`/`stats.rs`, the widget renders
third. Ordered cheapest-first.

- [x] **Author affinity** (v0.11.0). Reading time and finished-book count
      rolled up per author (top 10), an overview "Authors" card ranked by
      time. Whole-library like Series; files of one work count once.
      `stats::author_breakdown`.
- [x] **Personal records** (v0.12.0). Longest single session, biggest
      reading day by time, most pages in a day, each with its date, as an
      all-time "Records" card. Whole-history (does not move with the
      window). `stats::personal_records`.
- [x] **Forgotten books** (v0.12.0). A "Set aside" rail of books with
      logged reading whose furthest read is not "finished" and whose last
      reading-day is > 30 days ago, most-neglected first. Files of one work
      collapse; reading either copy recently keeps the work off the list.
      `stats::forgotten_books`.
- [x] **Period-over-period delta** (v0.13.0). The total-time tile shows the
      current window against the immediately preceding equal-length window
      (% change with an up/down/flat arrow), returning nothing rather than a
      fake ∞% when the prior window is empty. `stats::PeriodDelta`.
- [x] **Recap card** (v0.13.0). A whole-history composite (books finished,
      total time, longest streak, sessions, most-active month) assembled from
      numbers the overview already computes. Always all-time, so it stays put
      when a shorter window is selected; Tome's top-genre facet dropped
      (needs catalogue metadata Colophon lacks). `stats::Recap`. (Renamed
      from "year-in-review": the data is single-year for now, so a whole-
      history recap is the honest framing; a per-year variant can come with
      the completions timeline once the data spans years.)
- [x] **Per-book finish estimate + reading momentum** (v0.14.0). The
      existing KOReader-parity time-left/finish estimate now carries a
      high/medium/low confidence from the number of reading days behind it,
      and the book page gains a momentum read (last 7 days vs the prior 7:
      picking up / slowing down / holding steady), shown only while the book
      is currently active. `stats::reading_momentum`, `BookDetail`.
- [x] **Completion / abandonment rate** (v0.14.0). A completion figure on
      the Recap card: finished works over started works (both distinct by
      title). "Finished" is the inferred furthest-position heuristic, not
      user-declared, until §4.5 sidecar reconciliation lands.
      `stats::Recap::completion_rate`.
- [x] **HHI Variety trait for the reading-personality card** (v0.11.0). A
      fourth synthesised trait (focused ↔ eclectic) from an author-diversity
      Herfindahl index (`1 − HHI` over read time), shown once the library
      holds three or more distinct authors. Whole-library (author identity
      does not window). The book-type half stays blocked on catalogue
      metadata. `stats::variety_trait`.

## Phase 5 — Post-1.0 candidates (each needs its own go/no-go)

Ordered roughly by likelihood. None are commitments.

- [ ] **Word-count axis (the big one; needs a scope decision).** Tome's
      largest capability delta (`RESEARCH.md` §5.5): true words-per-minute
      (pagination-independent, unlike Colophon's pages/hour), lifetime
      words-read, and a book-length distribution — and it unlocks two more
      reading-personality axes (Length, Pace). **Off Colophon's stats-DB-only
      contract:** word counts come from the EPUB files, not KOReader, so this
      means (a) reaching the library files at all, and (b) an EPUB word
      counter — either a new dep (`ebooklib`) or a stdlib `zipfile` + regex
      path (Tome falls back to exactly that). Both are deliberate go/no-go
      calls, not slip-ins: it changes what Colophon reads. High value if the
      answer is yes.
- [ ] Reading goals ({books|minutes|pages} per {day|week|month|year}, with a
      prorated on-pace line), stats-DB-feasible. **Deliberately declined at
      research time** (Colophon is a stats *viewer*, not a tracker); listed
      only so the decision is on the record. Re-raise on request.
- [ ] `.sdr` highlight/note *content*: sandboxed-Lua parsing (KoShelf's
      `mlua` + `StdLib::NONE` pattern), joined via `partial_md5_checksum`
      = `book.md5`. Unlocks a highlight browser, annotation markers with
      text, and the sidecar `summary.status` user-declared finished flag
      to cross-check inferred completions. Sidecar sample now in hand
      (2026-07-05); remaining gate is just the `mlua` dependency ask.
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

## Phase 6 — Hyprland-leaning design (post-1.0)

Brandon moved his desktop from GNOME Shell to Hyprland (a Wayland tiling
compositor) in 2026-07. Colophon stays GTK4/libadwaita; nothing here may
regress GNOME Shell behavior, so every item is additive polish for a
tiling-WM, keyboard-first, portal-mediated session, not a rewrite. Same
discipline as Phase 5: each item is its own go/no-go, ordered roughly
cheapest-first.

- [ ] **Tiling geometry audit for the two fixed-width charts.**
      `HourHeatmap` requests a hard content width of `LEFT + (CELL_W +
      GAP) * 24` (34 + 25 * 24 = 634px, `charts/hour_heatmap.rs:41-42`),
      and `YearHeatmap` grows with weeks of history (`LEFT + (CELL + GAP)
      * weeks`, `charts/heatmap.rs:119-120`). Both already sit inside a
      `GtkScrolledWindow` with `hscrollbar-policy: automatic` so they
      scroll instead of forcing the window wide
      (`overview_page.ui:146-148`, `:166-168`), while the outer page
      scroller stays `hscrollbar-policy: never` (`overview_page.ui:6`) so
      only the two heat grids can go wide. Confirm at a genuine
      quarter-monitor tile (roughly 480px) that: (a) the horizontal
      scrollbar is discoverable without a mouse hover, since Hyprland
      users are more likely to be keyboard/touchpad-only; (b) cell size
      and per-cell tooltip legibility survive being clipped to that
      width; (c) `book_page.ui`'s activity strip and speed chart (which
      only set `content_height`, not `content_width`) keep reflowing
      cleanly at the same width instead of relying on the scroller.
- [ ] **Width-adaptive label thinning instead of a hardcoded modulus.**
      The session-starts bar chart currently hides all but every sixth
      hour label at data-prep time (`hour % 6 == 0`,
      `overview_page.rs:400-406`), a fixed choice independent of the
      widget's actual allocated width. `BarChart::draw` already receives
      the live width and computes slot width from it
      (`charts/bar.rs`), so this is fixable inside the draw callback:
      measure each candidate label with `text_width`
      (`charts/mod.rs:107-115`) against the slot width and thin (or
      widen) the label set to what actually fits at the tile's current
      size, rather than a fixed skip-every-N baked in upstream. Apply the
      same audit to the weekday/monthly bar labels and the hour-heatmap's
      hour-of-day column headers, none of which currently vary their
      density with allocation.
- [ ] **Minimum-height audit on short tiles.** A quarter-monitor tile can
      be short as well as narrow. Check the fixed chart heights
      (`LineChart` 150px, `BarChart` 150px, `PageActivityStrip` 96px,
      `SpanBar` 26px; the `set_content_height` calls in `charts/*.rs`)
      against the overview page's full vertical stack (totals tiles,
      streaks, both heatmaps, weekday/monthly bars, speed trend, session
      charts, records, recap, finished-books rows) and confirm the outer
      `GtkScrolledWindow` (`overview_page.ui:5`) reliably takes and keeps
      keyboard-scroll focus, so a short tile is a scroll away rather than
      a dead end.
- [ ] **Fractional-scaling hairline check.** Chart rules and strokes are
      drawn at fixed cairo widths: the baseline rule fill in
      `charts/bar.rs:120` (`cr.rectangle(0.0, baseline, w, 1.0)`) and the
      trend-line strokes in `charts/line.rs:200` (1.5px muted / 2.0px
      active). Audit these under 1.25x and 1.5x fractional scale, the
      classic Wayland blurry-or-misaligned-hairline bug, and snap to the
      device pixel grid where cairo allows it instead of assuming the 1x
      integer-scale GNOME default.
- [ ] **Keyboard-first navigation pass.** Today's full accelerator table
      is four entries: `Ctrl+O` import, `Ctrl+R` / `F5` refresh,
      `Ctrl+comma` preferences, `Ctrl+Q` quit (`ui/actions.rs:91-94`).
      The library list is already a `selection-mode: browse`
      `GtkListBox` (`library_view.ui`), so arrow-key row navigation is
      free, but there is no `GtkShortcutsWindow` documenting any of this,
      and no explicit keyboard path back from a book's detail pane to the
      library beyond `AdwNavigationSplitView`'s built-in collapsed-mode
      back button. Add a shortcuts window (`Ctrl+question` / `F1`), an
      explicit `Escape` binding back to the library when the split view
      is collapsed, and confirm `Tab`/arrow focus order through the
      overview's `GtkFlowBox` tile grids (`tiles` at
      `overview_page.rs:28`, `profile_tiles` at `:32`, `record_tiles` at
      `:62`, `recap_tiles` at `:66`) is sane when driven purely from the
      keyboard, since a tiling-WM user is more likely to be mouse-light
      than a GNOME Shell user.
- [ ] **Reconfirm dark/light stays portal-clean.** Already correct and
      worth protecting: theme resolution goes through
      `adw::StyleManager` (`is_dark`, `connect_dark_notify`,
      `theme.rs:321-345`), never a direct `org.gnome.desktop.interface`
      GSettings read. Under Hyprland this only resolves correctly if a
      settings portal implementation (`xdg-desktop-portal-hyprland` or
      `-gtk`) is running and answering the color-scheme query; note that
      as a documented runtime dependency for a non-GNOME session rather
      than something Colophon needs to code around, and spot-check the
      eight-theme Follow-system mode (`theme.rs`'s `resolve`) actually
      flips live outside a GNOME session.
- [ ] **Font family stays generic (no change needed, keep it that way).**
      `charts/mod.rs:94-115`'s `draw_text` and `text_width` already
      select `"sans-serif"` through cairo's toy font API, not a
      GNOME-specific family like Cantarell. This item is a guardrail, not
      a fix: don't let a future chart hardcode a GNOME default font, and
      don't let the icon or headerbar styling pick one up either.
- [ ] **App-id / window-rule stability audit (already sound, verify and
      document).** `APP_ID` in `main.rs:18` (`org.virinvictus.Colophon`)
      matches the `.desktop` basename
      (`data/org.virinvictus.Colophon.desktop`) and the Flatpak `app-id`
      (`org.virinvictus.Colophon.json:2`), and the native meson-built
      binary sets the same `application_id` at runtime (`main.rs:37`), so
      a Hyprland `windowrulev2` matched on app-id should already be
      stable across the Flatpak and native-build paths alike. Confirm
      this holds for both build paths and record it, since there is
      nothing else to fix here.
- [ ] **Narrow the Flatpak filesystem grant toward portal-only access.**
      The manifest currently requests `--filesystem=host:ro`
      (`org.virinvictus.Colophon.json`), a blanket read grant that
      predates a fully portal-mediated import flow. Import already goes
      through `gtk::FileDialog` (`ui/window.rs:173-176`), which is
      portal-backed when sandboxed and grants access to the chosen file
      through the document portal without a standing filesystem
      permission. Evaluate dropping `--filesystem=host:ro` now that both
      the statistics-db import and the per-book `.sdr` sidecar attachment
      go through user-driven file pickers. This is a sandboxing
      improvement in its own right, not a Hyprland-only fix, since it
      reduces reliance on a broad host-trust grant either desktop offers;
      it needs its own go/no-go against the `/mnt/Kindle`-style
      auto-detect path used by Refresh, which does read the filesystem
      directly rather than through a picker.
- [ ] **CSD posture under a hidden-buttons layout.** Colophon's
      headerbars are plain `AdwHeaderBar` with no custom
      `decoration-layout` (`ui/window.ui:38`, `:126`), so they inherit
      whatever the session's GTK settings provide. Several Hyprland
      setups run with server-side decorations disabled and
      `gtk-decoration-layout` configured to hide the minimize/maximize
      buttons, or all three. Test the overview and book-page headerbars
      with `GTK_DECORATION_LAYOUT` set to hide some or all window
      buttons, and confirm header content (title, refresh/import/
      preferences buttons) doesn't rely on button spacing for layout
      balance, and that the window stays fully closable via `Ctrl+Q` when
      no titlebar close button is drawn.

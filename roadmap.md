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
      - [ ] Annotation count markers. **Unblocked 2026-07-05** (sidecar
            sample in hand). Approach settled in `RESEARCH.md` §7.1:
            annotations live in the `.sdr` sidecar and drift with pagination
            exactly like page stats, so markers reuse the strip's existing
            page-interval rescale (never the raw `pageno`); three-way
            bookmark/highlight/note classification. Rides on the §7.2 /
            Phase 4.5 sidecar-parsing work.
- [x] Book velocity, remaining piece (v0.5.0): read-through cards now
      carry pages/day over the calendar span (the page axis was already
      covered by the activity strip).
- [x] Per-completion cards (v0.4.0): inferred read-throughs on the book
      page (dates, time, sessions, pages/hour, coverage), hidden for
      books with none.
      - [ ] Overview completions timeline / books finished per
            year/month. **Data now exists** (2026-07-05: Royal Assassin
            reads as complete). Still thin (one finished book); see Phase
            4.5 for the sequenced build.

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
- [ ] `VERSION` → `1.0.0` (deferred behind the expansion below).

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
      - [x] Series aggregation (v0.10.0): overview "Series" section grouping
            books by the `series` field ("Name #index" parsed, "N/A"
            skipped, files of one work deduped), with finished counts.
      - [x] Re-read detection (v0.10.0): "Pages revisited" per-book stat
            from `page_totals` (reads > 1).
      - [ ] Language breakdown deferred: Brandon's library is single-language
            ("en"), so it would render dull; revisit if that changes.
- [ ] **Completions / year timeline.** Books-per-month/year and a
      completions timeline, unblocked now the data contains a finished book;
      grows as Brandon re-imports after finishing each book.
- [x] **`.sdr` finished-flag reconciliation** (v0.15.0). Brandon chose the
      sandboxed `mlua` route over a stdlib parser. `colophon-core::sidecar`
      parses each `metadata.*.lua` in a locked-down Lua VM (`StdLib::NONE`,
      text-only, UTF-8-lossy) and joins `partial_md5_checksum` to
      `book.md5`; an optional read-only "KOReader library folder" set in
      Preferences (GSettings `library-dir`) is scanned on load, and the
      declared `summary.status` becomes authoritative through the single
      `LibraryEntry::is_finished` used by every finished count and marker.
      The book page shows the device's status. Opportunistic: an unset or
      unmounted folder falls back to inference. Verified by a round-trip
      test that reconciles the real Royal Assassin sidecar
      (`status="complete"`) against the live stats DB. Reading the
      `annotations` array (highlight content, annotation markers) is the
      remaining sidecar work, now that the parser and dep are in place.

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

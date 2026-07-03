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
- [ ] Actually read those four tools' source for (a) what KOReader's own
      built-in stats screen already shows and (b) their full derived-metric
      catalogues, to sanity-check/extend the widget list in `spec.md`. Not
      done yet — cloning them is not the same as having read them.
- [ ] Track down per-book `.sdr` sidecar metadata (highlight/note *content*,
      as opposed to the `book.notes`/`book.highlights` counts already
      confirmed) if highlight content ever becomes in-scope.
- [ ] Once the above is done, update `spec.md`'s widget/chart list from a
      brainstorm to something grounded in both the schema and what's
      already been tried, then move to Phase 1.

## Phase 1 — Ingestion core

- [ ] Lock the real schema into `colophon-core` (replace the placeholder
      `table_names()` probe in `colophon-core/src/lib.rs`).
- [ ] Typed query layer: per-book summaries, per-session records, derived
      aggregates (streaks, pace, etc.) as plain Rust structs.
- [ ] A way to get a copy of the live db onto disk (manual path for now;
      revisit `/mnt/Kindle` SSHFS auto-detect later if it's worth it).
- [ ] Test fixtures: a handful of representative sample databases checked
      into the repo (synthetic or scrubbed) so tests don't depend on
      Brandon's live device.

## Phase 2 — App shell

- [ ] Real `adw::ApplicationWindow` layout (nav/sidebar + content area,
      matching the Atrium/Conservatory/Viaduct shape) replacing the current
      placeholder `StatusPage`.
- [ ] Load a chosen db file, show *something* real from it (even a plain
      list) to prove the ingestion → UI path end to end.
- [ ] Kanagawa Dragon theming pass.

## Phase 3 — Widget variety

- [ ] Build out the widget/chart catalogue validated in Phase 0, one at a
      time, each as an independent, reusable widget.
- [ ] Decide the charting approach (cairo direct vs. a Rust charting crate)
      once real chart shapes are known.

## Phase 4 — Polish & packaging

- [ ] Icon pass (replace the placeholder `logo.svg`).
- [ ] Meson wrapper + desktop entry + AppStream metainfo + Flatpak manifest,
      matching the Atrium/Conservatory/Viaduct pattern.
- [ ] `VERSION` → `1.0.0`.

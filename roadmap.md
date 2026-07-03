# Colophon — roadmap

## Phase 0 — Research (blocking, do this before writing UI code)

**This phase is mandatory before the spec can be locked or any real feature
work starts.** See `CLAUDE.md` for the full brief. Short version:

- [ ] Pin down KOReader's real `statistics.sqlite3` schema from source
      (`plugins/statistics.koplugin/` in `koreader/koreader` upstream), not
      assumption — table names, columns, how page counts/font-size changes
      are handled, timestamp format, migration history.
- [ ] Get a real sample database. `/mnt/Kindle` is available via SSHFS; copy
      the live file out (read-only) rather than guessing schema from docs
      alone.
- [ ] Survey what KOReader's own built-in stats screen already shows, so
      widgets don't just duplicate it.
- [ ] Survey existing third-party KOReader stats tools/dashboards for ideas
      worth stealing or gaps worth filling.
- [ ] Check for other on-device data worth mining (highlights/notes DB,
      vocabulary builder DB, per-book `.sdr` sidecars).
- [ ] Update `spec.md` with confirmed schema + a real (not brainstormed)
      widget/chart list, then this roadmap's later phases.

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

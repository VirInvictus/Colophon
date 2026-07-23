# CLAUDE.md — Colophon

Per-project guidance. Overrides `~/.claude/CLAUDE.md` where the two
conflict; read that file first for the general house style (portfolio doc
set, git habits, prose style, etc.) — this file only covers what's specific
to Colophon.

## What this project is

A native GTK4 desktop app (no libadwaita since v2.0.0) that turns
KOReader's reading statistics into attractive, varied graphs and widgets. The explicit reason
it exists: every KOReader stats tool Brandon has found is a web dashboard or
a self-hosted Docker instance, and he doesn't want that. See `README.md` and
`spec.md`.

## Where this stands right now (2026-07-16, v2.1.0)

**Shipped 2.1.** Phases 0 through 4.6 plus Phase 7 (device auto-pull,
v1.1.0) and Phase 6 (the de-adwaita migration, v2.0.0) are complete. Two
releases followed the migration: **v2.0.1** scoped the focus ring to
discrete interactive controls (`button`/`entry`/`switch`/`scale`
`:focus-visible`), ending the accent flash a bare modifier press used to
fire across every widget in the focus chain; **v2.1.0** added the
speed-by-hour chart and the cumulative reading curve. The
spec is fully built and the app is the portfolio's de-adwaita pilot, the
template for Atrium/Conservatory/Viaduct/Framework. Scaffolding was
Sonnet's, everything since is Fable's. The Phase 6e polish tail (tiling
geometry audit, draw-time label thinning, keyboard pass, shortcuts window)
is the only Phase 6 remainder. Phase 5 is the post-1.0 candidate list, and
each item needs its own go/no-go; the big open one is a word-count axis,
which is off the stats-DB-only contract because it means reading the
library EPUB files. Don't start any of it without a decision.

Architecture worth knowing before you touch code:

- **`colophon-core`** is the read-only ingestion + pure derived-metric layer:
  the typed query layer over the confirmed KOReader schema (md5-merged books,
  raw events, the rescaled `page_stat` view consumed as a per-page `GROUP BY`
  reduction rather than fanned-out rows), WAL-safe `snapshot()` (the source
  db is never opened in place), the metric functions (sessions, daily totals,
  streaks, interval-union coverage, capped/uncapped totals, speed series,
  completion detection), and `sidecar` (sandboxed `mlua`, `StdLib::NONE`,
  joining `partial_md5_checksum` → `book.md5`).
- **`colophon`** is the plain-GTK4 app: a GtkPaned shell with one flat
  headerbar and an F9 sidebar toggle (Viaduct-style composite templates,
  GSettings, `gio::spawn_blocking` for db work, no tokio). Sidebar "All
  Books" overview + per-book detail, both respecting the junk filter.
  Overview aggregates live in `src/stats.rs` (pure, tested), split into
  `OverviewBase` (window-independent, cached) and the windowed charts so a
  window toggle stays cheap. Charts are custom cairo on `GtkDrawingArea`
  (`src/charts/`), no charting crate. Eight themes drive both the
  app-owned generated stylesheet and the chart colours from one `Theme`;
  the provider registers just above USER priority so a global
  ~/.config/gtk-4.0/gtk.css skin can't half-override the in-app palettes.
  Follow-system dark/light reads org.freedesktop.portal.Settings over gio
  D-Bus (dark default when no portal answers); small owned widgets replace
  the adw ones (`ui/clamp.rs`, `ui/rows.rs`, the toast/banner revealers in
  `window.ui`).
- **Packaging**: Meson wrapper + `.desktop` + AppStream metainfo + Flatpak
  (`org.virinvictus.Colophon.json`, GNOME 50, `--filesystem=host:ro`).

Standing rules that still bind post-1.0: every new widget's metric lands in
`spec.md` first; ask before adding any dependency; Colophon reads only
paths the user has explicitly given it (the remembered stats-db source and
each attached sidecar's remembered origin; auto-pull keeps those fresh on
mount, see spec "Device auto-pull"), never scans or discovers device files,
and any stat needing a file the user has not provided stays hidden until
they add it.

Real sample databases and `.sdr` sidecars from Brandon's own Kindle live at
`research/samples/` (**gitignored, never commit them**); the on-device plugin
Lua source is checked into `research/koreader-plugin-src/`. Standing errand:
re-import `statistics.sqlite3` after finishing each book so the completions
timeline and estimate-accuracy data grow richer over time.

## Spec discipline

`spec.md`'s derived-metric definitions are normative: if a widget shows a
number KOReader also shows, it must use KOReader's math (capped totals for
its "time spent reading" and estimates) so the app never disagrees with
the device. New metrics must be defined in `spec.md` before they're built.

## Hard constraints (from the top-level house style, restated because they
matter a lot here)

- **Read-only, always.** Colophon must never open KOReader's live database
  file in place, and must never write anything to a path KOReader owns.
  Always operate on a copy. This is non-negotiable — it's someone's actual
  reading history and device state.
- **No Docker, no web UI, no cloud/self-hosted service.** That's the entire
  reason this project exists instead of using what's already out there.
- **Local-first.** No accounts, no telemetry, fully offline-capable.

## Stack

Rust 2024, plain GTK4 (v4_16; libadwaita removed in v2.0.0), `rusqlite`
(read-only opens only). Two-crate workspace (`colophon-core`, `colophon`),
matching the shape of `Viaduct`/`Conservatory` rather than Atrium's larger
seven-crate split — this project doesn't need that much separation yet.
Charts are hand-drawn cairo (decided Phase 3; no charting crate).

## Naming

"Colophon": a printer's mark historically placed at the end of a book
recording production details (press, date, edition). The metaphor: this app
is the technical production record of *how a book was read*, not printed.
Fits the workspace's book-craft naming vein (`Bindery`, `oceanstrip`) without
colliding with anything else in `~/.gitrepos/`.

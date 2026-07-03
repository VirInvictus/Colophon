# Patchnotes

## v0.2.0 — 2026-07-03

Phase 2: the real app shell. The placeholder window is gone; Colophon now
opens, imports, and shows a library.

The window is a NavigationSplitView (library sidebar, detail pane
reserved for Phase 3) built from composite templates in the Viaduct house
shape. Imports always snapshot: the chosen file is copied to a staging
dir, validated, and only then promoted to the app's canonical copy, so no
user-chosen database is ever opened in place and a bad pick can't destroy
a good snapshot. Refresh (Ctrl+R/F5) re-imports from the remembered
source. An adw::Banner warns on unfamiliar schema versions instead of
refusing. The library list shows total time, interval-union unique pages,
and relative last-open per book; same-title/author copies group under a
header row without being merged in data; a persisted junk filter (default
on, 5 minutes) hides plugin READMEs and other accidental "books".
Kanagawa Dragon theming applies in full on dark and as accents on light,
following the system preference live. All database work runs off the main
thread via gio::spawn_blocking; no new dependencies.

53 tests across the workspace (11 new app-side: formatting, grouping,
staged-import protocol), plus a headless screenshot smoke run against the
real sample data.

## v0.1.0 — 2026-07-03

Phase 0 research completed and Phase 1 ingestion core shipped.

Research: KOReader's built-in stats UI surveyed from the on-device plugin
source; KoInsight, KoShelf, Kodashboard, and readingstreak.koplugin read in
depth. Findings, converged conventions (session gap, streak rule, md5
identity, capped/uncapped totals), and the underexplored territory Colophon
targets are all in `RESEARCH.md`. `spec.md` is locked for v1: normative
derived-metric definitions plus a three-tier widget catalogue. The `.sdr`
sidecar format is documented (Tier C until a sample is copied).

Code: `colophon-core` grew its real query layer (read-only opens only;
md5-merged books; raw `page_stat_data` events and the rescaled `page_stat`
view; WAL-safe `snapshot()` that never opens the source) and a pure
derived-metric layer (sessions, daily totals, streaks, interval-union
coverage, KOReader-parity capped totals, reading-speed series, completion
detection). 42 tests, including a schema-verbatim synthetic fixture builder
and a live-sample reconciliation test that skips when the gitignored Kindle
copy is absent.

## v0.0.1 — 2026-07-03

Initial scaffolding. Cargo workspace (`colophon-core` + `colophon`), empty
GTK4/libadwaita shell window, standard portfolio doc set. No KOReader data
has been read yet — Phase 0 research is the next step, not implementation.

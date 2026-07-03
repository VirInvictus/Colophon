# Patchnotes

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

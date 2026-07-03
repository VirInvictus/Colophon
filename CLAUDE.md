# CLAUDE.md — Colophon

Per-project guidance. Overrides `~/.claude/CLAUDE.md` where the two
conflict; read that file first for the general house style (portfolio doc
set, git habits, prose style, etc.) — this file only covers what's specific
to Colophon.

## What this project is

A native GTK4/libadwaita desktop app that turns KOReader's reading
statistics into attractive, varied graphs and widgets. The explicit reason
it exists: every KOReader stats tool Brandon has found is a web dashboard or
a self-hosted Docker instance, and he doesn't want that. See `README.md` and
`spec.md`.

## Where this stands right now (2026-07-03)

This repo was scaffolded by Sonnet, then had a first Phase 0 research pass
done the same day (also Sonnet, light-touch — schema confirmation and
fetching reference material, not a deep feature survey). **The brunt of
this project's design and build work still happens in Fable, not Sonnet.**

What's done:
- The Cargo workspace builds clean (empty GTK shell window, placeholder
  `StatsDb` in `colophon-core`).
- The real `statistics.sqlite3` schema is confirmed from source (not
  guessed) — see `RESEARCH.md` §1. Real sample databases from Brandon's own
  Kindle are at `research/samples/` (gitignored, don't commit them), and the
  actual on-device plugin Lua source is checked into
  `research/koreader-plugin-src/` for reference.
- Four third-party KOReader stats tools are cloned for study into
  `~/.gitrepos/.studyrepos/` (`KoInsight`, `KoShelf`, `Kodashboard`,
  `readingstreak.koplugin`) — see `RESEARCH.md` §4. **They've only been
  cloned, not read in depth yet.**

What's still open (read `RESEARCH.md` §5 and `roadmap.md` Phase 0 before
doing anything else):
- Actually reading those four tools' source for their metric/chart
  catalogues and for what KOReader's own built-in stats screen shows, so
  Colophon's widget set adds value instead of duplicating either.
- Locating per-book `.sdr` sidecar metadata if highlight/note *content*
  (not just the counts already confirmed) ever becomes in-scope.
- `spec.md`'s widget/chart list is still a brainstorm, not validated
  against either KOReader's own UI or the third-party tools' feature sets.

## Mandatory next step before Phase 1 (ingestion core)

Do not start building `colophon-core`'s real query layer against the
confirmed schema until the remaining Phase 0 items above are done. The
point of researching first is to avoid spending build effort re-inventing
widgets those four existing tools (or KOReader itself) already cover well —
read them, note what's genuinely underexplored, *then* lock `spec.md`'s
widget list and move to Phase 1.

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

Rust 2024, GTK4 / libadwaita, `rusqlite` (read-only opens only). Two-crate
workspace (`colophon-core`, `colophon`), matching the shape of
`Viaduct`/`Conservatory` rather than Atrium's larger seven-crate split —
this project doesn't need that much separation yet. Charting library choice
is explicitly deferred to after Phase 0 (see `spec.md` non-goals/open
questions) — don't pick one prematurely.

## Naming

"Colophon": a printer's mark historically placed at the end of a book
recording production details (press, date, edition). The metaphor: this app
is the technical production record of *how a book was read*, not printed.
Fits the workspace's book-craft naming vein (`Bindery`, `oceanstrip`) without
colliding with anything else in `~/.gitrepos/`.

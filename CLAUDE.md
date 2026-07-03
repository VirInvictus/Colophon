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

This repo was scaffolded by Sonnet in a single session with **no research
done yet** — the Cargo workspace, doc set, and empty GTK window exist purely
to have a real toolchain and git history in place. `spec.md` is explicitly a
draft with an "open questions" section instead of a real data model, and the
`colophon-core` crate's `StatsDb` is a placeholder that only lists table
names — it does not know the real schema.

**The brunt of this project's design and build work happens in Fable, not
Sonnet.** Sonnet's job was scaffolding only.

## Mandatory first step: deep research (Phase 0)

Before writing any real feature code or locking `spec.md`, do a genuinely
deep research pass — this is a hard requirement from Brandon, not a nice-to-
have. Do not skip to building widgets against assumed columns. Specifically:

1. **Get the real schema, from source, not memory or guesswork.** KOReader
   is open source (`koreader/koreader` on GitHub). The statistics plugin
   lives at `plugins/statistics.koplugin/` — read `main.lua` and any
   schema/migration code directly. Confirm real table and column names for:
   - the book-level table (title, authors, series, language, hash/md5,
     total pages, total read pages, total read time, last-read timestamp)
   - the per-session/per-page table (timestamp, duration, page number,
     which book, total page count *at the time*, since KOReader's page
     count is not stable across font-size changes — this is a known gotcha,
     confirm exactly how it's handled)
   - anything else in the same database (highlights/notes may or may not
     live here vs. a separate per-book `.sdr` Lua metadata sidecar)
   - schema version history if the format has changed over time
   - timestamp format/timezone handling, and how re-reads and multi-device
     sync show up in the data
2. **Get a real sample database to test against.** `/mnt/Kindle` is
   SSHFS-mounted and available — as of this scaffolding pass it was not
   checked for a live `statistics.sqlite3` (do that first; the file
   typically lives under a `koreader/settings/` path on the device). Copy
   it out read-only. If a real device sample isn't available, build
   synthetic fixtures matching the confirmed schema instead of guessing.
3. **Survey what KOReader's own built-in statistics screen already shows**
   (in-app calendar heatmap, per-book stats view) so Colophon's widget set
   adds real value instead of re-skinning what's already there.
4. **Survey existing third-party tools** that already visualize KOReader
   stats (GitHub projects, scripts, dashboards). Note what metrics/chart
   ideas they've already explored and which of those are worth adopting or
   improving on, and confirm they're the "web/Docker" pattern Brandon wants
   to avoid rather than something closer to what he actually wants.
5. **Check for other on-device data sources** worth mining: KOReader's
   vocabulary builder/flashcard feature has its own DB; highlights/notes and
   per-book `.sdr` sidecars may add data the core stats DB doesn't have.

Write the findings up (a `RESEARCH.md` dossier, matching the pattern used in
`~/.gitrepos/Coffer/RESEARCH.md` for a similar Phase-0-research project) with
real schema tables and citations, *then* update `spec.md` and `roadmap.md`
with the confirmed data model and a real (not brainstormed) widget list.
Only after that should Phase 1 (ingestion core) start for real.

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

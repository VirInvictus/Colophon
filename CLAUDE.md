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

## Where this stands right now (2026-07-03, v0.1.0)

Phases 0 and 1 are complete (both finished 2026-07-03; scaffolding was
Sonnet's, research synthesis and the ingestion core are Fable's).

- **Phase 0 (research) is done.** `RESEARCH.md` is the canonical record:
  confirmed schema (§1), KOReader's own built-in stats UI surveyed from
  the plugin source (§4), all four third-party tools read in depth (§5),
  the converged conventions Colophon adopts (§6), the `.sdr` sidecar
  format (§7), and the underexplored territory that justifies the project
  (§8). `spec.md` is locked: normative derived-metric definitions plus a
  three-tier widget catalogue.
- **Phase 1 (ingestion core) is done.** `colophon-core` has the typed
  read-only query layer (md5-merged books, raw events, the rescaled
  `page_stat` view, WAL-safe `snapshot()`) and pure derived-metric
  functions (sessions, daily totals, streaks, interval-union coverage,
  capped/uncapped totals, speed series, completion detection). 42 tests;
  fixtures are built programmatically from verbatim KOReader DDL, plus a
  live-sample test that skips when the gitignored Kindle copy is absent.
- Real sample databases from Brandon's own Kindle live at
  `research/samples/` (**gitignored, never commit them**); the on-device
  plugin Lua source is checked into `research/koreader-plugin-src/`.

Next up is Phase 2 (app shell): real `adw::ApplicationWindow` layout
replacing the placeholder `StatusPage`, loading a chosen db copy and
proving the ingestion → UI path. The charting approach (cairo vs. a crate)
is still deliberately undecided; decide it in Phase 3 against real widget
shapes, and ask before adding any dependency.

Small outstanding research nicety (not blocking): copy one real `.sdr`
sidecar (`<book>.sdr/metadata.epub.lua` for a highlighted book) into
`research/samples/` next time the Kindle is SSHFS-mounted at
`/mnt/Kindle`.

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

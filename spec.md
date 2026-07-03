# Colophon — spec

**Status: draft, pre-research.** This spec is intentionally thin. Phase 0
(see `roadmap.md`) is a deep research pass into what KOReader's statistics
data actually contains; large parts of this document are placeholders until
that pass locks the real schema and data model. Do not treat anything below
as final — especially the widget list, which is a brainstorm, not a
commitment.

## Core concept

Colophon is a native GTK4 / libadwaita desktop app that turns KOReader's
reading-statistics database into attractive graphs and a wide variety of
statistic widgets. It is a *viewer*, not a KOReader plugin and not a sync
service — it operates on a local copy of the data.

## Philosophy

- **Read-only, always.** Colophon never opens KOReader's live database file
  in place, and never writes to any file KOReader owns. It works from a
  copy (transferred by the user, e.g. via the SSHFS-mounted `/mnt/Kindle`,
  or a manual file drop).
- **Local-first.** No accounts, no cloud sync, no telemetry, works fully
  offline. On-brand for the rest of the portfolio.
- **Native, not a web dashboard.** The explicit gap this project targets:
  every existing KOReader stats tool Brandon has run into is a web UI or a
  self-hosted Docker service. This is a real GTK4/libadwaita app.
- **Breadth over one fixed report.** KOReader's own in-app statistics screen
  already covers the basics (calendar heatmap, per-book totals). Colophon's
  reason to exist is a *wide variety* of widgets/charts pulling different
  cuts of the same underlying data — the value is in depth and variety, not
  in re-skinning what KOReader already shows.

## Open questions (blocking a real spec)

All of these require the Phase 0 research pass:

1. **Real schema.** Confirmed table/column names for `statistics.sqlite3`
   (book-level table, per-page/session table, any others). Source of truth:
   KOReader's own `plugins/statistics.koplugin/` Lua source, not guesswork.
2. **Data granularity.** What's actually recorded per reading session —
   timestamp resolution, page-level vs. chunk-level duration, whether page
   count is stable per book (it isn't, if font size changes — need to know
   how KOReader handles that).
3. **What KOReader's own stats UI already shows**, so widget design doesn't
   just duplicate it.
4. **Other data sources on the device** worth mining beyond the core stats
   DB (highlights/notes, vocabulary builder DB, per-book `.sdr` sidecar
   metadata).
5. **Existing third-party tools** in this space — what's been tried, what
   metrics/visualizations they found compelling, what gaps remain.
6. **Multi-device / re-read behavior** — how the data represents reading the
   same book across devices or reading it more than once.

## Widget/chart brainstorm (unvalidated — a starting list, not a commitment)

Placeholder ideas to sanity-check once the real schema is known: reading
pace over time, session-length distribution, time-of-day/day-of-week reading
heatmap, pages-per-day streaks, reading speed trends per book vs. overall,
"velocity" through a single book (did it drag in the middle?), estimated
finish-date projections, library-wide totals (total hours, total pages,
books/year), longest sessions, comparison across books/genres if metadata
allows it.

## Stack

Rust 2024, GTK4 / libadwaita, `rusqlite` (read-only opens only). Two-crate
workspace: `colophon-core` (ingestion/querying) and `colophon` (the GTK
shell). Charting approach (native cairo drawing vs. a Rust charting crate)
is an open decision for after Phase 0 — don't lock it in prematurely.

## Non-goals (for now)

- Writing back to KOReader's database or config in any way.
- Cloud sync / multi-device merge logic beyond what's already baked into
  the KOReader data itself.
- Supporting reading-stats formats from other e-reader software (Kobo's
  native firmware stats, Moon+ Reader, etc.) — KOReader only, unless a
  strong case emerges later.

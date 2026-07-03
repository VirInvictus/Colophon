# Colophon — spec

**Status: draft, Phase 0 partially done.** The real `statistics.sqlite3`
schema is now confirmed (see `RESEARCH.md`) — this spec is no longer
guessing at the data model. What's still open: the deeper read of existing
third-party tools and KOReader's own built-in stats UI (roadmap Phase 0),
which the widget list below still needs before it's a real commitment
rather than a brainstorm.

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

## Data model (confirmed — see `RESEARCH.md` §1 for full detail)

`statistics.sqlite3` (`koreader/settings/statistics.sqlite3` on device) has
three things that matter: a `book` table (title/authors/series/language/
md5, `pages`, running `total_read_time`/`total_read_pages` totals, plus
`notes`/`highlights` as **counts only**, not content); a raw
`page_stat_data` table (one row per page-turn: book, page, start_time,
duration, and the page count *at that moment*, which is how KOReader copes
with font-size changes shifting pagination); and a `page_stat` view that
rescales historical rows onto the book's current page count for
apples-to-apples charting. Timestamps are unix epoch seconds, no stored
timezone.

## Open questions (blocking a real spec)

Down to what Phase 0's second pass needs to finish (see `roadmap.md`):

1. **What KOReader's own stats UI already shows**, so widget design doesn't
   just duplicate it. Not yet surveyed.
2. **Existing third-party tools** in this space — `KoInsight`, `KoShelf`,
   `Kodashboard`, `readingstreak.koplugin` are cloned into
   `~/.gitrepos/.studyrepos/` (see `RESEARCH.md` §4) but not yet read in
   depth for their metric/visualization catalogues.
3. **Highlight/note *content*** (as opposed to the counts already
   confirmed) lives in per-book `.sdr` sidecar metadata, not in this
   database — not yet located. Only matters if Colophon wants to surface
   highlight text, not just counts.
4. **Multi-device merge.** The schema is purely local to one device; if
   Brandon ever reads on more than one, merging two `statistics.sqlite3`
   files is unsolved by KOReader itself (see `RESEARCH.md` §1's sync note).
   Not a v1 concern, flagged for later.

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

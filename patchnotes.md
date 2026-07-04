# Patchnotes

## v0.5.0 — 2026-07-03

Time windows, the per-book speed overlay, and session patterns.

The All Books overview gains a 30d / 90d / 365d / all-time selector. It
scopes the totals tiles and the behaviour charts (when-do-I-read, speed,
sessions, weekday averages) to calendar windows ending today; streaks,
the year heatmap, and the monthly bars deliberately stay whole-history,
since windowing a streak or a year grid would just lie. Windowed totals
are computed from event sums (identical to the device counters for
all-time, verified against the sample).

The book page gets its own reading-speed trend with the library baseline
muted behind it, on the same bucket so the two series are comparable
(the line chart grew date-scaled multi-series support). Session
analytics add sessions-per-active-day and a starts-by-hour chart with
per-bar tooltips (bar charts now support tooltips generally). Read-
through cards carry pages/day over the calendar span, completing the
book-velocity item.

68 tests (new: window scoping, session starts/density).

## v0.4.0 — 2026-07-03

The Tier A widgets land: the analytics nobody else ships.

On the All Books overview: a weekday-by-hour "When do I read" heatmap
over the whole history (per-cell tooltips; the aggregate profile KOReader
only shows one day at a time), a reading-speed trend (pages/hour as a
cairo line/area chart, daily buckets while the history is young and
weekly past ten weeks, nearest-point tooltips), session analytics (a
session-length histogram from under-5-minutes to over-2-hours, with
count, median, and longest-session records), and the monthly distribution
with empty months rendered rather than skipped.

On the book page: the per-page activity strip (per-page time and read
count on the stable page axis, sqrt scaling capped at the 90th
percentile, pixel-binned so long books stay readable, per-range
tooltips), which doubles as the "did it drag in the middle" velocity
view; and read-through cards from the completion detector (dates, time,
sessions, pages/hour, coverage), hidden for books with none.

New core metric: `hourly_profile` (weekday x hour bucketing, attribution
by start time). Two new chart widgets (hour heatmap, line chart) plus the
activity strip, all on the same cairo scaffolding. 66 tests.

## v0.3.0 — 2026-07-03

Phase 3 opens: the charting decision is settled and the first widgets
ship on two new content surfaces.

The sidebar gains an "All Books" entry (Brandon's request) above the book
list. Selecting it shows the library-wide overview: totals tiles (time,
pages read, books, active days, busiest day), current and longest streak
tiles with date ranges, a GitHub-style year heatmap (quantized intensity,
per-day tooltips, grid sized to the history), and average time by weekday
(normalized by weekdays elapsed, strongest day highlighted). Selecting a
book shows the per-book page: interval-union progress bar and the
device-parity stat cards (capped "as shown on device" total with the
uncapped sum alongside, days reading, averages, sessions, and KOReader's
own time-left and finish-date estimate math). Both surfaces respect the
junk filter and recompute live when it toggles.

Charting verdict: custom cairo drawing on GtkDrawingArea, no charting
crate. The heatmap and bar chart shipped as production widgets with
shared Kanagawa ramps for light and dark, a discrete intensity quantizer,
and theme-reactive redraws. Zero new dependencies.

59 tests (new: overview/book-detail aggregate math, weekday
normalization, heat quantizer).

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

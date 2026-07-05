# Patchnotes

## v0.10.0 — 2026-07-05

More from the data you already have.

The overview gains a **Series** section: books grouped by their series
metadata (Calibre-style "Name #index"), each row showing how many works
it holds, how many you have finished, and total time. Two files of one
work (the same title appearing twice) count once; books without series
metadata are left out.

The book page gains **re-read detection**: "Pages revisited", the count
of current-axis pages read more than once, shown when there are any.

Both come straight from data already loaded, no new dependencies. Series
composition is whole-library, so it does not move with the time-window
selector. 81 tests.

## v0.9.0 — 2026-07-05

Reading personality.

The All Books overview gains a "Reading personality" section: three
plain-language traits synthesised from the behaviour data the charts
already show, no new data required. Chronotype (early bird, daytime,
evening, night owl) from your peak reading hour; session style
(marathoner, steady, sipper) from your median sitting; and weekly rhythm
(weekday, weekend, all-week reader) from how your weekday and weekend
averages compare. Each trait names the number behind it, the section
respects the time-window selector, and it hides itself when there is too
little reading in the window to say anything honest.

80 tests.

## v0.8.0 — 2026-07-05

Themes.

Colophon was Kanagawa Dragon only (with a plain accent-on-Adwaita light
mode). It now ships eight palettes: Kanagawa Dragon, Kanagawa Wave,
Kanagawa Lotus, Gruvbox Dark, Gruvbox Light, Nord, Rosé Pine, and
Solarized Light, plus a Follow-system mode that tracks the desktop
light/dark preference (Dragon when dark, Lotus when light).

Pick one in the new Preferences dialog (Ctrl+comma). A single theme
definition drives both the libadwaita colours and the chart palette, so
the whole window and every graph reflow together, live, the moment you
switch.

Under the hood the two static CSS sheets are gone: the stylesheet is
generated from the active theme, and the cairo charts read it at draw
time. New GSettings key `theme`. 77 tests.

## v0.7.0 — 2026-07-05

Honest per-book progress.

The per-book progress bar was showing interval-union coverage (unique
pages logged, as a fraction of the whole book) as a single left-anchored
fill. For a book read partly outside KOReader (say, started on a stock
Kindle and resumed in KOReader after a mid-book jailbreak), that reads as
"partly done" when the book was actually finished: KOReader only logged
the pages it saw.

It is now a positional span bar. It draws *where* in the book reading was
logged (read regions filled, unlogged gaps empty) on the page axis, with a
marker at the furthest position reached, so a book read from the middle
onward reads honestly instead of looking half-finished. When the furthest
position reaches the end (within the last 2 %, the endpoint the completion
detector already uses), the book gets a Finished marker. The caption names
the gap: "600 of 866 pages logged (69%), ~31% read before KOReader."

Two new pure, tested core metrics back it: `coverage_spans` (the merged
read intervals) and `furthest_position` (the progress measure, unaffected
by an unlogged leading gap). The `.sdr` sidecar's user-declared finished
flag will override the inference once sidecars are in scope.

74 tests.

## v0.6.0 — 2026-07-04

A performance pass for large libraries (Phase 4), measured against a
synthetic multi-year database rather than the tiny real sample.

Colophon no longer holds KOReader's fanned-out `page_stat` view in memory.
That view rescales every stored row across the current page axis (up to
about 1000x), and the old load pulled all of it into memory for every
book. Now a per-page `GROUP BY` reduction (one row per current-axis page)
feeds the capped totals and the activity strip, and the last-read page is
recovered from the most recent raw event instead of a second scan of the
view. The numbers are identical, locked by a parity test against the old
path; on a synthetic 200-book, four-year, 222k-event database the resident
set drops from 27 MB to 19 MB (103k rows held instead of 367k), and the
saving grows with the library.

The All Books overview now caches its whole-history aggregates: the daily
map behind streaks, the year heatmap, and the monthly bars. They are
rebuilt only when the filtered set changes (a junk-filter toggle or a
re-import), not on every time-window switch. Switching windows recomputes
just the windowed behaviour charts, so narrowing to a recent window went
from about 20 ms to 3 ms and the all-time view from 44 ms to 23 ms on that
same database. First render and junk-filter toggles are unchanged by
design.

Under the hood: a new ignored measurement harness
(`colophon-core/tests/perf.rs`) generates a deterministic multi-year
fixture and reports load, render, and memory numbers, so future changes
are measured rather than guessed. 70 tests.

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

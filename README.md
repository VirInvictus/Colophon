# Colophon

A native GNOME (GTK4 / libadwaita) statistics viewer for [KOReader](https://koreader.rocks/).

KOReader already tracks a surprising amount about how you read: per-page
timing, per-book totals, session history. The existing ways to look at that
data are web dashboards or self-hosted Docker services. Colophon is neither:
it's a local desktop app that reads a copy of your KOReader statistics
database and turns it into attractive, varied graphs and widgets. No
server, no account, no cloud.

**Status:** early scaffolding (v0.0.1). Not usable yet — see `roadmap.md`.

## What it is

- Reads a **copy** of KOReader's `statistics.sqlite3` (never the live file on
  the device, always read-only).
- A widget-based dashboard: a wide variety of small, focused stat/graph
  widgets rather than one fixed report, so the app can grow new views as we
  learn what the data actually supports.
- Kanagawa Dragon themed, matching the rest of the [Vir Invictus](https://github.com/VirInvictus) portfolio.
- Local-first: no telemetry, no accounts, works fully offline.

## Why "Colophon"

A colophon is the note printers historically placed at the end of a book —
press, date, paper, edition details. It's the book's own record of its
production. This app is that idea turned toward *reading* instead of
printing: the technical record of how a book was actually read.

## Stack

Rust 2024, GTK4 / libadwaita, `rusqlite` (read-only). Two crates:
`colophon-core` (KOReader stats ingestion) and `colophon` (the GTK app).

## Building

```sh
cargo build
```

(No Meson/Flatpak packaging yet — that arrives once the UI design is locked;
see `roadmap.md`.)

## License

MIT. See `LICENSE`.

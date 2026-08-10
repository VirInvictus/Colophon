# Vendored KOReader plugin source (reference only)

This directory holds a verbatim copy of KOReader's reading-statistics
plugin, taken from the device on 2026-07-03. It is **third-party source,
not Colophon's**, and it is here for one reason: it is the authoritative
definition of the `statistics.sqlite3` schema, the `page_stat` rescaling
view, and the capped-totals math that Colophon reproduces so its numbers
never contradict the device.

| | |
|---|---|
| Upstream | <https://github.com/koreader/koreader> |
| Path upstream | `plugins/statistics.koplugin/` |
| Files here | `_meta.lua`, `main.lua`, `calendarview.lua`, `readerprogress.lua` |
| Copied from | Brandon's jailbroken Kindle Oasis 3, 2026-07-03 |
| Copyright | The KOReader contributors |
| Licence | **AGPL-3.0-or-later** |

## Why this needs saying

Colophon itself is MIT (see the top-level `LICENSE`). These four files are
not: they are AGPL-3.0-or-later, like the rest of KOReader. Nothing here is
compiled, linked, imported, or shipped by Colophon. No Lua from this
directory runs at any point; the app's only Lua is the sandboxed `mlua` VM
that parses `.sdr` sidecars, and that VM executes the user's own sidecar
files, never these.

So the copy is reference material a human reads, and the licences do not
mix. Keep it that way:

- **Do not** copy code out of these files into `colophon/` or
  `colophon-core/`. Reproduce the *behaviour* from the documented
  definitions in `RESEARCH.md`, which is what the derived-metric layer
  already does.
- **Do** cite the file and line here when writing a metric that has to
  match the device, exactly as `RESEARCH.md` §1 and the `spec.md` normative
  definitions do.

## Refreshing it

These files track whatever KOReader version the device is running, so they
can drift from upstream master. If you re-copy them, note the date and the
device's KOReader version here, and re-check `RESEARCH.md` §1: a schema or
view change upstream is exactly the kind of thing that would silently
invalidate Colophon's parity math.

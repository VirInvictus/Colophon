# Security policy

## Supported versions

Only the latest release (see the releases page) receives security fixes.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting:
the **Security** tab of this repository → **Report a vulnerability**.
Reports stay private until a fix is ready; please do not open a public
issue for anything you believe is exploitable.

## Scope notes

Colophon is a local desktop application: no server, no telemetry, no
network access at runtime. The areas worth scrutiny if you are looking
for them:

- The database import path accepts any `*.db` a user picks; it is
  snapshotted (plain file copy), validated by fully loading the copy,
  and only then promoted. Everything opens `SQLITE_OPEN_READ_ONLY`.
- The `.sdr` sidecar parser evaluates user-supplied Lua in a sandboxed
  VM with the standard library disabled (`StdLib::NONE`), text chunks
  only, UTF-8 repaired lossily.
- The app reads only file paths the user has explicitly given it; it
  never scans or discovers device files.

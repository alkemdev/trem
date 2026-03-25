# User-facing errors and diagnostics

## What

Replace opaque `anyhow` chains at CLI boundaries with structured errors: Rung parse line/column, MIDI import issues, audio device open failures with actionable hints (permissions, device busy).

## Why

Music tools are judged on first-run success; good errors reduce support load and GitHub noise.

## Notes

- Keep `anyhow` internal; use `thiserror` at crate boundaries where it helps.
- Link to `docs/install.md` sections from error text where stable.

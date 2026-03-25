# Headless WAV export CLI

## What

Add a `trem render` (or `cargo run -- render`) subcommand: load a project descriptor (graph + pattern or Rung clip + tempo), render offline to WAV/FLAC via existing offline render paths, no TUI.

## Why

The TUI is great for iteration; CI, batch rendering, and sharing stems need a scriptable path. This also dogfoods the library-first boundary (`trem` without `trem-tui`).

## Notes

- Start with a minimal JSON or Rung-based project file format; avoid inventing a second graph DSL without need.
- Consider stdout-friendly progress for long renders.

# Arrange / clip timeline editor (TUI)

## What

A third fullscreen mode (or sub-mode) for song-level structure: clips on a timeline, looping regions, maybe markers — aligned with `docs/tui-editor-roadmap.md` "Arrange" row.

## Why

Pattern roll solves phrase-level editing; long-form work needs arrangement without leaving the terminal philosophy.

## Notes

- Depends on a clip/buffer model in the engine; may start as "Rung clips on a linear beat ruler" before full audio warping.
- Shared contract: `docs/modes/principles.md`.

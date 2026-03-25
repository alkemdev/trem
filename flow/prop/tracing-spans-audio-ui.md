# `tracing` spans: audio thread vs UI

## What

Optional `tracing` integration: spans around `process()` blocks, bridge send/recv, TUI frame timing; feature-gated so default builds stay lean.

## Why

Debugging dropouts and "who blocks whom" between UI and audio is otherwise printf archaeology.

## Notes

- Never log per-sample in hot paths; aggregate counters or rare events only.
- Document how to capture with `tracing-subscriber` env filter.

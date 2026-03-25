# Rung ↔ pattern engine bridge

## What

First-class conversion between `trem::rung` clips and `Tree`/`Grid` models (both directions where possible), plus CLI/TUI actions: "import clip to pattern", "export selection to Rung".

## Why

Rung is already the interchange format; deeper integration makes the piano-roll and grid sequencer feel like one product instead of two tools.

## Notes

- Voice/class mapping and polyphony limits need explicit rules per instrument.
- Round-trip tests: Rung → internal → Rung should preserve invariants you care about.

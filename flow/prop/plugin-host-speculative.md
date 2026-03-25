# External plugin host (CLAP / VST3) — speculative

## What

Long-horizon exploration: host CLAP or VST3 plugins as `Processor` nodes with fixed buffer contracts, or alternatively run trem *as* a plugin in other DAWs.

## Why

Interoperability with the wider audio ecosystem; acknowledges the ceiling of an all-internal DSP graph.

## Notes

- Huge surface area (presets, latency, MIDI, sidechains). Treat as research spike only.
- If pursued, likely a separate crate behind features and explicit non-goals doc.

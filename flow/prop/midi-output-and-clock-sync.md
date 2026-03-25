# MIDI output and clock sync

## What

Optional MIDI clock / MTC / note output from the transport layer (likely `trem-rta` or a thin `trem-midi-out` crate behind a feature flag), driven by the same rational timing model as the engine.

## Why

Integrates trem with hardware, DAWs, and lighting rigs. Many users expect "sync out" even if the engine stays internal.

## Notes

- MIDI time is coarse; document mapping from rational beats to ticks and jitter bounds.
- Feature-gate platform MIDI crates to keep core dependency-light.

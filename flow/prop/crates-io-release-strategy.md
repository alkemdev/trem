# crates.io publishing strategy

## What

Define which crates publish (`trem`, `trem-rta`, `trem-dsp`, `trem-tui`, others), semver rules for the DSP registry and graph types, and a changelog discipline.

## Why

Metadata was added to manifests; actual releases unlock downstream tools depending on `trem` without git pins.

## Notes

- Pre-1.0 is fine; document breaking areas (`registry` tags, `Node` trait).
- Consider `-sys` / platform crates only if MIDI I/O expands.

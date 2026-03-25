# Sample editor: waveform, trim, regions

## What

Editor sketched in the TUI roadmap: load short samples, view waveform in terminal (braille blocks or simplified), set start/end, assign to a sampler node.

## Why

Unlocks realistic drums and vocals inside trem without external DAW prep for every sound.

## Notes

- Needs a buffer ownership story in core (or a dedicated `SampleBuffer` type) and bridge commands for playhead/selection.
- Consider max sample length and memory caps for TTY use.

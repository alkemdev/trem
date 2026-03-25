# Mixer / bus strips in the TUI

## What

Dedicated view or sidebar mode for per-bus levels, mute/solo, sends — matching roadmap "Mixer / buses" once the host graph exposes enough structure.

## Why

Graph view excels at topology; mixing is a different mental model. Reduces hunting for gain nodes on large graphs.

## Notes

- May require richer snapshots from `Graph` (bus list, metering taps).
- Align metering with existing spectrum/waveform philosophy (instrument vs master).

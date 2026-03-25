# New DSP: granular and spectral processors

## What

Add processors for short-grain playback (classic granular), maybe a minimal FFT magnitude bin shaper for experimental timbre — within existing `Processor` trait patterns.

## Why

Expands sound palette beyond subtractive/wavetable; showcases graph modularity.

## Notes

- FFT size vs latency tradeoff; probably offline-first or high-latency real-time.
- Document CPU cost class in registry metadata.

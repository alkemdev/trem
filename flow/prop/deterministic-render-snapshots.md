# Deterministic offline render regression suite

## What

Fixed-seed, short renders of representative graphs to golden WAV hashes (or RMS/peak envelopes) checked in CI; catch accidental DSP drift.

## Why

Floating DSP will drift across platforms; you can still detect *unexpected* change. Protects refactors in `Graph` scheduling and processor order.

## Notes

- Use a tolerance band or compare downsampled envelopes, not bitwise WAV.
- Store tiny fixtures under `tests/` or `assets/snapshots/`.

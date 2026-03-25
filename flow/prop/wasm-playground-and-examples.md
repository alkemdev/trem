# WASM playground and browser examples

## What

Ship a minimal web page (or separate `trem-wasm-demo` crate) that loads the core `trem` library compiled to WASM, exposes a tiny API (build a graph, render N frames to an `AudioWorklet` or offline buffer), and documents the exact feature subset that works without threads or cpal.

## Why

README already claims the core compiles to WASM; most users never see it. A working demo validates that claim, catches `std`/`cpal` leaks into core, and is a strong onboarding story for contributors and educators.

## Notes

- Prefer `wasm-bindgen` + a single static example over a heavy frontend stack.
- Call out float boundary explicitly in UI copy (exact timing vs float samples).

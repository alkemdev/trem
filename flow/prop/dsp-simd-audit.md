# SIMD / autovectorization audit for DSP hot loops

## What

Profile offline render and real-time path; identify inner loops (filters, mixing) where `stdsimd`, portable `wide`, or careful `#[target_feature]` helps; maintain a scalar fallback for WASM and deterministic builds.

## Why

Headroom matters for nested graphs; SIMD is often free performance if boundaries are clean.

## Notes

- Determinism: SIMD order can differ slightly from scalar; document if acceptable only at float boundary.
- Run `cargo bench -p trem` before/after with fixed seeds.

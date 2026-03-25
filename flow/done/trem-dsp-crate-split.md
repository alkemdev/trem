# Split built-in DSP into `trem-dsp` — **done**

## Layout (as shipped)

| Part | Role |
|------|------|
| **`trem_dsp::interfaces`** | Re-exports from `trem` (`Node`, `Graph`, `GraphEvent`, …) for custom node authors. |
| **`trem_dsp::standard`** | Stock `Node` implementations (former `trem::dsp::*`). |
| **`register_standard` / `standard_registry`** | Fill a `trem::registry::Registry` with built-in tags. |

`Registry::standard()` was removed from `trem` to avoid a **circular dependency** (`trem` → `trem-dsp` → `trem`). Callers use `trem_dsp::standard_registry()` or `register_standard(&mut reg)`.

## Cargo constraint: no `trem` dev-dependency on `trem-dsp`

If `trem` listed `trem-dsp` under `[dev-dependencies]`, Cargo builds two copies of `trem`, and `Node` types from `trem_dsp` no longer match `trem::graph::Node`.

Therefore:

- Integration tests that need **both** core + stock DSP live under **`crates/trem-dsp/tests/`** (e.g. `render_pattern.rs`).
- Examples **`offline_render`** and **`custom_processor`** live under **`crates/trem-dsp/examples/`**.
- DSP + graph **benchmarks** live under **`crates/trem-dsp/benches/`**.

The root binary depends on `trem-dsp` normally (not as a dev-dep of `trem`), so the demo graph is unaffected.

## Optional follow-ups

- `serde` on `trem` / future preset formats: decide whether `trem-dsp` gets optional `serde` too.
- crates.io: publish order `trem` then `trem-dsp` with path replaced by version.

# trem

A mathematical music engine in Rust.

**trem** is a library-first DAW built on exact arithmetic, xenharmonic pitch systems,
recursive temporal trees, and typed audio graphs. The terminal UI is a first-class citizen.

## Principles

- **Exact where possible.** Time is rational (integer pairs). Pitch degree is an integer
  index into an arbitrary scale. Floating-point only appears at the DSP boundary.
- **Few assumptions.** No 12-TET default, no 4/4 default, no fixed grid resolution.
  Tuning, meter, and subdivision are all parameters.
- **Composition is a tree.** Patterns are recursive `Tree<Event>` structures. Children
  subdivide the parent's time span. Triplets, quintuplets, nested polyrhythms — just tree shapes.
- **Sound is a graph.** Audio processing is a DAG of typed processor nodes. The graph is
  data: serializable, inspectable, modifiable at runtime.
- **Library first.** The core `trem` crate has zero I/O dependencies. It compiles to WASM.
  It renders offline to buffers. The TUI and audio driver are separate crates.

## Crates

| Crate | Purpose |
|-------|---------|
| `trem` | Core library — math, pitch, time, trees, graphs, DSP, rendering |
| `trem-cpal` | Real-time audio backend via cpal |
| `trem-tui` | Terminal UI via ratatui |

## Quick Start

```bash
cargo run
```

## Building the library only

```bash
cargo build -p trem
```

## License

MIT OR Apache-2.0

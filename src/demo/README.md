# Demo patch (`trem` binary)

This directory is the **default session** loaded by **`trem`** / **`cargo run`** (or explicitly `trem tui`).

| File        | Role |
|-------------|------|
| `levels.rs` | All mix constants (channels, buses, master FX). Start here to rebalance loudness. |
| `graph.rs`  | Nested instrument channels, drum/inst/main buses, and parameter exposure. |
| `pattern.rs`| 32×5 starter grid (lead arp, bass, drums, hats). |

`main.rs` only wires I/O and calls `demo::build_graph()` / `demo::build_pattern()`.

# Design review: types, functional style, data-driven layout

This note captures a pass over **trem** (core + TUI) for stronger typing, less ad-hoc representation, and clearer separation of **data** vs **effects**.

## What’s already in good shape

- **`trem::graph`**: `NodeId`, `PortIdx`, `Sig`, `Edge`, `ParamDescriptor`, `ParamUnit`, `GroupHint` — introspection is explicitly data-driven; processors describe themselves for any UI.
- **`Sig::chain` / `parallel`**: algebraic API for port shapes instead of unchecked wiring everywhere.
- **Bridge `Command` / `Notification`**: explicit sum types for cross-thread messages.
- **Grid / rational time / scale types**: domain concepts are named, not only `f64` soup.

## Improvements applied in-repo

1. **`GraphSnapshot::edges`**: was `Vec<(u32, u16, u32, u16)>`, now **`Vec<Edge>`** — same shape as runtime graph and `topology()`, no duplicate anonymous tuple type.
2. **TUI graph plumbing** (`App`, `GraphViewWidget`, `compute_graph_nav`, `compute_layout`): all take **`&[Edge]`** / `Vec<Edge>` so field names (`src_node`, `dst_node`, …) document intent at call sites.
3. **`detail_panel_scroll`**: pure function + **unit tests** for the detail-panel scroll math (avoids fragile `clamp` when derived bounds disagree).

## Recommended next steps (prioritized)

### High value / low churn

- **`NodePath` usage end-to-end**: `Command::SetParam.path` is already `Vec<u32>`; consider a thin newtype `NodePath(Vec<NodeId>)` (or re-export path type from `trem` in `trem-cpal`) so “list of node indices” isn’t interchangeable with arbitrary `Vec<u32>`.
- **`(NodeId, String)` rows**: introduce e.g. `struct NodeRow { id: NodeId, label: String }` or `type NodeRow = (NodeId, String)` **in `trem::graph`** and use it in TUI + snapshots for one canonical “node list row” type.
- **`snapshot_all_params` return type**: today `Vec<(Vec<ParamDescriptor>, Vec<f64>, Vec<ParamGroup>)>`. A `struct NodeParamSnapshot { descriptors, values, groups }` (or nested `NodeSnapshot`-aligned struct) removes tuple noise and keeps field names at boundaries.

### Medium: data vs behavior

- **Pattern undo**: `Vec<Vec<Option<NoteEvent>>>` duplicates grid shape; fine for MVP, but a small `struct PatternSnapshot(Vec<...>)` or storing only **diffs** would clarify invariants (length == `grid.rows * grid.columns` etc.).
- **Graph navigation**: `graph_move_left` / `right` build adjacency on the fly each keypress. A **precomputed** `HashMap<NodeId, Vec<NodeId>>` (outgoing / incoming) rebuilt when `graph_nodes` / `graph_edges` change would be more data-driven and easier to test without `App`.

### DSP / engine (when touching that code)

- Prefer **small value types** for modulation routing IDs instead of raw `u32` where the space isn’t truly flat.
- Keep **table-driven** registry (`registry.rs`) as the single place that maps tags → constructors; avoid parallel `match` trees in callers.

## Functional style (pragmatic)

Rust will stay mostly imperative inside hot paths; “functional” here means:

- **Pure helpers** for layout/scrolling/formatting (testable, no `self`).
- **Iterators** over explicit structures instead of index loops where clarity wins.
- **Enums** for mode and message types (already done for `Command` / `View` / `Mode`).

Avoid “functional” at the cost of **allocations** in the audio thread; the UI thread has more freedom.

## How to validate after refactors

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings   # optional stricter gate
```

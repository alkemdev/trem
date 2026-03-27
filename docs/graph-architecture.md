# Graph architecture: `Graph`, `Node`, and composition helpers

This document describes the audio graph model in the **`trem`** crate (`trem::graph`). For stock node implementations, see **`trem-dsp`**.

## Terms

| Name | Meaning |
|------|---------|
| **`Node`** | Trait implemented by every graph vertex: run [`Node::process`](https://docs.rs/trem/latest/trem/graph/trait.Node.html#tymethod.process) on one audio callback, optional parameter introspection, optional inner graph for nesting. |
| **`Graph`** | Directed acyclic graph of `Box<dyn Node>` vertices, wires, buffer pool, and I/O ports. **Also a `Node`**, so graphs nest inside graphs. |
| **Graph block size** | Maximum samples per `process` call for that graph (buffer capacity). Not the same as the `Node` trait: the trait = “node kind,” block size = “how many samples per callback.” |
| **`ProcessContext`** | Input/output slices, `frames`, `sample_rate`, and block-relative [`TimedEvent`](https://docs.rs/trem/latest/trem/event/struct.TimedEvent.html)s for one callback. |
| **`Sig`** | Input/output **port counts** for a node; used to validate chaining and parallel merge. |
| **`NodeInfo`** | Name + `Sig` + description exposed when a node (or nested graph) describes itself. |
| **`Registry`** | Maps short tags (e.g. `"osc"`) to factories of boxed `Node`s; fill with `trem_dsp::register_standard`. |

The trait was previously named **`Processor`**, then **`Block`**; rustdoc lists those as search aliases on `Node`.

## Building graphs

### Manual

1. [`Graph::new`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.new) or [`Graph::labeled`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.labeled) with a **block size**.
2. [`add_node`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.add_node)(`Box<dyn Node>`).
3. [`connect`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.connect)(from_node, from_port, to_node, to_port).
4. [`set_input`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.set_input) / [`set_output`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.set_output) when this graph is used as a nested `Node` with external I/O.

### Sequential chain

Wire `a → b → c` automatically when each stage’s outputs match the next stage’s inputs:

- [`Graph::chain`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.chain) / [`Graph::from_chain`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.from_chain) — same behavior; `from_chain` reads well in call sites.
- [`Graph::chain_from_iter`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.chain_from_iter) — like `chain`, but takes any iterator of `Box<dyn Node>` (e.g. `vec![…]`).

On signature mismatch, these return [`SigMismatch`](https://docs.rs/trem/latest/trem/graph/struct.SigMismatch.html).

### Parallel bundle

Side-by-side nodes; total inputs/outputs are the sums of children (split/merge in port order):

- [`Graph::parallel`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.parallel) / [`Graph::from_parallel`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.from_parallel).

### Fluent pipeline

[`Graph::pipeline`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.pipeline) returns a [`PipelineBuilder`](https://docs.rs/trem/latest/trem/graph/struct.PipelineBuilder.html): optional [`input`](https://docs.rs/trem/latest/trem/graph/struct.PipelineBuilder.html#method.input), then repeated [`then`](https://docs.rs/trem/latest/trem/graph/struct.PipelineBuilder.html#method.then), then [`build`](https://docs.rs/trem/latest/trem/graph/struct.PipelineBuilder.html#method.build).

## Parameters and nesting

- Leaf nodes override [`params`](https://docs.rs/trem/latest/trem/graph/trait.Node.html#method.params), [`get_param`](https://docs.rs/trem/latest/trem/graph/trait.Node.html#method.get_param), [`set_param`](https://docs.rs/trem/latest/trem/graph/trait.Node.html#method.set_param) for automation.
- A nested **`Graph`** exposes parameters via [`expose_param`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.expose_param) / [`expose_param_in_group`](https://docs.rs/trem/latest/trem/graph/struct.Graph.html#method.expose_param_in_group).
- [`inner_graph`](https://docs.rs/trem/latest/trem/graph/trait.Node.html#method.inner_graph) / [`inner_graph_mut`](https://docs.rs/trem/latest/trem/graph/trait.Node.html#method.inner_graph_mut) on `Node` allow UIs to drill into nested graphs.

## Examples in the repo

- `crates/trem/src/graph.rs` — unit tests for `chain`, `parallel`, `pipeline`, nesting.
- `crates/trem-dsp/examples/offline_render.rs` — small graph built with stock nodes.
- `crates/trem-dsp/examples/custom_processor.rs` — custom `Node` + stock oscillator.
- `crates/trem-bin/src/demo/graph.rs` — full demo patch (nested buses, `trem_dsp::standard`).

## See also

- [AGENTS.md](../AGENTS.md) — build/test commands.
- [README.md](../README.md) — workspace overview and nested-graph snippet.
- **Prepare / block sizing:** [`PrepareEnv`](https://docs.rs/trem/latest/trem/graph/struct.PrepareEnv.html) and [`Node::prepare`](https://docs.rs/trem/latest/trem/graph/trait.Node.html#method.prepare) run when topology or graph buffer capacity changes (not every callback). Unit tests in `crates/trem/src/graph.rs` exercise prepare and buffer sizing.

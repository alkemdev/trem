# `trem-dsl` — domain-specific language (draft scope)

## Intent

A **textual** language for describing parts of a trem project — patches, patterns, trees, or full songs — so users can version-control music as source, generate from tools, or script without recompiling Rust.

## Open questions (decide before implementation)

- **Surface area**: Graph wiring only? Step grid? `Tree` rhythms? Rung-compatible note data? All of the above with a module system?
- **Execution model**: Parse → AST → build `Graph`/`Grid` in memory; or compile to a binary/IR; or interpret with hot reload.
- **Relationship to existing artifacts**: JSON Rung, future graph presets ([`graph-preset-serialization.md`](graph-preset-serialization.md)), and [`scripting-live-coding-layer.md`](scripting-live-coding-layer.md) — DSL should not fight these; prefer one canonical lowering target (e.g. “DSL lowers to `Graph` + `Grid` + `Scale`”) or explicit import/export.
- **Semantics for time**: DSL should align with rational beats and scale degrees where possible; document where floats are allowed (e.g. Hz for LFO rate).
- **Error messages**: Source spans, file names, and suggestions — part of the value of a DSL over raw JSON.

## Possible phases

1. **v0**: Expression subset for parameter automation or single-instrument patches (smallest grammar).
2. **v1**: Declarative graph blocks (nodes, edges, params) + named presets.
3. **v2**: Temporal layer (pattern rows or tree literals) + multi-file `use`.

## Crate layout (when ready)

- Workspace crate `trem-dsl` (parser + AST + lowering), depends on `trem`.
- Optional: `trem-dsl-cli` or integrate as `trem dsl check|run` later — tie to [`headless-wav-export-cli.md`](headless-wav-export-cli.md) if batch render is a goal.

## Non-goals (unless explicitly reopened)

- Becoming a general-purpose programming language (prefer embedding or IPC for Turing-complete control).
- Replacing the TUI; DSL complements headless and CI workflows.

## Next step

Narrow v0 scope in a short design note (one page: grammar sketch + one worked example), then move to `flow/todo/` when accepted.

# AGENTS.md

## Scope

**trem** is a mathematical music engine in Rust. It is structured as a
workspace of three crates:

- `crates/trem/` -- Core library (no I/O). Rational arithmetic, pitch/scale
  systems, temporal trees, audio processing graphs, DSP processors, Euclidean
  rhythms, processor registry, and offline rendering.
- `crates/trem-cpal/` -- Real-time audio backend using cpal + rtrb.
- `crates/trem-tui/` -- Terminal UI using ratatui + crossterm.

The binary (`src/main.rs`) wires them together into a TUI DAW.

## Commands

```bash
cargo check --workspace          # type-check everything
cargo test --workspace           # run all unit + integration tests
cargo test --workspace --doc     # run doc-test examples
cargo bench -p trem -- --test    # compile-check benchmarks
cargo doc --workspace --no-deps  # build documentation
cargo run                        # launch the TUI demo
cargo run -p trem --example <name>  # run a library example
```

## Workflow

Work items move through four directories at the repo root:

```
prop/  ->  todo/  ->  work/  ->  docs/
```

### prop/ -- Proposals

Draft ideas. Anyone can add a markdown file here. Describe what and why; rough
implementation is fine. Name files descriptively: `streaming-export.md`,
`midi-input.md`, etc.

### todo/ -- Accepted

Reviewed proposals that are ready to be picked up. Move here from `prop/` when
the idea is approved. Do not start implementation until a file is in `todo/`.

### work/ -- In Progress

Active work. Move here from `todo/` when you begin. Only one agent or person
should work on a given item at a time. The file may be updated with progress
notes during implementation.

### docs/ -- Done

Completed items. Move here from `work/` when implementation is finished, tests
pass, and the change is verified. Add a "Completed" date and a brief summary
of what was delivered. This directory is the project's decision log.

## Code Standards

- **License**: MIT.
- **Tests**: Run `cargo test --workspace` before marking anything done. All
  tests must pass.
- **Doc comments**: Every public type, function, and module should have a doc
  comment. Key types should have `# Examples` with tested code blocks.
- **No narration comments**: Do not add comments that just describe what the
  code does. Comments should only explain non-obvious intent or trade-offs.
- **Formatting**: Use `cargo fmt` conventions. No manual formatting overrides.
- **Dependencies**: Prefer well-maintained Rust crates over reinventing. Use
  the package manager to add dependencies (don't make up versions).

## Important Areas

- `crates/trem/src/graph.rs` -- The core `Graph` and `Processor` trait.
  `Graph` implements `Processor`, enabling recursive nesting.
- `crates/trem/src/dsp/` -- All built-in processors (oscillators, filters,
  dynamics, effects, drum synths, etc.).
- `crates/trem/src/registry.rs` -- Runtime processor factory mapping tags to
  constructors.
- `crates/trem/src/tree.rs` -- Recursive temporal trees for rhythm.
- `crates/trem/src/pitch.rs` -- Xenharmonic pitch and tuning systems.
- `crates/trem-cpal/src/driver.rs` -- Audio thread loop.
- `crates/trem-tui/src/app.rs` -- TUI application state and render loop.
- `crates/trem-tui/src/view/` -- Widget implementations for each TUI pane.
- `src/main.rs` -- Demo project: graph construction, pattern setup, TUI launch.

## Validation

Before completing any work item:

1. `cargo check --workspace` -- no errors
2. `cargo test --workspace` -- all tests pass
3. `cargo test --workspace --doc` -- doc examples pass
4. If DSP or graph changes: `cargo bench -p trem -- --test` -- benchmarks compile

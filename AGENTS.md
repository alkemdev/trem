# AGENTS.md

**New here?** End-user setup: [docs/install.md](docs/install.md) · this file is for **contributors** and **automation**. **Flow** (planning lifecycle): [flow/README.md](flow/README.md).

## Scope

**trem** is a mathematical music engine in Rust. The repo root is a **virtual
workspace** (`Cargo.toml` only): all crates live under **`crates/`**, including
the **`trem-bin`** application (`cargo run` targets it via **`default-members`**).

- `crates/trem/` -- Core library (no codec I/O). Rational arithmetic, pitch/scale
  systems, temporal trees, audio processing graphs, Euclidean rhythms, node
  registry, offline rendering, and typed PCM units (`trem::signal`). Optional **`rung`** / **`midi`**:
  Rung clip JSON (`trem::rung`) and SMF import.
- `crates/trem-mio/` -- **Media I/O** (not the Tokio **`mio`** crate): planar WAV/FLAC today
  (`trem_mio::audio`, `trem_mio::wav`); room for images later. Feature **`audio`** (default **on**).
  On `wasm32` without WASI use in-memory APIs (`from_bytes`, `write_planar_to_vec`, `open_memory` / `done_into_vec`).
- `crates/trem-dsp/` -- Built-in graph nodes (`standard` module), registry
  wiring (`register_standard`, `standard_registry`), and `interfaces` re-exports
  for custom `Node` implementations (see `docs/graph-architecture.md`).
- `crates/trem-rta/` -- Real-time playback host (cpal + rtrb). Feature **`cli`**: `cli_output` (**`StereoOutputCli`**, **`AudioPlayer`**). File output uses **`trem_mio::audio`** (crate **`trem-mio`**).
- `crates/trem-tui/` -- Terminal UI using ratatui + crossterm.

The **`trem-bin`** package (`crates/trem-bin/`) wires libraries into the TUI DAW.
The default patch and pattern live under **`crates/trem-bin/src/demo/`** (`levels.rs` =
mix constants, `graph.rs` = routing, `pattern.rs` = starter grid).

## Commands

```bash
cargo check --workspace          # type-check everything
cargo test --workspace           # run all unit + integration tests (mirrors GitHub Actions CI)
cargo test -p trem --features rung,midi  # Rung + MIDI import tests (also run in CI)
cargo test -p trem-mio           # WAV/FLAC I/O tests
cargo test --workspace --doc     # run doc-test examples
cargo bench -p trem -- --test       # compile-check trem benchmarks
cargo bench -p trem-dsp -- --test   # compile-check trem-dsp benchmarks
cargo doc --workspace --no-deps  # build documentation
cargo run                        # launch the TUI demo (workspace default-member `crates/trem-bin`; same as `trem tui`)
cargo run -p trem-bin            # explicit package (equivalent)
cargo run -- rung import file.mid   # MIDI → Rung JSON (`clip` is a visible alias for `rung`)
cargo run -- rung import tune.mid -o -   # write JSON to stdout
cargo run -- rung edit clip.rung.json  # Rung piano-roll editor (TTY)
cargo check -p trem-mio --examples                 # all `trem-mio` I/O examples (WAV/FLAC)
cargo run -p trem-mio --example save_planar_wav   # planar WAV write (feature `audio` default on `trem-mio`)
cargo run -p trem-dsp --example <name>    # graph + stock DSP examples
cargo check -p trem-rta --features cli              # `StereoOutputCli` only
```

## Workflow

Work items use the **`flow/`** directory (same model as the **ezc** repo). Full rules:
**[flow/README.md](flow/README.md)**.

```
flow/prop/  ->  flow/todo/  ->  flow/plan/  ->  flow/work/  ->  flow/done/
```

- **`flow/prop/`** — Draft proposals (what / why).
- **`flow/todo/`** — Accepted; committed to doing.
- **`flow/plan/`** — Detailed implementation plan (use for non-trivial work).
- **`flow/work/`** — In progress (one item per agent when possible).
- **`flow/done/`** — Complete; add **Lessons learned**. This is the **decision log for shipped work items**.

Top-level **`docs/`** is **not** a flow stage — it holds ongoing user and architecture
docs (`install.md`, `graph-architecture.md`, `modes/`, …).

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
- **Design velocity**: This project is young and changes fast. Prefer **one**
  clear design and **break + rewrite** when something better appears — not
  long-lived parallel APIs, compatibility shims, or “legacy until migrated”
  layers unless there is an explicit, time-bounded reason.

## Important Areas

- `demos/` -- Placeholder for future demos (see `demos/README.md`).
- `crates/trem-bin/src/demo/` -- Default patch for the binary: `levels.rs` (mix), `graph.rs`
  (routing), `pattern.rs` (grid). See `crates/trem-bin/src/demo/README.md`.
- `crates/trem/src/graph.rs` -- The core `Graph` and `Node` trait (`PrepareEnv` / `Node::prepare`, pooled input mix).
  `Graph` implements `Node`, enabling recursive nesting. See [docs/graph-architecture.md](docs/graph-architecture.md).
  See `docs/graph-architecture.md` for prepare/block sizing behavior.
- `crates/trem-mio/` -- Media codecs and file helpers (`audio`, `wav`); depends on **`trem`** for `signal` types only.
- `crates/trem-dsp/src/standard/` -- Stock nodes (oscillators, filters,
  dynamics, effects, drum synths, etc.).
- `crates/trem/src/registry.rs` -- Runtime node factory mapping tags to
  constructors (populate via `trem_dsp::register_standard`).
- `crates/trem/src/tree.rs` -- Recursive temporal trees for rhythm.
- `crates/trem/src/pitch.rs` -- Xenharmonic pitch and tuning systems.
- `crates/trem-rta/src/driver.rs` -- Audio thread loop.
- `crates/trem-tui/src/app.rs` -- TUI application state and render loop.
- `crates/trem-tui/src/view/` -- Widget implementations for each TUI pane.
- `docs/tui-testing.md` -- TUI testing: `tests/keyboard_flows.rs`, `tests/widget_labels.rs`,
  `cargo test -p trem-tui`, optional `expect scripts/tui-smoke.expect` on a real terminal.
- `docs/modes/` -- User stories and input specs for fullscreen **editing modes** (pattern roll first);
  `docs/modes/principles.md` is the shared contract for future modes.
- `crates/trem/src/rung/` -- Rung interchange (`RungFile`, `Clip`; MIDI import behind crate features **`rung`** + **`midi`**).
- `crates/trem-bin/src/main.rs` -- CLI + demo graph/pattern + TUI launch.

## Validation

Before completing any work item:

1. `cargo check --workspace` -- no errors
2. `cargo test --workspace` -- all tests pass
3. `cargo test --workspace --doc` -- doc examples pass
4. If DSP or graph changes: `cargo bench -p trem -- --test` and
   `cargo bench -p trem-dsp -- --test` -- benchmarks compile

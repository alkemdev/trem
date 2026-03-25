# Contributing

- **Run & test:** [docs/install.md](docs/install.md), then `cargo test --workspace` and `cargo fmt --all -- --check`.
- **Project rules & workflow:** [AGENTS.md](AGENTS.md) and [flow/README.md](flow/README.md) (`flow/prop` → `todo` → `plan` → `work` → `done`; top-level `docs/` is user/architecture reference, not a flow stage).
- **TUI changes:** extend [crates/trem-tui/tests/keyboard_flows.rs](crates/trem-tui/tests/keyboard_flows.rs) when bindings change; see [docs/tui-testing.md](docs/tui-testing.md).
- **License:** MIT (see [LICENSE](LICENSE)).

Pull requests welcome; open an issue first for large design shifts.

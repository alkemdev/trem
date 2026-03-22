# Contributing

- **Run & test:** [docs/install.md](docs/install.md), then `cargo test --workspace` and `cargo fmt --all -- --check`.
- **Project rules & workflow:** [AGENTS.md](AGENTS.md) (scope, commands, `prop/` → `todo/` → `work/` → `docs/`).
- **TUI changes:** extend [crates/trem-tui/tests/keyboard_flows.rs](crates/trem-tui/tests/keyboard_flows.rs) when bindings change; see [docs/tui-testing.md](docs/tui-testing.md).
- **License:** MIT (see [LICENSE](LICENSE)).

Pull requests welcome; open an issue first for large design shifts.

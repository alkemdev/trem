# Testing the trem TUI

Strategy: **fast deterministic tests in CI** (keyboard + widget buffers) plus an **optional PTY smoke** on a real machine.

## 1. Keyboard flows (integration) — **primary contract**

All routing goes through [`handle_key`](../crates/trem-tui/src/input.rs) + [`InputContext`](../crates/trem-tui/src/input.rs).

| File | Purpose |
|------|---------|
| [`crates/trem-tui/tests/keyboard_flows.rs`](../crates/trem-tui/tests/keyboard_flows.rs) | User-visible flows: global chords, help overlay, sequencer NAV/EDIT, graph NAV/EDIT, Ctrl chords |

The module doc at the top of `keyboard_flows.rs` has a **flow map table** — update it when bindings change.

```bash
cargo test -p trem-tui --test keyboard_flows
cargo test -p trem-tui                  # all trem-tui tests (unit + integration)
```

**CI / agents:** extend `keyboard_flows.rs` when you add a chord or mode; no terminal required.

## 2. Widget labels (integration)

| File | Purpose |
|------|---------|
| [`crates/trem-tui/tests/widget_labels.rs`](../crates/trem-tui/tests/widget_labels.rs) | **`HelpOverlay`**, **Info** sidebar (incl. perf at bottom), transport tabs |

```bash
cargo test -p trem-tui --test widget_labels
```

| File | Purpose |
|------|---------|
| [`crates/trem-tui/src/view/transport.rs`](../crates/trem-tui/src/view/transport.rs) (`#[cfg(test)]`) | Transport row: `SEQ`, `GRAPH`, `[…]` active tab |

```bash
cargo test -p trem-tui transport::
```

Other crates: graph scroll tests, spectrum tests, etc. — `cargo test -p trem-tui`.

Layout: `App::draw` uses `info_sidebar_width` (unit-tested in `app::sidebar_width_tests`) so narrow terminals still leave space for the main editor.

## 3. PTY smoke (optional, local)

Runs the **real** binary under `expect`, waits, sends **`q`**.

```bash
expect scripts/tui-smoke.expect
```

Needs a normal environment (TTY + audio stack). **Do not** rely on this in sandboxed CI; use sections 1–2 there.

## 4. Full workspace

```bash
cargo test --workspace
```

## 5. GitHub Actions

[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) runs `cargo test --workspace` and `cargo fmt --check` on push/PR to `main` / `master`.

## 6. AI agents

Prefer **`keyboard_flows`** + **`widget_labels`** over scraping PTY output (often escape-heavy and environment-dependent).

When changing UX, add one focused test per flow you care about breaking.

## 7. Dead code

Orphan TUI files are removed rather than kept untested (e.g. old arrange stub); new editors start from the roadmap when there is engine data to drive them.

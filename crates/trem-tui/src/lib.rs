//! # trem-tui — terminal user interface
//!
//! Ratatui-based **modal** TUI: **Sequencer** (step grid; **`e`** or **Enter** enters note edit)
//! and **Graph** (nested DAG + params); transport tabs **SEQ** / **GRAPH**. **`?`** opens the full keymap.
//! Info column (cursor/project/keys + **perf** at bottom: CPU, RSS, meters). Future editors:
//! `docs/tui-editor-roadmap.md`. Testing: `docs/tui-testing.md` (integration tests
//! `keyboard_flows`, `widget_labels`; optional `scripts/tui-smoke.expect`).
//!
//! The [`App`] struct owns all UI state and communicates with the audio engine
//! via a [`trem_cpal::Bridge`].

pub mod app;
pub mod editor;
pub mod input;
pub mod pattern_roll;
pub mod project;
pub mod theme;
pub mod view;

pub use app::App;

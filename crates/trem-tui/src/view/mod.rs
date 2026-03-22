//! Ratatui widget implementations for each TUI pane.
//!
//! Each sub-module exports a single `Widget` struct that borrows application
//! state and renders one region of the terminal (pattern grid, audio graph,
//! transport bar, etc.).

pub mod graph;
pub mod help;
pub mod info;
pub mod perf;

pub use perf::HostStatsSnapshot;
pub mod pattern;
pub mod scope;
pub mod spectrum;
pub mod transport;

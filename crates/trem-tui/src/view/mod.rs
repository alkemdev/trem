//! Ratatui widget implementations for each TUI pane.
//!
//! Each sub-module exports a single `Widget` struct that borrows application
//! state and renders one region of the terminal (pattern grid, audio graph,
//! transport bar, etc.).

pub mod context;
pub mod fullscreen;
pub mod graph;
pub mod help;
pub mod info;
pub mod overview;
pub mod perf;

pub use perf::HostStatsSnapshot;
pub mod pattern;
pub mod scope;
pub mod spectrum;
pub mod status;
pub mod transport;

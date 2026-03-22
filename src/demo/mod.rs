//! Default **demo** content for the `trem` binary: routing graph, mix constants, and starter pattern.
//!
//! - [`levels`] — all channel / bus / master numeric defaults in one place.
//! - [`graph`] — nested buses, instruments, and FX chain.
//! - [`pattern`] — 32-step grid.

pub mod graph;
pub mod levels;
pub mod pattern;

pub use graph::build_graph;
pub use pattern::build_pattern;

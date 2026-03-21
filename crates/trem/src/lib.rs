//! # trem — mathematical music engine
//!
//! Core library for exact-arithmetic music composition and audio processing.
//! No I/O dependencies — this crate compiles to WASM and renders offline.
//!
//! ## Key modules
//!
//! - [`math`] — Exact rational arithmetic (`Rational` = p/q in lowest terms)
//! - [`pitch`] — Pitch representation, scales, and tuning systems (EDO, just intonation, free)
//! - [`time`] — Beat-accurate durations and time spans
//! - [`event`] — Note events (scale degree + octave + velocity) and graph events (frequency + voice)
//! - [`tree`] — Recursive temporal trees for rhythmic subdivision (`Seq`, `Par`, `Weight`)
//! - [`grid`] — 2D step sequencer grid (rows = steps, columns = voices)
//! - [`graph`] — Audio processing DAG with typed processor nodes
//! - [`dsp`] — Built-in processors: oscillators, envelopes, filters, effects, drum synths
//! - [`euclidean`] — Euclidean rhythm generation (Toussaint 2005)
//! - [`render`] — Offline rendering of trees/grids through audio graphs

pub mod dsp;
pub mod euclidean;
pub mod event;
pub mod graph;
pub mod grid;
pub mod math;
pub mod pitch;
pub mod render;
pub mod time;
pub mod tree;

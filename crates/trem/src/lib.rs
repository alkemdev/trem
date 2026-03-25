//! # trem — mathematical music engine
//!
//! Core library for exact-arithmetic music composition and audio processing.
//! The default build stays lean (no JSON/MIDI clip stack) so this crate can target WASM and
//! offline rendering; enable features as needed.
//!
//! ## Key modules
//!
//! - [`math`] — Exact rational arithmetic (`Rational` = p/q in lowest terms)
//! - [`pitch`] — Pitch representation, scales, and tuning systems (EDO, just intonation, free)
//! - [`time`] — Beat-accurate durations and time spans
//! - [`event`] — Note events (scale degree + octave + velocity) and graph events (frequency + voice)
//! - [`tree`] — Recursive temporal trees for rhythmic subdivision (`Seq`, `Par`, `Weight`)
//! - [`grid`] — 2D step sequencer grid (rows = steps, columns = voices)
//! - [`graph`] — Audio [`Graph`] and [`graph::Node`] trait ([`graph::PrepareEnv`] / [`graph::Node::prepare`], pooled I/O)
//! - **trem-dsp** (crate `trem_dsp`) — Stock [`graph::Node`] implementations: oscillators,
//!   envelopes, filters, distortion, effects, drum synths, sidechain duck (`SidechainDucker` / `duk`),
//!   nested voices (`analog_voice`, `lead_voice`), and the `interfaces` re-export module for
//!   custom node authors.
//! - [`euclidean`] — Euclidean rhythm generation (Toussaint 2005)
//! - [`render`] — Offline rendering of trees/grids through audio graphs ([`render::render_captures`],
//!   [`render::loop_timed_events`] for repeating patterns)
//! - [`wav`] (feature **`wav`**) — IEEE float WAV file write ([`wav::write_stereo_wav_f32`])
//! - **`rung`** (features **`rung`** / **`midi`**) — Rung clip JSON (`Clip`, `RungFile`) and optional
//!   SMF import (`midi` submodule)

pub mod euclidean;
pub mod event;
pub mod graph;
pub mod grid;
pub mod math;
pub mod pitch;
pub mod registry;
pub mod render;
pub mod time;
pub mod tree;

#[cfg(feature = "rung")]
pub mod rung;

#[cfg(feature = "wav")]
pub mod wav;

//! # trem-tui — terminal user interface
//!
//! Ratatui-based TUI for trem. Provides a pattern sequencer view, an audio
//! graph view with inline parameter editing, a transport bar, waveform scope,
//! and contextual key hints.
//!
//! The [`App`] struct owns all UI state and communicates with the audio engine
//! via a [`trem_cpal::Bridge`].

pub mod app;
pub mod input;
pub mod theme;
pub mod view;

pub use app::App;

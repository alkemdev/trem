//! # trem-rta — real-time audio playback host
//!
//! Drives a [`trem::graph::Graph`] from a cpal output stream with lock-free
//! command/notification bridging between the audio thread and the UI.
//!
//! The [`Bridge`] / [`AudioBridge`] pair communicates via an [`rtrb`] ring
//! buffer. The UI sends [`Command`]s (play, pause, stop, set parameter, load events),
//! and the audio callback sends back [`Notification`]s (beat position, peak meters).
//!
//! [`preview`] plays pre-rendered stereo buffers once (no graph) — useful for small examples.

pub mod bridge;
pub mod driver;
pub mod preview;

pub use bridge::{
    create_bridge, AudioBridge, Bridge, Command, Notification, ScopeFocus, ScopeSnapshot,
};
pub use driver::AudioEngine;

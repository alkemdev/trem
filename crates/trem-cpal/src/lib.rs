//! # trem-cpal — real-time audio backend
//!
//! Drives a [`trem::graph::Graph`] from a cpal output stream with lock-free
//! command/notification bridging between the audio thread and the UI.
//!
//! The [`Bridge`] / [`AudioBridge`] pair communicates via an [`rtrb`] ring
//! buffer. The UI sends [`Command`]s (play, pause, stop, set parameter, load events),
//! and the audio callback sends back [`Notification`]s (beat position, peak meters).

pub mod bridge;
pub mod driver;

pub use bridge::{
    create_bridge, AudioBridge, Bridge, Command, Notification, ScopeFocus, ScopeSnapshot,
};
pub use driver::AudioEngine;

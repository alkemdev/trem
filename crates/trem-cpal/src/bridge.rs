//! Lock-free UI ↔ audio communication using [`rtrb`] ring buffers.
//!
//! The UI thread holds [`Bridge`] and pushes [`Command`]s; the audio callback holds [`AudioBridge`]
//! and pushes [`Notification`]s. Full queues drop the newest push silently (same as `send` ignoring errors).

use trem::event::TimedEvent;

/// Message from the UI/control thread to the realtime audio callback.
#[derive(Debug)]
pub enum Command {
    /// Start pattern playback from the current playhead.
    Play,
    /// Stop playback, reset playhead, and clear graph voice state.
    Stop,
    /// Set tempo in beats per minute for position reporting and scheduling.
    SetBpm(f64),
    /// Replace the repeating pattern used when playing.
    LoadEvents(Vec<TimedEvent>),
    /// Trigger a note on the given synth voice for the current audio block.
    NoteOn {
        frequency: f64,
        velocity: f64,
        voice: u32,
    },
    /// Release the given voice (matches [`Command::NoteOn`]).
    NoteOff { voice: u32 },
    /// Set a node parameter on the audio graph (live tweak from UI).
    SetParam {
        node: u32,
        param_id: u32,
        value: f64,
    },
}

/// Message from the audio thread back to the UI (scope, meters, transport).
#[derive(Debug)]
pub enum Notification {
    /// Approximate playhead position in beats (throttled in the callback).
    Position { beat: f64 },
    /// Recent mono samples for the oscilloscope view.
    ScopeData(Vec<f32>),
    /// Peak levels since the last meter notification (per channel).
    Meter { peak_l: f32, peak_r: f32 },
    /// Playback stopped from the audio side (e.g. device error path).
    Stopped,
}

/// UI-side endpoints: send commands, receive notifications.
pub struct Bridge {
    pub cmd_tx: rtrb::Producer<Command>,
    pub notif_rx: rtrb::Consumer<Notification>,
}

impl Bridge {
    /// Enqueue a command; drops silently if the command ring is full.
    pub fn send(&mut self, cmd: Command) {
        let _ = self.cmd_tx.push(cmd);
    }

    /// Returns the next notification, if any, without blocking.
    pub fn try_recv(&mut self) -> Option<Notification> {
        self.notif_rx.pop().ok()
    }
}

/// Audio-callback-side endpoints: receive commands, send notifications.
pub struct AudioBridge {
    pub cmd_rx: rtrb::Consumer<Command>,
    pub notif_tx: rtrb::Producer<Notification>,
}

/// Splits into UI and audio halves; command and notification rings each hold up to `capacity` items.
pub fn create_bridge(capacity: usize) -> (Bridge, AudioBridge) {
    let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(capacity);
    let (notif_tx, notif_rx) = rtrb::RingBuffer::new(capacity);
    (
        Bridge { cmd_tx, notif_rx },
        AudioBridge { cmd_rx, notif_tx },
    )
}

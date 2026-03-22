//! Lock-free UI ↔ audio communication using [`rtrb`] ring buffers.
//!
//! The UI thread holds [`Bridge`] and pushes [`Command`]s; the audio callback holds [`AudioBridge`]
//! and pushes [`Notification`]s. Full queues drop the newest push silently (same as `send` ignoring errors).

use trem::event::TimedEvent;

/// Where the UI wants stereo scope / spectrum taps to come from.
#[derive(Debug, Clone)]
pub enum ScopeFocus {
    /// Pattern view: instrument submix vs master output (patch-level).
    PatchBuses,
    /// Graph view: summed inputs vs outputs of the highlighted node (any nesting level).
    GraphNode {
        /// Same prefix as [`Command::SetParam`] when editing inside nested graphs.
        graph_path: Vec<u32>,
        /// Node id within [`graph_path`](ScopeFocus::GraphNode::graph_path)’s graph.
        node: u32,
    },
}

/// Message from the UI/control thread to the realtime audio callback.
#[derive(Debug)]
pub enum Command {
    /// Start pattern playback from the current playhead.
    Play,
    /// Pause playback at the current playhead and silence the graph (DSP reset).
    Pause,
    /// Stop playback, reset playhead to the loop start, and clear graph voice state.
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
    /// `path` identifies the node through nested graphs.
    SetParam {
        path: Vec<u32>,
        param_id: u32,
        value: f64,
    },
    /// Scope/spectrum left = “in”, right = “out” (see [`ScopeFocus`]).
    SetScopeFocus(ScopeFocus),
}

/// Stereo interleaved audio snippet for scopes / spectrum (L,R pairs).
#[derive(Debug, Clone)]
pub struct ScopeSnapshot {
    /// Right-hand / “out” pane: master output in [`ScopeFocus::PatchBuses`], or the focused
    /// node’s output in [`ScopeFocus::GraphNode`].
    pub master: Vec<f32>,
    /// Left-hand / “in” pane: instrument submix before master FX in patch mode, or the focused
    /// node’s summed inputs in graph mode.
    pub graph_in: Vec<f32>,
}

/// Message from the audio thread back to the UI (scope, meters, transport).
#[derive(Debug)]
pub enum Notification {
    /// Approximate playhead position in beats (throttled in the callback).
    Position { beat: f64 },
    /// Recent stereo samples for waveform / spectrum (see [`ScopeSnapshot`]).
    ScopeData(ScopeSnapshot),
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

//! Keyboard routing: maps crossterm keys to high-level [`Action`]s for each [`Mode`].
//!
//! [`View`] is not passed here; callers interpret the same action differently per view.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Major screen: step sequencer vs. node graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Row/column grid of notes and voices.
    Pattern,
    /// Synth graph navigation and parameter edit target.
    Graph,
}

impl View {
    /// Short label for the transport/header strip.
    pub fn label(self) -> &'static str {
        match self {
            View::Pattern => "PATTERN",
            View::Graph => "GRAPH",
        }
    }

    /// Switches between pattern and graph.
    pub fn next(self) -> Self {
        match self {
            View::Pattern => View::Graph,
            View::Graph => View::Pattern,
        }
    }

    /// Fixed ordering for UI lists that enumerate views.
    pub const ALL: [View; 2] = [View::Pattern, View::Graph];
}

/// Whether arrow/vim keys move the grid/graph or edit parameters / enter notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Navigation and transport-focused bindings.
    Normal,
    /// Note entry on the pattern grid, coarse param nudge on the graph (see app handler).
    Edit,
}

impl Mode {
    /// Short label for the status line.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NAVIGATE",
            Mode::Edit => "EDIT",
        }
    }
}

/// Semantic user intent produced from a single key press (may be ignored per view in the app).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    /// Tab: rotate pattern ↔ graph.
    CycleView,
    /// `e` / Esc in edit: enter or leave edit mode.
    ToggleEdit,
    /// Space: start/stop pattern playback.
    TogglePlay,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    /// Scale degree from home-row or digit keys (0–9, z/m column) while in edit mode.
    NoteInput(i32),
    DeleteNote,
    OctaveUp,
    OctaveDown,
    /// `+`/`-`; in graph edit the app may treat these as fine param nudge instead of BPM.
    BpmUp,
    BpmDown,
    /// `f`: Euclidean rhythm fill for the current voice column.
    EuclideanFill,
    /// `r`: randomize notes on the current voice.
    RandomizeVoice,
    /// `t`: reverse step order for the current voice.
    ReverseVoice,
    /// `,` / `.`: rotate the current voice pattern along steps.
    ShiftVoiceLeft,
    ShiftVoiceRight,
    /// `w` / `q` in edit: bump note velocity up or down.
    VelocityUp,
    VelocityDown,
}

/// Maps a key to an action for the given mode; release events and unbound keys yield `None`.
pub fn handle_key(key: KeyEvent, mode: &Mode) -> Option<Action> {
    if key.kind == KeyEventKind::Release {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') | KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Tab => return Some(Action::CycleView),
        KeyCode::Char(' ') => return Some(Action::TogglePlay),
        KeyCode::Up => return Some(Action::MoveUp),
        KeyCode::Down => return Some(Action::MoveDown),
        KeyCode::Left => return Some(Action::MoveLeft),
        KeyCode::Right => return Some(Action::MoveRight),
        KeyCode::Char('+') | KeyCode::Char('=') => return Some(Action::BpmUp),
        KeyCode::Char('-') => return Some(Action::BpmDown),
        KeyCode::Char('[') => return Some(Action::OctaveDown),
        KeyCode::Char(']') => return Some(Action::OctaveUp),
        KeyCode::Esc if *mode == Mode::Edit => return Some(Action::ToggleEdit),
        _ => {}
    }

    match mode {
        Mode::Normal => match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('e') => Some(Action::ToggleEdit),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('j') => Some(Action::MoveDown),
            _ => None,
        },
        Mode::Edit => match key.code {
            KeyCode::Delete | KeyCode::Backspace => Some(Action::DeleteNote),
            KeyCode::Char('z') => Some(Action::NoteInput(0)),
            KeyCode::Char('s') => Some(Action::NoteInput(1)),
            KeyCode::Char('x') => Some(Action::NoteInput(2)),
            KeyCode::Char('d') => Some(Action::NoteInput(3)),
            KeyCode::Char('c') => Some(Action::NoteInput(4)),
            KeyCode::Char('v') => Some(Action::NoteInput(5)),
            KeyCode::Char('g') => Some(Action::NoteInput(6)),
            KeyCode::Char('b') => Some(Action::NoteInput(7)),
            KeyCode::Char('h') => Some(Action::NoteInput(8)),
            KeyCode::Char('n') => Some(Action::NoteInput(9)),
            KeyCode::Char('j') => Some(Action::NoteInput(10)),
            KeyCode::Char('m') => Some(Action::NoteInput(11)),
            KeyCode::Char(ch @ '0'..='9') => Some(Action::NoteInput(ch as i32 - '0' as i32)),
            KeyCode::Char('f') => Some(Action::EuclideanFill),
            KeyCode::Char('r') => Some(Action::RandomizeVoice),
            KeyCode::Char('t') => Some(Action::ReverseVoice),
            KeyCode::Char(',') => Some(Action::ShiftVoiceLeft),
            KeyCode::Char('.') => Some(Action::ShiftVoiceRight),
            KeyCode::Char('w') => Some(Action::VelocityUp),
            KeyCode::Char('q') => Some(Action::VelocityDown),
            _ => None,
        },
    }
}

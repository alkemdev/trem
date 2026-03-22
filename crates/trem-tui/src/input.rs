//! Keyboard routing for **modal editors**: pattern grid and signal graph. Each editor
//! owns a key family; [`InputContext`] disambiguates global chords (Tab, `?`, Esc) vs
//! nested-graph exit. **SEQ navigate:** **Enter** opens the fullscreen piano roll; **`e`** toggles grid note edit.
//! Future editors: see repository `docs/tui-editor-roadmap.md`.
//!
//! Full bindings: **`?`** help overlay. Sidebar shows a short **popular** subset only.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Which **editor surface** is focused (modal). Tab cycles this list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Editor {
    /// Step sequencer: time × voice.
    Pattern,
    /// Nested audio graph + parameters.
    Graph,
}

impl Editor {
    /// Short transport tab label.
    pub fn tab_label(self) -> &'static str {
        match self {
            Editor::Pattern => "SEQ",
            Editor::Graph => "GRAPH",
        }
    }

    /// Sidebar / docs: human name.
    pub fn title(self) -> &'static str {
        match self {
            Editor::Pattern => "Sequencer",
            Editor::Graph => "Graph",
        }
    }

    /// One-line intent for the modal system.
    pub fn intent(self) -> &'static str {
        match self {
            Editor::Pattern => "time · voices",
            Editor::Graph => "signal · routing",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Editor::Pattern => Editor::Graph,
            Editor::Graph => Editor::Pattern,
        }
    }

    pub const ALL: [Editor; 2] = [Editor::Pattern, Editor::Graph];
}

/// State needed to route keys without ambiguity.
#[derive(Debug, Clone, Copy)]
pub struct InputContext<'a> {
    pub editor: Editor,
    pub mode: &'a Mode,
    /// True when graph editor is inside a nested graph (`graph_path` non-empty).
    pub graph_is_nested: bool,
    pub help_open: bool,
}

/// Bottom pane visualizer: stereo waveform or frequency spectrum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomPane {
    Waveform,
    Spectrum,
}

impl BottomPane {
    pub fn next(self) -> Self {
        match self {
            BottomPane::Waveform => BottomPane::Spectrum,
            BottomPane::Spectrum => BottomPane::Waveform,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BottomPane::Waveform => "SCOPE",
            BottomPane::Spectrum => "SPECTRUM",
        }
    }
}

/// Within an editor: navigate vs change values / paint notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Edit,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NAV",
            Mode::Edit => "EDIT",
        }
    }
}

/// Semantic user intent from one key (may be ignored in [`crate::App::handle_action`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    /// Tab: next editor.
    CycleEditor,
    /// `e` / Esc: toggle edit mode (when not in help / graph-exit).
    ToggleEdit,
    /// SEQ navigate: **Enter** — fullscreen MIDI piano roll (apply on Esc).
    OpenPatternRoll,
    TogglePlay,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    NoteInput(i32),
    DeleteNote,
    OctaveUp,
    OctaveDown,
    BpmUp,
    BpmDown,
    ParamFineUp,
    ParamFineDown,
    EuclideanFill,
    RandomizeVoice,
    ReverseVoice,
    ShiftVoiceLeft,
    ShiftVoiceRight,
    VelocityUp,
    VelocityDown,
    GateCycle,
    Undo,
    Redo,
    SwingUp,
    SwingDown,
    SaveProject,
    LoadProject,
    CycleBottomPane,
    EnterGraph,
    ExitGraph,
    /// `?` toggles full key reference overlay.
    ToggleHelp,
}

/// Maps a key to an action; release events and unbound keys yield `None`.
pub fn handle_key(key: KeyEvent, ctx: &InputContext<'_>) -> Option<Action> {
    if key.kind == KeyEventKind::Release {
        return None;
    }

    if ctx.help_open {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') => Some(Action::ToggleHelp),
            _ => None,
        };
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') | KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('s') => Some(Action::SaveProject),
            KeyCode::Char('o') => Some(Action::LoadProject),
            KeyCode::Char('z') => Some(Action::Undo),
            KeyCode::Char('y') => Some(Action::Redo),
            _ => None,
        };
    }

    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Left => return Some(Action::ParamFineDown),
            KeyCode::Right => return Some(Action::ParamFineUp),
            KeyCode::Char('U') => return Some(Action::Redo),
            _ => {}
        }
    }

    match key.code {
        KeyCode::Tab => return Some(Action::CycleEditor),
        KeyCode::Char('?') => return Some(Action::ToggleHelp),
        KeyCode::Char(' ') => return Some(Action::TogglePlay),
        KeyCode::Up => return Some(Action::MoveUp),
        KeyCode::Down => return Some(Action::MoveDown),
        KeyCode::Left => return Some(Action::MoveLeft),
        KeyCode::Right => return Some(Action::MoveRight),
        KeyCode::Char('+') | KeyCode::Char('=') => return Some(Action::BpmUp),
        KeyCode::Char('-') => return Some(Action::BpmDown),
        KeyCode::Char('[') => return Some(Action::OctaveDown),
        KeyCode::Char(']') => return Some(Action::OctaveUp),
        KeyCode::Char('{') => return Some(Action::SwingDown),
        KeyCode::Char('}') => return Some(Action::SwingUp),
        KeyCode::Char('`') => return Some(Action::CycleBottomPane),
        KeyCode::Esc if *ctx.mode == Mode::Edit => return Some(Action::ToggleEdit),
        KeyCode::Esc
            if *ctx.mode == Mode::Normal && ctx.editor == Editor::Graph && ctx.graph_is_nested =>
        {
            return Some(Action::ExitGraph);
        }
        _ => {}
    }

    match ctx.editor {
        Editor::Pattern => pattern_keys(key.code, ctx.mode),
        Editor::Graph => graph_keys(key.code, ctx.mode),
    }
}

fn pattern_keys(code: KeyCode, mode: &Mode) -> Option<Action> {
    match mode {
        Mode::Normal => match code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Enter => Some(Action::OpenPatternRoll),
            KeyCode::Char('e') => Some(Action::ToggleEdit),
            KeyCode::Char('u') => Some(Action::Undo),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('j') => Some(Action::MoveDown),
            _ => None,
        },
        Mode::Edit => match code {
            KeyCode::Enter => Some(Action::ToggleEdit),
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
            KeyCode::Char('a') => Some(Action::GateCycle),
            _ => None,
        },
    }
}

fn graph_keys(code: KeyCode, mode: &Mode) -> Option<Action> {
    match mode {
        Mode::Normal => match code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('e') => Some(Action::ToggleEdit),
            KeyCode::Char('u') => Some(Action::Undo),
            KeyCode::Char('h') => Some(Action::MoveLeft),
            KeyCode::Char('l') => Some(Action::MoveRight),
            KeyCode::Char('k') => Some(Action::MoveUp),
            KeyCode::Char('j') => Some(Action::MoveDown),
            KeyCode::Enter => Some(Action::EnterGraph),
            _ => None,
        },
        Mode::Edit => None,
    }
}

//! Keyboard routing for **modal editors**: pattern grid and signal graph. Each editor
//! owns a key family; [`InputContext`] disambiguates global chords (Tab, `?`, Esc) vs
//! nested-graph exit. **SEQ navigate:** **Enter** opens the fullscreen piano roll; **`e`** toggles grid note edit.
//! Future editors: see repository `docs/tui-editor-roadmap.md`.
//!
//! Full bindings: **`?`** help overlay. Sidebar shows a short **popular** subset only.

#[cfg(not(target_arch = "wasm32"))]
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

/// Backend-neutral key code used by [`handle_key_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKeyCode {
    Char(char),
    F(u8),
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Tab,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Esc,
    Unknown,
}

/// Backend-neutral key event used by [`handle_key_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppKeyEvent {
    pub code: AppKeyCode,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Maps a backend-neutral key to an action; unbound keys yield `None`.
pub fn handle_key_event(key: AppKeyEvent, ctx: &InputContext<'_>) -> Option<Action> {
    if ctx.help_open {
        return match key.code {
            AppKeyCode::Esc | AppKeyCode::Char('?') => Some(Action::ToggleHelp),
            _ => None,
        };
    }

    if key.ctrl {
        return match key.code {
            AppKeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'c') => Some(Action::Quit),
            AppKeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'q') => Some(Action::Quit),
            AppKeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'s') => Some(Action::SaveProject),
            AppKeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'o') => Some(Action::LoadProject),
            AppKeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'z') => Some(Action::Undo),
            AppKeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'y') => Some(Action::Redo),
            _ => None,
        };
    }

    if key.shift {
        match key.code {
            AppKeyCode::Left => return Some(Action::ParamFineDown),
            AppKeyCode::Right => return Some(Action::ParamFineUp),
            AppKeyCode::Char('U') | AppKeyCode::Char('u') => return Some(Action::Redo),
            _ => {}
        }
    }

    match key.code {
        AppKeyCode::Tab => return Some(Action::CycleEditor),
        AppKeyCode::Char('?') => return Some(Action::ToggleHelp),
        AppKeyCode::Char(' ') => return Some(Action::TogglePlay),
        AppKeyCode::Up => return Some(Action::MoveUp),
        AppKeyCode::Down => return Some(Action::MoveDown),
        AppKeyCode::Left => return Some(Action::MoveLeft),
        AppKeyCode::Right => return Some(Action::MoveRight),
        AppKeyCode::Char('+') | AppKeyCode::Char('=') => return Some(Action::BpmUp),
        AppKeyCode::Char('-') => return Some(Action::BpmDown),
        AppKeyCode::Char('[') => return Some(Action::OctaveDown),
        AppKeyCode::Char(']') => return Some(Action::OctaveUp),
        AppKeyCode::Char('{') => return Some(Action::SwingDown),
        AppKeyCode::Char('}') => return Some(Action::SwingUp),
        AppKeyCode::Char('`') => return Some(Action::CycleBottomPane),
        AppKeyCode::Esc if *ctx.mode == Mode::Edit => return Some(Action::ToggleEdit),
        AppKeyCode::Esc
            if *ctx.mode == Mode::Normal && ctx.editor == Editor::Graph && ctx.graph_is_nested =>
        {
            return Some(Action::ExitGraph);
        }
        _ => {}
    }

    match ctx.editor {
        Editor::Pattern => pattern_keys(&key, ctx.mode),
        Editor::Graph => graph_keys(&key, ctx.mode),
    }
}

fn pattern_keys(key: &AppKeyEvent, mode: &Mode) -> Option<Action> {
    match mode {
        Mode::Normal => match key.code {
            AppKeyCode::Char('q') => Some(Action::Quit),
            AppKeyCode::Enter => Some(Action::OpenPatternRoll),
            AppKeyCode::Char('e') => Some(Action::ToggleEdit),
            AppKeyCode::Char('u') => Some(Action::Undo),
            AppKeyCode::Char('h') => Some(Action::MoveLeft),
            AppKeyCode::Char('l') => Some(Action::MoveRight),
            AppKeyCode::Char('k') => Some(Action::MoveUp),
            AppKeyCode::Char('j') => Some(Action::MoveDown),
            _ => None,
        },
        Mode::Edit => match key.code {
            AppKeyCode::Enter => Some(Action::ToggleEdit),
            AppKeyCode::Delete | AppKeyCode::Backspace => Some(Action::DeleteNote),
            AppKeyCode::Char('z') => Some(Action::NoteInput(0)),
            AppKeyCode::Char('s') => Some(Action::NoteInput(1)),
            AppKeyCode::Char('x') => Some(Action::NoteInput(2)),
            AppKeyCode::Char('d') => Some(Action::NoteInput(3)),
            AppKeyCode::Char('c') => Some(Action::NoteInput(4)),
            AppKeyCode::Char('v') => Some(Action::NoteInput(5)),
            AppKeyCode::Char('g') => Some(Action::NoteInput(6)),
            AppKeyCode::Char('b') => Some(Action::NoteInput(7)),
            AppKeyCode::Char('h') => Some(Action::NoteInput(8)),
            AppKeyCode::Char('n') => Some(Action::NoteInput(9)),
            AppKeyCode::Char('j') => Some(Action::NoteInput(10)),
            AppKeyCode::Char('m') => Some(Action::NoteInput(11)),
            AppKeyCode::Char(ch @ '0'..='9') => Some(Action::NoteInput(ch as i32 - '0' as i32)),
            AppKeyCode::Char('f') => Some(Action::EuclideanFill),
            AppKeyCode::Char('r') => Some(Action::RandomizeVoice),
            AppKeyCode::Char('t') => Some(Action::ReverseVoice),
            AppKeyCode::Char(',') => Some(Action::ShiftVoiceLeft),
            AppKeyCode::Char('.') => Some(Action::ShiftVoiceRight),
            AppKeyCode::Char('w') => Some(Action::VelocityUp),
            AppKeyCode::Char('q') => Some(Action::VelocityDown),
            AppKeyCode::Char('a') => Some(Action::GateCycle),
            _ => None,
        },
    }
}

fn graph_keys(key: &AppKeyEvent, mode: &Mode) -> Option<Action> {
    match mode {
        Mode::Normal => match key.code {
            AppKeyCode::Char('q') => Some(Action::Quit),
            AppKeyCode::Char('e') => Some(Action::ToggleEdit),
            AppKeyCode::Char('u') => Some(Action::Undo),
            AppKeyCode::Char('h') => Some(Action::MoveLeft),
            AppKeyCode::Char('l') => Some(Action::MoveRight),
            AppKeyCode::Char('k') => Some(Action::MoveUp),
            AppKeyCode::Char('j') => Some(Action::MoveDown),
            AppKeyCode::Enter => Some(Action::EnterGraph),
            _ => None,
        },
        Mode::Edit => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn from_crossterm_key(key: KeyEvent) -> Option<AppKeyEvent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }

    let code = match key.code {
        KeyCode::Backspace => AppKeyCode::Backspace,
        KeyCode::Enter => AppKeyCode::Enter,
        KeyCode::Left => AppKeyCode::Left,
        KeyCode::Right => AppKeyCode::Right,
        KeyCode::Up => AppKeyCode::Up,
        KeyCode::Down => AppKeyCode::Down,
        KeyCode::Tab => AppKeyCode::Tab,
        KeyCode::Delete => AppKeyCode::Delete,
        KeyCode::Home => AppKeyCode::Home,
        KeyCode::End => AppKeyCode::End,
        KeyCode::PageUp => AppKeyCode::PageUp,
        KeyCode::PageDown => AppKeyCode::PageDown,
        KeyCode::Esc => AppKeyCode::Esc,
        KeyCode::F(n) => AppKeyCode::F(n),
        KeyCode::Char(ch) => AppKeyCode::Char(ch),
        _ => AppKeyCode::Unknown,
    };

    Some(AppKeyEvent {
        code,
        ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
        alt: key.modifiers.contains(KeyModifiers::ALT),
        shift: key.modifiers.contains(KeyModifiers::SHIFT),
    })
}

/// Maps a crossterm key to an action; release events and unbound keys yield `None`.
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_key(key: KeyEvent, ctx: &InputContext<'_>) -> Option<Action> {
    let key = from_crossterm_key(key)?;
    handle_key_event(key, ctx)
}

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Pattern,
    Graph,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Pattern => "PATTERN",
            View::Graph => "GRAPH",
        }
    }

    pub fn next(self) -> Self {
        match self {
            View::Pattern => View::Graph,
            View::Graph => View::Pattern,
        }
    }

    pub const ALL: [View; 2] = [View::Pattern, View::Graph];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Edit,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NAVIGATE",
            Mode::Edit => "EDIT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    CycleView,
    ToggleEdit,
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
    EuclideanFill,
    RandomizeVoice,
    ReverseVoice,
    ShiftVoiceLeft,
    ShiftVoiceRight,
    VelocityUp,
    VelocityDown,
}

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

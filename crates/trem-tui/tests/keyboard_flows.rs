//! Repeatable **user-flow** checks for keyboard routing ([`trem_tui::input::handle_key`]).
//!
//! Run: `cargo test -p trem-tui --test keyboard_flows`
//! Or:  `cargo test -p trem-tui` (includes this + unit tests)
//!
//! ## Flow map (keep in sync when bindings change)
//!
//! | Flow | What we assert |
//! |------|----------------|
//! | Global | Tab, Space, `?`, arrows, `+-` BPM, `[]` oct, `` ` `` pane, `{}` swing, Shift+arrows fine |
//! | Global Ctrl | C/Q quit, S save, O load, Z undo, Y redo |
//! | Help | Only Esc / `?` close; Space swallowed |
//! | Sequencer NAV | e edit, hjkl move, q quit, u undo |
//! | Sequencer EDIT | Esc nav, z note, a gate, Del clear |
//! | Graph NAV | e edit, hjkl, Enter inner, Esc up when nested |
//! | Graph EDIT | Esc nav; arrows from global (not graph_keys) |

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use trem_tui::input::{handle_key, Action, Editor, InputContext, Mode};

fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

fn ctx<'a>(editor: Editor, mode: &'a Mode, graph_nested: bool, help: bool) -> InputContext<'a> {
    InputContext {
        editor,
        mode,
        graph_is_nested: graph_nested,
        help_open: help,
    }
}

// --- Global chords (any editor, help closed) ---

#[test]
fn global_tab_cycles_editor() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Tab, KeyModifiers::NONE), &c),
        Some(Action::CycleEditor)
    );
}

#[test]
fn global_space_toggles_play() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Graph, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char(' '), KeyModifiers::NONE), &c),
        Some(Action::TogglePlay)
    );
}

#[test]
fn global_question_toggles_help() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('?'), KeyModifiers::NONE), &c),
        Some(Action::ToggleHelp)
    );
}

#[test]
fn global_arrow_keys_move() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Up, KeyModifiers::NONE), &c),
        Some(Action::MoveUp)
    );
    assert_eq!(
        handle_key(press(KeyCode::Left, KeyModifiers::NONE), &c),
        Some(Action::MoveLeft)
    );
}

#[test]
fn global_bpm_octave_swing_backtick() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('+'), KeyModifiers::NONE), &c),
        Some(Action::BpmUp)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('-'), KeyModifiers::NONE), &c),
        Some(Action::BpmDown)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char(']'), KeyModifiers::NONE), &c),
        Some(Action::OctaveUp)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('['), KeyModifiers::NONE), &c),
        Some(Action::OctaveDown)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('}'), KeyModifiers::NONE), &c),
        Some(Action::SwingUp)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('{'), KeyModifiers::NONE), &c),
        Some(Action::SwingDown)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('`'), KeyModifiers::NONE), &c),
        Some(Action::CycleBottomPane)
    );
}

#[test]
fn global_shift_arrows_param_fine() {
    let mode = Mode::Edit;
    let c = ctx(Editor::Graph, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Right, KeyModifiers::SHIFT), &c),
        Some(Action::ParamFineUp)
    );
    assert_eq!(
        handle_key(press(KeyCode::Left, KeyModifiers::SHIFT), &c),
        Some(Action::ParamFineDown)
    );
}

#[test]
fn global_ctrl_chords_project_and_history() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('s'), KeyModifiers::CONTROL), &c),
        Some(Action::SaveProject)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('o'), KeyModifiers::CONTROL), &c),
        Some(Action::LoadProject)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('z'), KeyModifiers::CONTROL), &c),
        Some(Action::Undo)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('y'), KeyModifiers::CONTROL), &c),
        Some(Action::Redo)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('c'), KeyModifiers::CONTROL), &c),
        Some(Action::Quit)
    );
}

#[test]
fn ctrl_shift_u_redo() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('U'), KeyModifiers::SHIFT), &c),
        Some(Action::Redo)
    );
}

// --- Help overlay ---

#[test]
fn help_swallows_space_and_tab() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, true);
    assert_eq!(
        handle_key(press(KeyCode::Char(' '), KeyModifiers::NONE), &c),
        None
    );
    assert_eq!(
        handle_key(press(KeyCode::Tab, KeyModifiers::NONE), &c),
        None
    );
}

#[test]
fn help_esc_and_question_close() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Graph, &mode, false, true);
    assert_eq!(
        handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &c),
        Some(Action::ToggleHelp)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('?'), KeyModifiers::NONE), &c),
        Some(Action::ToggleHelp)
    );
}

// --- Sequencer (pattern) ---

#[test]
fn sequencer_normal_nav_keys() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('e'), KeyModifiers::NONE), &c),
        Some(Action::ToggleEdit)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('h'), KeyModifiers::NONE), &c),
        Some(Action::MoveLeft)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('l'), KeyModifiers::NONE), &c),
        Some(Action::MoveRight)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('k'), KeyModifiers::NONE), &c),
        Some(Action::MoveUp)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('j'), KeyModifiers::NONE), &c),
        Some(Action::MoveDown)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('q'), KeyModifiers::NONE), &c),
        Some(Action::Quit)
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('u'), KeyModifiers::NONE), &c),
        Some(Action::Undo)
    );
}

#[test]
fn sequencer_edit_esc_returns_nav() {
    let mode = Mode::Edit;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &c),
        Some(Action::ToggleEdit)
    );
}

#[test]
fn sequencer_edit_note_and_gate() {
    let mode = Mode::Edit;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('z'), KeyModifiers::NONE), &c),
        Some(Action::NoteInput(0))
    );
    assert_eq!(
        handle_key(press(KeyCode::Char('a'), KeyModifiers::NONE), &c),
        Some(Action::GateCycle)
    );
    assert_eq!(
        handle_key(press(KeyCode::Delete, KeyModifiers::NONE), &c),
        Some(Action::DeleteNote)
    );
}

#[test]
fn sequencer_edit_s_is_note_not_save() {
    let mode = Mode::Edit;
    let c = ctx(Editor::Pattern, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('s'), KeyModifiers::NONE), &c),
        Some(Action::NoteInput(1))
    );
}

// --- Graph ---

#[test]
fn graph_normal_enter_nested() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Graph, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Enter, KeyModifiers::NONE), &c),
        Some(Action::EnterGraph)
    );
}

#[test]
fn graph_nested_esc_exits_nest() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Graph, &mode, true, false);
    assert_eq!(
        handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &c),
        Some(Action::ExitGraph)
    );
}

#[test]
fn graph_root_esc_is_noop_in_normal() {
    let mode = Mode::Normal;
    let c = ctx(Editor::Graph, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Esc, KeyModifiers::NONE), &c),
        None
    );
}

#[test]
fn graph_edit_arrows_from_global() {
    let mode = Mode::Edit;
    let c = ctx(Editor::Graph, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Right, KeyModifiers::NONE), &c),
        Some(Action::MoveRight)
    );
    assert_eq!(
        handle_key(press(KeyCode::Left, KeyModifiers::NONE), &c),
        Some(Action::MoveLeft)
    );
}

#[test]
fn graph_edit_mode_has_no_letter_routing() {
    let mode = Mode::Edit;
    let c = ctx(Editor::Graph, &mode, false, false);
    assert_eq!(
        handle_key(press(KeyCode::Char('h'), KeyModifiers::NONE), &c),
        None
    );
}

#[test]
fn editor_tab_order() {
    assert_eq!(Editor::Pattern.next(), Editor::Graph);
    assert_eq!(Editor::Graph.next(), Editor::Pattern);
}

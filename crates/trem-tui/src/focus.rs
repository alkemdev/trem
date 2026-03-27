//! Focus-stack model for the modal TUI shell.
//!
//! The top item owns input. Items below it are parent context. Overlays (help today)
//! sit at the top of the stack and should not replace the underlying surface.

use crate::input::Editor;

/// Kind of focus layer in the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusKind {
    /// Root project/session shell.
    Project,
    /// Top-level editor (`SEQ`, `GRAPH`).
    Editor(Editor),
    /// A deeper focus surface such as nested graph, roll, or edit layer.
    Surface(&'static str),
    /// Temporary UI on top of another surface.
    Overlay(&'static str),
}

/// One item in the focus stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusFrame {
    pub kind: FocusKind,
    pub label: String,
}

impl FocusFrame {
    pub fn project(label: impl Into<String>) -> Self {
        Self {
            kind: FocusKind::Project,
            label: label.into(),
        }
    }

    pub fn editor(editor: Editor) -> Self {
        Self {
            kind: FocusKind::Editor(editor),
            label: editor.title().to_string(),
        }
    }

    pub fn surface(kind: &'static str, label: impl Into<String>) -> Self {
        Self {
            kind: FocusKind::Surface(kind),
            label: label.into(),
        }
    }

    pub fn overlay(kind: &'static str, label: impl Into<String>) -> Self {
        Self {
            kind: FocusKind::Overlay(kind),
            label: label.into(),
        }
    }

    pub fn is_overlay(&self) -> bool {
        matches!(self.kind, FocusKind::Overlay(_))
    }
}

/// What `Esc` does from the current top-of-stack surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscBehavior {
    None,
    Back,
    ApplyAndBack,
    CancelAndBack,
}

/// Visible focus path plus `Esc` behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusStack {
    frames: Vec<FocusFrame>,
    esc_behavior: EscBehavior,
}

impl FocusStack {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            esc_behavior: EscBehavior::None,
        }
    }

    pub fn with_esc_behavior(mut self, esc_behavior: EscBehavior) -> Self {
        self.esc_behavior = esc_behavior;
        self
    }

    pub fn push(&mut self, frame: FocusFrame) {
        self.frames.push(frame);
    }

    pub fn frames(&self) -> &[FocusFrame] {
        &self.frames
    }

    pub fn current(&self) -> Option<&FocusFrame> {
        self.frames.last()
    }

    pub fn parent(&self) -> Option<&FocusFrame> {
        if self.frames.len() < 2 {
            None
        } else {
            self.frames.get(self.frames.len() - 2)
        }
    }

    pub fn breadcrumb(&self) -> String {
        self.frames
            .iter()
            .map(|frame| frame.label.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    }

    pub fn esc_behavior(&self) -> EscBehavior {
        self.esc_behavior
    }

    pub fn esc_hint(&self) -> Option<String> {
        let parent = self.parent()?.label.as_str();
        match self.esc_behavior {
            EscBehavior::None => None,
            EscBehavior::Back => Some(format!("Esc back to {parent}")),
            EscBehavior::ApplyAndBack => Some(format!("Esc apply + back to {parent}")),
            EscBehavior::CancelAndBack => Some(format!("Esc cancel + back to {parent}")),
        }
    }
}

impl Default for FocusStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breadcrumb_joins_stack_labels() {
        let mut stack = FocusStack::new();
        stack.push(FocusFrame::project("Project"));
        stack.push(FocusFrame::editor(Editor::Graph));
        stack.push(FocusFrame::surface("inner-graph", "Lead"));
        assert_eq!(stack.breadcrumb(), "Project > Graph > Lead");
    }

    #[test]
    fn esc_hint_points_at_parent_layer() {
        let mut stack = FocusStack::new();
        stack.push(FocusFrame::project("Project"));
        stack.push(FocusFrame::editor(Editor::Pattern));
        stack.push(FocusFrame::surface("roll", "MIDI Track"));
        let stack = stack.with_esc_behavior(EscBehavior::ApplyAndBack);
        assert_eq!(
            stack.esc_hint().as_deref(),
            Some("Esc apply + back to Sequencer")
        );
    }

    #[test]
    fn overlay_frames_are_marked() {
        assert!(FocusFrame::overlay("help", "Help").is_overlay());
        assert!(!FocusFrame::surface("roll", "MIDI Track").is_overlay());
    }
}

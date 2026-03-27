//! Minimal fullscreen HUD shown when shared shell chrome is collapsed.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

/// One-line overlay for fullscreen mode with active control identity and exit chord.
pub struct FullscreenHud<'a> {
    pub zone: &'a str,
    pub mode: &'a str,
    pub tool: &'a str,
    pub focus_path: &'a str,
    pub esc_hint: Option<&'a str>,
}

impl Widget for FullscreenHud<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let base = theme::shell_base();
        let sep = theme::shell_sep();
        let badge = theme::shell_badge();
        let key = theme::shell_key();
        let dim = theme::shell_dim();
        let val = Style::new().fg(theme::FG).bg(theme::SURFACE);

        let mut spans = vec![
            Span::styled(" FULL ", badge),
            Span::styled("│", sep),
            Span::styled(format!(" {} ", self.zone), badge),
            Span::styled(format!(" {} ", self.mode), val),
            Span::styled(format!(" {} ", self.tool), dim),
        ];

        if !self.focus_path.is_empty() {
            spans.push(Span::styled("│", sep));
            spans.push(Span::styled(format!(" {} ", self.focus_path), val));
        }

        spans.push(Span::styled("│", sep));
        spans.push(Span::styled(" Shift+Enter ", key));
        spans.push(Span::styled("shell", dim));

        if let Some(esc_hint) = self.esc_hint {
            spans.push(Span::styled("│", sep));
            spans.push(Span::styled(format!(" {} ", esc_hint), dim));
        }

        Paragraph::new(Line::from(spans))
            .style(base)
            .render(area, buf);
    }
}

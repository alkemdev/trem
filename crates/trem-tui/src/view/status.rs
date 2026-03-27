//! Single-line status strip: current selection, key family, and explicit `Esc` target.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

/// Bottom shell strip for the modal focus stack.
pub struct StatusBar<'a> {
    pub selection: &'a str,
    pub actions: &'a str,
    pub esc_hint: Option<&'a str>,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let base = theme::shell_base();
        let sep = theme::shell_sep();
        let key = theme::shell_badge();
        let sel = Style::new().fg(theme::FG).bg(theme::SURFACE);
        let dim = theme::shell_dim();
        let esc = theme::warning();

        let mut spans = vec![
            Span::styled(" SEL ", key),
            Span::styled(format!(" {} ", self.selection), sel),
            Span::styled("│", sep),
            Span::styled(" ACT ", key),
            Span::styled(format!(" {} ", self.actions), dim),
        ];
        if let Some(esc_hint) = self.esc_hint {
            spans.push(Span::styled("│", sep));
            spans.push(Span::styled(" ESC ", esc));
            spans.push(Span::styled(format!(" {} ", esc_hint), dim));
        }

        Paragraph::new(Line::from(spans))
            .style(base)
            .render(area, buf);
    }
}

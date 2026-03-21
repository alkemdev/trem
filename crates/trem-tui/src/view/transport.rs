use crate::input::{Mode, View};
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

pub struct TransportView<'a> {
    pub bpm: f64,
    pub beat_position: f64,
    pub playing: bool,
    pub mode: &'a Mode,
    pub view: &'a View,
    pub scale_name: &'a str,
    pub octave: i32,
}

impl<'a> Widget for TransportView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let base = theme::transport();
        let sep = Style::new().fg(theme::MUTED).bg(theme::SURFACE);

        let play_fg = if self.playing {
            theme::GREEN
        } else {
            theme::DIM
        };
        let play_icon = if self.playing {
            " \u{25b6} "
        } else {
            " \u{25a0} "
        };

        let mode_fg = match self.mode {
            Mode::Normal => theme::FG,
            Mode::Edit => theme::ACCENT,
        };

        let mut spans = vec![
            Span::styled(play_icon, base.fg(play_fg)),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" {:.0} BPM ", self.bpm), base),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" beat {:.1} ", self.beat_position), base),
            Span::styled("\u{2502}", sep),
        ];

        // Tab indicators
        spans.push(Span::styled(" ", base));
        for tab in View::ALL {
            let is_active = tab == *self.view;
            let style = if is_active {
                Style::new()
                    .fg(theme::ACCENT)
                    .bg(theme::SURFACE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme::DIM).bg(theme::SURFACE)
            };
            let label = if is_active {
                format!("[{}]", tab.label())
            } else {
                format!(" {} ", tab.label())
            };
            spans.push(Span::styled(label, style));
        }
        spans.push(Span::styled(" ", base));

        spans.extend([
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" {} ", self.mode.label()), base.fg(mode_fg)),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" {} ", self.scale_name), base.fg(theme::NOTE_COLOR)),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" oct {} ", self.octave), base),
        ]);

        Paragraph::new(Line::from(spans))
            .style(base)
            .render(area, buf);
    }
}

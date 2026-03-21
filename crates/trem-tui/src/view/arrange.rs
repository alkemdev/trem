use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Widget};

pub struct ArrangeView<'a> {
    pub steps: u32,
    pub voices: u32,
    pub event_count: usize,
    pub instrument_names: &'a [String],
    pub bpm: f64,
    pub playing: bool,
    pub beat_position: f64,
}

impl<'a> Widget for ArrangeView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Arrangement ", theme::title()))
            .padding(Padding::horizontal(1))
            .style(Style::new().bg(theme::BG));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 4 {
            return;
        }

        let dim = Style::new().fg(theme::DIM).bg(theme::BG);
        let val = theme::value();
        let accent = Style::new()
            .fg(theme::ACCENT)
            .bg(theme::BG)
            .add_modifier(Modifier::BOLD);

        let mut y = inner.y;

        buf.set_line(
            inner.x,
            y,
            &Line::from(vec![
                Span::styled("  Pattern: ", dim),
                Span::styled(
                    format!("{} steps x {} voices", self.steps, self.voices),
                    val,
                ),
            ]),
            inner.width,
        );
        y += 1;

        buf.set_line(
            inner.x,
            y,
            &Line::from(vec![
                Span::styled("  Events:  ", dim),
                Span::styled(format!("{}", self.event_count), val),
            ]),
            inner.width,
        );
        y += 2;

        buf.set_line(
            inner.x,
            y,
            &Line::from(Span::styled("  Timeline", accent)),
            inner.width,
        );
        y += 1;

        let bar_width = inner.width.saturating_sub(12) as usize;
        if bar_width == 0 {
            return;
        }

        let colors = [
            theme::NOTE_COLOR,
            theme::ACCENT,
            theme::GREEN,
            theme::YELLOW,
            theme::FG,
        ];

        for (i, name) in self.instrument_names.iter().enumerate() {
            if y >= inner.y + inner.height {
                break;
            }
            let label = format!("  {:<8}", &name[..name.len().min(8)]);
            let bar = "\u{2588}".repeat(bar_width);
            let color = colors[i % colors.len()];
            buf.set_line(
                inner.x,
                y,
                &Line::from(vec![
                    Span::styled(label, dim),
                    Span::styled(bar, Style::new().fg(color).bg(theme::SURFACE)),
                ]),
                inner.width,
            );
            y += 1;
        }

        if self.playing && y + 1 < inner.y + inner.height {
            y += 1;
            let beat_duration = self.steps as f64;
            let pct = if beat_duration > 0.0 {
                (self.beat_position % beat_duration) / beat_duration
            } else {
                0.0
            };
            let pos = (pct * bar_width as f64) as usize;
            let pad = " ".repeat(pos.min(bar_width.saturating_sub(1)));
            let marker = format!("  {:>8}{}|", "", pad);
            buf.set_line(
                inner.x,
                y,
                &Line::from(Span::styled(marker, accent)),
                inner.width,
            );
        }
    }
}

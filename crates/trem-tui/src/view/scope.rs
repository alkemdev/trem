use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Padding, Widget};

pub struct ScopeView<'a> {
    pub samples: &'a [f32],
}

impl<'a> Widget for ScopeView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::TOP)
            .border_style(theme::border())
            .padding(Padding::ZERO)
            .style(Style::new().bg(theme::BG));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // Center line
        let center_y = inner.y + inner.height / 2;
        let dim = Style::new().fg(theme::MUTED).bg(theme::BG);
        for x in inner.x..inner.x + inner.width {
            buf.set_string(x, center_y, "─", dim);
        }

        if self.samples.is_empty() {
            return;
        }

        let width = inner.width as usize;
        let height = inner.height as f32;
        let accent = Style::new().fg(theme::ACCENT).bg(theme::BG);

        for col in 0..width {
            let idx = col * self.samples.len() / width;
            let sample = self.samples[idx.min(self.samples.len() - 1)].clamp(-1.0, 1.0);

            let y_norm = (1.0 - sample) / 2.0;
            let y = (y_norm * (height - 1.0)).round() as u16;
            let y = y.min(inner.height - 1);

            buf.set_string(inner.x + col as u16, inner.y + y, "█", accent);
        }
    }
}

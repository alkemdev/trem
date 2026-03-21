//! Real-time stereo waveform scope rendered as a braille dot plot.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Padding, Widget};

/// Waveform oscilloscope widget. Plots the most recent audio samples as
/// braille characters scaled to the widget height.
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

        if inner.width == 0 || inner.height < 2 {
            return;
        }

        let half_h = inner.height / 2;
        let top_area = Rect::new(inner.x, inner.y, inner.width, half_h);
        let bot_area = Rect::new(
            inner.x,
            inner.y + half_h,
            inner.width,
            inner.height - half_h,
        );

        let dim = Style::new().fg(theme::MUTED).bg(theme::BG);

        let l_center = top_area.y + top_area.height / 2;
        let r_center = bot_area.y + bot_area.height / 2;
        for x in inner.x..inner.x + inner.width {
            buf.set_string(x, l_center, "\u{2500}", dim);
            buf.set_string(x, r_center, "\u{2500}", dim);
        }

        let l_label = Style::new().fg(theme::DIM).bg(theme::BG);
        buf.set_string(inner.x, top_area.y, "L", l_label);
        buf.set_string(inner.x, bot_area.y, "R", l_label);

        if self.samples.len() < 2 {
            return;
        }

        let width = inner.width as usize;
        let stereo_pairs = self.samples.len() / 2;
        let accent_l = Style::new().fg(theme::ACCENT).bg(theme::BG);
        let accent_r = Style::new().fg(theme::GREEN).bg(theme::BG);

        for col in 0..width {
            let idx = col * stereo_pairs / width;
            let si = (idx * 2).min(self.samples.len().saturating_sub(2));
            let sl = self.samples[si].clamp(-1.0, 1.0);
            let sr = self.samples[si + 1].clamp(-1.0, 1.0);

            if top_area.height > 1 {
                let h = top_area.height as f32;
                let yn = (1.0 - sl) / 2.0;
                let y = (yn * (h - 1.0)).round() as u16;
                let y = y.min(top_area.height - 1);
                buf.set_string(inner.x + col as u16, top_area.y + y, "\u{2588}", accent_l);
            }

            if bot_area.height > 1 {
                let h = bot_area.height as f32;
                let yn = (1.0 - sr) / 2.0;
                let y = (yn * (h - 1.0)).round() as u16;
                let y = y.min(bot_area.height - 1);
                buf.set_string(inner.x + col as u16, bot_area.y + y, "\u{2588}", accent_r);
            }
        }
    }
}

//! Persistent side context panel with focus stack and selection info.

use crate::focus::FocusFrame;
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Widget};

/// Right-side shell panel showing focus stack, transport, and current selection.
pub struct ContextPanel<'a> {
    pub title: &'a str,
    pub zone: &'a str,
    pub mode: &'a str,
    pub tool: &'a str,
    pub frames: &'a [FocusFrame],
    pub details: &'a [String],
    pub selection: &'a str,
    pub actions: &'a str,
    pub esc_hint: Option<&'a str>,
    pub playing: bool,
    pub bpm: f64,
    pub beat_position: f64,
}

impl<'a> Widget for ContextPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::LEFT)
            .border_style(theme::border())
            .title_style(theme::title())
            .title(format!(" {} ", self.title))
            .padding(Padding::new(1, 1, 0, 0))
            .style(theme::panel());
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.width < 12 || inner.height < 8 {
            return;
        }

        let dim = Style::new().fg(theme::DIM).bg(theme::PANEL);
        let head = Style::new()
            .fg(theme::ACCENT)
            .bg(theme::PANEL)
            .add_modifier(Modifier::BOLD);
        let active = Style::new()
            .fg(theme::FG)
            .bg(theme::PANEL)
            .add_modifier(Modifier::BOLD);
        let val = Style::new().fg(theme::FG).bg(theme::PANEL);

        let mut y = inner.y;
        let max_y = inner.y + inner.height;
        let x = inner.x;
        let w = inner.width;

        let section = |label: &str, y: &mut u16, buf: &mut Buffer| {
            if *y >= max_y {
                return;
            }
            let line = Line::from(vec![
                Span::styled(format!(" {} ", label), head),
                Span::styled(
                    "-".repeat(w.saturating_sub(label.len() as u16 + 2) as usize),
                    dim,
                ),
            ]);
            buf.set_line(x, *y, &line, w);
            *y += 1;
        };

        section("STACK", &mut y, buf);
        for (idx, frame) in self.frames.iter().enumerate() {
            if y >= max_y {
                return;
            }
            let style = if idx + 1 == self.frames.len() {
                active
            } else {
                dim
            };
            let indent = "  ".repeat(idx);
            let marker = if idx + 1 == self.frames.len() {
                "▸"
            } else {
                "·"
            };
            buf.set_line(
                x,
                y,
                &Line::from(Span::styled(
                    format!(" {}{} {}", indent, marker, frame.label),
                    style,
                )),
                w,
            );
            y += 1;
        }

        if y + 1 < max_y {
            y += 1;
        }
        section("CONTROL", &mut y, buf);
        for line in [
            format!("zone {}", self.zone),
            format!("mode {}", self.mode),
            format!("tool {}", self.tool),
        ] {
            if y >= max_y {
                return;
            }
            buf.set_line(x, y, &Line::from(Span::styled(line, val)), w);
            y += 1;
        }

        if y + 1 < max_y {
            y += 1;
        }
        section("NOW", &mut y, buf);
        if y < max_y {
            let play = if self.playing { "PLAY" } else { "PAUSE" };
            buf.set_line(
                x,
                y,
                &Line::from(vec![
                    Span::styled(" state ", dim),
                    Span::styled(play, if self.playing { head } else { dim }),
                ]),
                w,
            );
            y += 1;
        }
        if y < max_y {
            buf.set_line(
                x,
                y,
                &Line::from(Span::styled(
                    format!(" bpm {:.0}  beat {:.2}", self.bpm, self.beat_position),
                    val,
                )),
                w,
            );
            y += 1;
        }

        if y + 1 < max_y {
            y += 1;
        }
        section("INFO", &mut y, buf);
        for line in self.details {
            if y >= max_y {
                return;
            }
            buf.set_line(x, y, &Line::from(Span::styled(line, val)), w);
            y += 1;
        }

        if y + 1 < max_y {
            y += 1;
        }
        section("SELECTION", &mut y, buf);
        for line in wrap_text(self.selection, w as usize) {
            if y >= max_y {
                return;
            }
            buf.set_line(x, y, &Line::from(Span::styled(line, val)), w);
            y += 1;
        }

        if y + 1 < max_y {
            y += 1;
        }
        section("ACTIONS", &mut y, buf);
        for line in wrap_text(self.actions, w as usize) {
            if y >= max_y {
                return;
            }
            buf.set_line(x, y, &Line::from(Span::styled(line, dim)), w);
            y += 1;
        }

        if let Some(esc_hint) = self.esc_hint {
            if y + 1 < max_y {
                y += 1;
            }
            section("ESC", &mut y, buf);
            for line in wrap_text(esc_hint, w as usize) {
                if y >= max_y {
                    return;
                }
                buf.set_line(
                    x,
                    y,
                    &Line::from(Span::styled(
                        line,
                        Style::new().fg(theme::YELLOW).bg(theme::PANEL),
                    )),
                    w,
                );
                y += 1;
            }
        }
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let extra = if current.is_empty() { 0 } else { 1 };
        if current.len() + extra + word.len() > width {
            if !current.is_empty() {
                out.push(current);
                current = String::new();
            }
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

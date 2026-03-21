use crate::input::Mode;
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use trem::event::NoteEvent;
use trem::pitch::Scale;

const NOTE_NAMES: [&str; 12] = [
    "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
];

fn gate_suffix(event: &NoteEvent) -> &'static str {
    let g = event.gate.to_f64();
    if g <= 0.26 {
        "\u{00b7}"
    } else if g <= 0.51 {
        ":"
    } else if g <= 0.76 {
        "\u{2502}"
    } else {
        ""
    }
}

fn format_note(event: &NoteEvent, scale: &Scale) -> String {
    let suffix = gate_suffix(event);
    if scale.len() == 12 {
        let idx = event.degree.rem_euclid(12) as usize;
        format!("{}{}{}", NOTE_NAMES[idx], event.octave, suffix)
    } else {
        format!(
            "{}.{}{}",
            event.degree.rem_euclid(scale.len() as i32),
            event.octave,
            suffix
        )
    }
}

pub struct PatternView<'a> {
    pub grid: &'a trem::grid::Grid,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub current_play_row: Option<u32>,
    pub mode: &'a Mode,
    pub scale: &'a Scale,
    pub instrument_names: &'a [String],
}

impl<'a> Widget for PatternView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 8 || area.height < 2 {
            return;
        }

        let voice_label_w: u16 = 7;
        let step_w: u16 = 5;
        let header_h: u16 = 1;

        let avail_w = area.width.saturating_sub(voice_label_w);
        let visible_steps = (avail_w / step_w) as u32;
        if visible_steps == 0 {
            return;
        }

        let half = visible_steps / 2;
        let scroll = if self.cursor_row > half {
            (self.cursor_row - half).min(self.grid.rows.saturating_sub(visible_steps))
        } else {
            0
        };

        let hdr_dim = Style::new().fg(theme::DIM).bg(theme::BG);
        buf.set_string(area.x, area.y, "       ", hdr_dim);
        for si in 0..visible_steps {
            let step = scroll + si;
            if step >= self.grid.rows {
                break;
            }
            let x = area.x + voice_label_w + si as u16 * step_w;
            if x + step_w > area.x + area.width {
                break;
            }

            let is_play = self.current_play_row == Some(step);
            let is_cursor_step = step == self.cursor_row;
            let beat_marker = step % 4 == 0;

            let style = if is_play {
                Style::new()
                    .fg(theme::GREEN)
                    .bg(theme::ACTIVE_ROW)
                    .add_modifier(Modifier::BOLD)
            } else if is_cursor_step {
                Style::new()
                    .fg(theme::ACCENT)
                    .bg(theme::BG)
                    .add_modifier(Modifier::BOLD)
            } else if beat_marker {
                Style::new().fg(theme::FG).bg(theme::BG)
            } else {
                hdr_dim
            };

            let label = if beat_marker {
                format!("{:<4}", step)
            } else {
                format!(" \u{b7}{:<2}", step % 4)
            };
            buf.set_string(x, area.y, &label, style);
        }

        let visible_voices = (area.height.saturating_sub(header_h)) as u32;
        for vi in 0..visible_voices.min(self.grid.columns) {
            let y = area.y + header_h + vi as u16;
            if y >= area.y + area.height {
                break;
            }

            let is_cursor_voice = vi == self.cursor_col;
            let name = self
                .instrument_names
                .get(vi as usize)
                .map(|s| s.as_str())
                .unwrap_or("???");

            let label_style = if is_cursor_voice {
                Style::new()
                    .fg(theme::ACCENT)
                    .bg(theme::BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme::DIM).bg(theme::BG)
            };
            buf.set_string(
                area.x,
                y,
                &format!("{:<6} ", &name[..name.len().min(6)]),
                label_style,
            );

            for si in 0..visible_steps {
                let step = scroll + si;
                if step >= self.grid.rows {
                    break;
                }
                let x = area.x + voice_label_w + si as u16 * step_w;
                if x + step_w > area.x + area.width {
                    break;
                }

                let is_play = self.current_play_row == Some(step);
                let is_cursor = step == self.cursor_row && vi == self.cursor_col;
                let beat_marker = step % 4 == 0;

                let col_bg = if is_play {
                    theme::ACTIVE_ROW
                } else {
                    theme::BG
                };

                let (text, style) = match self.grid.get(step, vi) {
                    Some(event) => {
                        let name = format_note(event, self.scale);
                        let vel = event.velocity.to_f64();
                        let s = if is_cursor {
                            theme::cell_cursor()
                        } else {
                            Style::new().fg(theme::note_velocity_color(vel)).bg(col_bg)
                        };
                        (name, s)
                    }
                    None => {
                        let empty = if beat_marker {
                            "\u{b7}\u{2500}\u{2500}"
                        } else {
                            "\u{2500}\u{2500}\u{2500}"
                        };
                        let s = if is_cursor {
                            theme::cell_cursor()
                        } else if is_play {
                            Style::new().fg(theme::MUTED).bg(col_bg)
                        } else {
                            Style::new().fg(theme::DIM).bg(col_bg)
                        };
                        (empty.to_string(), s)
                    }
                };

                let avail = (area.x + area.width).saturating_sub(x) as usize;
                let display = format!("{:<4}", text);
                let truncated: String = display.chars().take(avail).collect();
                buf.set_string(x, y, &truncated, style);
            }
        }
    }
}

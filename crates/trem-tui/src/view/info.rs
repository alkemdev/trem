use crate::input::{Mode, View};
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Widget};
use trem::event::NoteEvent;
use trem::pitch::Scale;

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn format_gate(gate: &trem::math::Rational) -> String {
    let v = gate.to_f64();
    if v <= 0.26 {
        format!("staccato ({})", gate)
    } else if v <= 0.51 {
        format!("short ({})", gate)
    } else if v <= 0.76 {
        format!("medium ({})", gate)
    } else {
        format!("legato ({})", gate)
    }
}

fn format_note_long(event: &NoteEvent, scale: &Scale) -> String {
    if scale.len() == 12 {
        let idx = event.degree.rem_euclid(12) as usize;
        format!("{}{} (deg {})", NOTE_NAMES[idx], event.octave, event.degree)
    } else {
        format!("deg {} oct {}", event.degree, event.octave)
    }
}

fn meter_bar(peak: f32, width: usize) -> (String, Style) {
    let filled = ((peak.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(width - filled);
    let color = if peak > 0.9 {
        theme::ACCENT
    } else if peak > 0.6 {
        theme::YELLOW
    } else {
        theme::GREEN
    };
    (bar, Style::new().fg(color).bg(theme::BG))
}

fn context_hints(view: &View, mode: &Mode) -> Vec<(&'static str, &'static str)> {
    match (view, mode) {
        (View::Pattern, Mode::Normal) => vec![
            ("TAB", "view"),
            ("e", "edit"),
            ("SPC", "play"),
            ("\u{2190}\u{2192}\u{2191}\u{2193}", "move"),
            ("+/-", "bpm"),
            ("[/]", "oct"),
            ("{/}", "swing"),
            ("u/U", "undo/redo"),
            ("q", "quit"),
        ],
        (View::Pattern, Mode::Edit) => vec![
            ("ESC", "nav"),
            ("z-m", "notes"),
            ("DEL", "clear"),
            ("a", "gate"),
            ("f", "euclidean"),
            ("r", "random"),
            ("t", "reverse"),
            (",/.", "shift"),
            ("w/q", "vel +/-"),
            ("{/}", "swing"),
            ("SPC", "play"),
        ],
        (View::Graph, Mode::Normal) => vec![
            ("TAB", "view"),
            ("e", "edit FX"),
            ("\u{2190}\u{2192}", "follow"),
            ("\u{2191}\u{2193}", "layer"),
            ("SPC", "play"),
            ("q", "quit"),
        ],
        (View::Graph, Mode::Edit) => vec![
            ("ESC", "nav"),
            ("\u{2191}\u{2193}", "param"),
            ("\u{2190}\u{2192}", "adjust"),
            ("+/-", "fine"),
            ("SPC", "play"),
        ],
    }
}

pub struct InfoView<'a> {
    pub mode: &'a Mode,
    pub view: &'a View,
    pub octave: i32,
    pub cursor_step: u32,
    pub cursor_voice: u32,
    pub grid_steps: u32,
    pub grid_voices: u32,
    pub note_at_cursor: Option<&'a NoteEvent>,
    pub scale: &'a Scale,
    pub scale_name: &'a str,
    pub peak_l: f32,
    pub peak_r: f32,
    pub instrument_names: &'a [String],
    pub swing: f64,
    pub euclidean_k: u32,
    pub undo_depth: usize,
}

impl<'a> Widget for InfoView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::RIGHT)
            .border_style(theme::border())
            .padding(Padding::horizontal(1))
            .style(Style::new().bg(theme::BG));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 4 {
            return;
        }

        let x = inner.x;
        let w = inner.width;
        let mut y = inner.y;
        let y_max = inner.y + inner.height;

        let dim = Style::new().fg(theme::DIM).bg(theme::BG);
        let val = theme::value();
        let section = Style::new()
            .fg(theme::ACCENT)
            .bg(theme::BG)
            .add_modifier(Modifier::BOLD);

        // Helper: draw section header
        let draw_section = |buf: &mut Buffer, y: &mut u16, label: &str| -> bool {
            if *y >= y_max {
                return false;
            }
            let pad = w.saturating_sub(label.len() as u16 + 1) as usize;
            let line = Line::from(vec![
                Span::styled(format!(" {} ", label), section),
                Span::styled("\u{2500}".repeat(pad), dim),
            ]);
            buf.set_line(x, *y, &line, w);
            *y += 1;
            true
        };

        let draw_kv = |buf: &mut Buffer, y: &mut u16, key: &str, value_spans: Vec<Span>| -> bool {
            if *y >= y_max {
                return false;
            }
            let mut spans = vec![Span::styled(format!(" {:<7}", key), dim)];
            spans.extend(value_spans);
            buf.set_line(x, *y, &Line::from(spans), w);
            *y += 1;
            true
        };

        // --- CURSOR section ---
        if !draw_section(buf, &mut y, "CURSOR") {
            return;
        }

        let voice_name = self
            .instrument_names
            .get(self.cursor_voice as usize)
            .map(|s| s.as_str())
            .unwrap_or("---");

        let mode_style = match self.mode {
            Mode::Normal => Style::new().fg(theme::FG).bg(theme::BG),
            Mode::Edit => Style::new().fg(theme::ACCENT).bg(theme::BG),
        };

        draw_kv(
            buf,
            &mut y,
            "Mode",
            vec![Span::styled(self.mode.label(), mode_style)],
        );
        draw_kv(
            buf,
            &mut y,
            "Voice",
            vec![Span::styled(
                format!(
                    "{} ({}/{})",
                    voice_name, self.cursor_voice, self.grid_voices
                ),
                val,
            )],
        );
        draw_kv(
            buf,
            &mut y,
            "Step",
            vec![Span::styled(
                format!("{}/{}", self.cursor_step, self.grid_steps),
                val,
            )],
        );

        let note_str = match self.note_at_cursor {
            Some(n) => format_note_long(n, self.scale),
            None => "---".to_string(),
        };
        draw_kv(
            buf,
            &mut y,
            "Note",
            vec![Span::styled(
                note_str,
                Style::new().fg(theme::NOTE_COLOR).bg(theme::BG),
            )],
        );

        if let Some(n) = self.note_at_cursor {
            let gate_label = format_gate(&n.gate);
            draw_kv(buf, &mut y, "Gate", vec![Span::styled(gate_label, val)]);
            draw_kv(
                buf,
                &mut y,
                "Vel",
                vec![Span::styled(format!("{}", n.velocity), val)],
            );
        }

        y += 1;
        if y >= y_max {
            return;
        }

        // --- PROJECT section ---
        if !draw_section(buf, &mut y, "PROJECT") {
            return;
        }
        draw_kv(
            buf,
            &mut y,
            "Scale",
            vec![Span::styled(
                format!("{} ({})", self.scale_name, self.scale.len()),
                val,
            )],
        );
        draw_kv(
            buf,
            &mut y,
            "Oct",
            vec![Span::styled(format!("{}", self.octave), val)],
        );
        if self.swing > 0.001 {
            draw_kv(
                buf,
                &mut y,
                "Swing",
                vec![Span::styled(
                    format!("{:.0}%", self.swing * 100.0),
                    Style::new().fg(theme::YELLOW).bg(theme::BG),
                )],
            );
        }
        if self.euclidean_k > 0 {
            draw_kv(
                buf,
                &mut y,
                "Euclid",
                vec![Span::styled(
                    format!("{}/{}", self.euclidean_k, self.grid_steps),
                    Style::new().fg(theme::ACCENT).bg(theme::BG),
                )],
            );
        }
        if self.undo_depth > 0 {
            draw_kv(
                buf,
                &mut y,
                "Undo",
                vec![Span::styled(format!("{} steps", self.undo_depth), dim)],
            );
        }

        y += 1;
        if y >= y_max {
            return;
        }

        // --- AUDIO section ---
        if !draw_section(buf, &mut y, "AUDIO") {
            return;
        }

        let meter_w = (w as usize).saturating_sub(9).min(14);
        if meter_w > 0 && y + 1 < y_max {
            let (bar_l, style_l) = meter_bar(self.peak_l, meter_w);
            draw_kv(buf, &mut y, "L", vec![Span::styled(bar_l, style_l)]);
            let (bar_r, style_r) = meter_bar(self.peak_r, meter_w);
            draw_kv(buf, &mut y, "R", vec![Span::styled(bar_r, style_r)]);
        }

        y += 1;
        if y >= y_max {
            return;
        }

        // --- KEYS section (contextual) ---
        if !draw_section(buf, &mut y, "KEYS") {
            return;
        }

        let key_style = Style::new().fg(theme::MUTED).bg(theme::BG);
        let hint_style = Style::new().fg(theme::DIM).bg(theme::BG);

        for (k, desc) in context_hints(self.view, self.mode) {
            if y >= y_max {
                break;
            }
            let line = Line::from(vec![
                Span::styled(format!(" {:<6}", k), key_style),
                Span::styled(desc, hint_style),
            ]);
            buf.set_line(x, y, &line, w);
            y += 1;
        }
    }
}

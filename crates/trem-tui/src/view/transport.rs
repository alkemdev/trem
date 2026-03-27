//! Transport bar widget: BPM, playback position, mode indicator, and bottom-pane selector.

use crate::input::{BottomPane, Editor, Mode};
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

/// Single-line transport bar at the top of the TUI.
pub struct TransportView<'a> {
    pub bpm: f64,
    pub beat_position: f64,
    pub playing: bool,
    pub mode: &'a Mode,
    pub editor: &'a Editor,
    pub zone: &'a str,
    pub mode_label: &'a str,
    pub tool_label: &'a str,
    pub focus_path: &'a str,
    pub project_mode: bool,
    pub project_name: Option<&'a str>,
    pub scale_name: &'a str,
    pub octave: i32,
    pub swing: f64,
    pub bottom_pane: BottomPane,
}

impl<'a> Widget for TransportView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let base = theme::shell_base();
        let sep = theme::shell_sep();
        let dim = theme::shell_dim();
        let badge = theme::shell_badge();

        let play_fg = if self.playing {
            theme::GREEN
        } else {
            theme::DIM
        };
        let play_icon = if self.playing {
            " \u{25b6} "
        } else {
            " \u{25a1} "
        };

        let mode_fg = match (self.mode, self.mode_label) {
            (Mode::Edit, _) | (_, "EDIT" | "PARAM" | "ATTR") => theme::ACCENT,
            _ => theme::FG,
        };

        // Beat phase within the unit interval → vertical block height (φ-weighted index feels less grid-locked).
        let phase = self.beat_position.rem_euclid(1.0);
        let phase_blocks = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
        let phase_i = ((phase * phase_blocks.len() as f64 * theme::PHI).floor() as usize)
            % phase_blocks.len();

        let mut spans = vec![
            Span::styled(" trem ", badge),
            Span::styled("\u{2502}", sep),
            Span::styled(play_icon, base.fg(play_fg)),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" {:.0} BPM ", self.bpm), base),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" {:.1} ", self.beat_position), base),
            Span::styled(phase_blocks[phase_i], badge),
            Span::styled(" φ ", dim),
            Span::styled("\u{2502}", sep),
        ];

        // Tab indicators
        spans.push(Span::styled(" ", base));
        for tab in Editor::ALL {
            let is_active = tab == *self.editor;
            let tab_label = if self.project_mode && tab == Editor::Pattern {
                "OVERVIEW"
            } else {
                tab.tab_label()
            };
            let style = if is_active { badge } else { dim };
            let label = if is_active {
                format!("[{}]", tab_label)
            } else {
                format!(" {} ", tab_label)
            };
            spans.push(Span::styled(label, style));
        }
        spans.push(Span::styled(" ", base));

        spans.extend([
            Span::styled("\u{2502}", sep),
            Span::styled(" zone ", dim),
            Span::styled(format!(" {} ", self.zone), badge),
            Span::styled(
                format!(" {} ", self.mode_label),
                Style::new()
                    .fg(mode_fg)
                    .bg(theme::SURFACE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("\u{2502}", sep),
            Span::styled(" tool ", dim),
            Span::styled(format!(" {} ", self.tool_label), dim),
        ]);

        if !self.focus_path.is_empty() {
            spans.push(Span::styled("\u{2502}", sep));
            spans.push(Span::styled(" focus ", dim));
            spans.push(Span::styled(
                format!(" {} ", self.focus_path),
                Style::new().fg(theme::FG).bg(theme::SURFACE),
            ));
        }

        if let Some(project_name) = self.project_name {
            spans.push(Span::styled("\u{2502}", sep));
            spans.push(Span::styled(
                format!(" {} ", project_name),
                base.fg(theme::NOTE_COLOR),
            ));
        } else {
            spans.push(Span::styled("\u{2502}", sep));
            spans.push(Span::styled(
                format!(" {} ", self.scale_name),
                base.fg(theme::NOTE_COLOR),
            ));
            spans.push(Span::styled("\u{2502}", sep));
            spans.push(Span::styled(format!(" oct {} ", self.octave), base));

            if self.swing > 0.001 {
                spans.push(Span::styled("\u{2502}", sep));
                spans.push(Span::styled(
                    format!(" swing {:.0}% ", self.swing * 100.0),
                    base.fg(theme::YELLOW),
                ));
            }
        }

        spans.push(Span::styled("\u{2502}", sep));
        spans.push(Span::styled(format!(" {} ", self.bottom_pane.label()), dim));

        Paragraph::new(Line::from(spans))
            .style(base)
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn render_transport(editor: Editor) -> String {
        let area = Rect::new(0, 0, 160, 1);
        let mut buf = Buffer::empty(area);
        let mode = Mode::Normal;
        TransportView {
            bpm: 120.0,
            beat_position: 0.0,
            playing: false,
            mode: &mode,
            editor: &editor,
            zone: "SEQ",
            mode_label: "NAV",
            tool_label: "step-focus",
            focus_path: "Project > Sequencer",
            project_mode: false,
            project_name: None,
            scale_name: "12-EDO",
            octave: 0,
            swing: 0.0,
            bottom_pane: BottomPane::Spectrum,
        }
        .render(area, &mut buf);
        let mut s = String::new();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    s.push_str(cell.symbol());
                }
            }
        }
        s
    }

    #[test]
    fn transport_shows_seq_and_graph_labels() {
        let row = render_transport(Editor::Pattern);
        assert!(
            row.contains("SEQ"),
            "expected SEQ tab in transport, got: {:?}",
            row.chars().take(120).collect::<String>()
        );
        assert!(row.contains("GRAPH"), "expected GRAPH tab: {:?}", row);
    }

    #[test]
    fn active_editor_shows_bracketed_tab() {
        let seq = render_transport(Editor::Pattern);
        assert!(seq.contains("[SEQ]"), "active seq: {:?}", seq);
        let gr = render_transport(Editor::Graph);
        assert!(gr.contains("[GRAPH]"), "active graph: {:?}", gr);
    }

    #[test]
    fn transport_shows_focus_path() {
        let row = render_transport(Editor::Pattern);
        assert!(row.contains("Project > Sequencer"), "focus path: {:?}", row);
    }
}

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
    pub scale_name: &'a str,
    pub octave: i32,
    pub swing: f64,
    pub bottom_pane: BottomPane,
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

        // Beat phase within the unit interval → vertical block height (φ-weighted index feels less grid-locked).
        let phase = self.beat_position.rem_euclid(1.0);
        let phase_blocks = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
        let phase_i = ((phase * phase_blocks.len() as f64 * theme::PHI).floor() as usize)
            % phase_blocks.len();

        let mut spans = vec![
            Span::styled(play_icon, base.fg(play_fg)),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" {:.0} BPM ", self.bpm), base),
            Span::styled("\u{2502}", sep),
            Span::styled(format!(" {:.1} ", self.beat_position), base),
            Span::styled(
                phase_blocks[phase_i],
                Style::new()
                    .fg(theme::ACCENT)
                    .bg(theme::SURFACE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" φ ", Style::new().fg(theme::DIM).bg(theme::SURFACE)),
            Span::styled("\u{2502}", sep),
        ];

        // Tab indicators
        spans.push(Span::styled(" ", base));
        for tab in Editor::ALL {
            let is_active = tab == *self.editor;
            let style = if is_active {
                Style::new()
                    .fg(theme::ACCENT)
                    .bg(theme::SURFACE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme::DIM).bg(theme::SURFACE)
            };
            let label = if is_active {
                format!("[{}]", tab.tab_label())
            } else {
                format!(" {} ", tab.tab_label())
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

        if self.swing > 0.001 {
            spans.push(Span::styled("\u{2502}", sep));
            spans.push(Span::styled(
                format!(" swing {:.0}% ", self.swing * 100.0),
                base.fg(theme::YELLOW),
            ));
        }

        spans.push(Span::styled("\u{2502}", sep));
        spans.push(Span::styled(
            format!(" {} ", self.bottom_pane.label()),
            base.fg(theme::DIM),
        ));

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
}

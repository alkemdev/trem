//! Full keymap overlay (**`?`**). Sidebar keeps a short “popular” subset only.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget, Wrap};

fn dim(s: &str) -> Span<'_> {
    Span::styled(s, Style::new().fg(theme::DIM).bg(theme::BG))
}

fn key(s: &str) -> Span<'_> {
    Span::styled(
        s,
        Style::new()
            .fg(theme::ACCENT)
            .bg(theme::BG)
            .add_modifier(Modifier::BOLD),
    )
}

fn head(s: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {} ", s),
            Style::new()
                .fg(theme::ACCENT)
                .bg(theme::BG)
                .add_modifier(Modifier::BOLD),
        ),
        dim(" ───────────────────────────────────────"),
    ])
}

/// Modal overlay: every binding, grouped by editor / global.
pub struct HelpOverlay;

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title_style(Style::new().fg(theme::FG).add_modifier(Modifier::BOLD))
            .title(Line::from(vec![
                Span::raw(" trem · keymap "),
                dim("· Esc or ? closes · modal editors: Tab "),
            ]))
            .style(Style::new().bg(theme::BG))
            .padding(Padding::horizontal(1));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 20 || inner.height < 4 {
            return;
        }

        let lines: Vec<Line> = vec![
            head("GLOBAL"),
            Line::from(vec![
                key("Tab"),
                dim(" next editor  "),
                key("Space"),
                dim(" play/stop  "),
                key("?"),
                dim(" this screen  "),
                key("Esc"),
                dim(" exit edit / close help"),
            ]),
            Line::from(vec![
                key("`"),
                dim(" waveform ↔ spectrum  "),
                key("+/-"),
                dim(" BPM  "),
                key("[]"),
                dim(" octave  "),
                key("{}"),
                dim(" swing"),
            ]),
            Line::from(vec![
                dim("Ctrl+"),
                key("S/O"),
                dim(" save/load  "),
                key("Z/Y"),
                dim(" undo/redo  "),
                key("C/Q"),
                dim(" quit"),
            ]),
            Line::from(vec![]),
            head("SEQ · step grid"),
            Line::from(vec![
                key("e"),
                dim(" edit notes  "),
                key("arrows · hjkl"),
                dim(" move  "),
                key("z-m"),
                dim(" degrees  "),
                key("0-9"),
                dim(" degree"),
            ]),
            Line::from(vec![
                key("Del"),
                dim(" clear  "),
                key("a"),
                dim(" gate  "),
                key("f"),
                dim(" euclidean  "),
                key("w/q"),
                dim(" velocity"),
            ]),
            Line::from(vec![
                key("r/t"),
                dim(" rand/rev voice  "),
                key(", ."),
                dim(" shift pattern"),
            ]),
            Line::from(vec![]),
            head("GRAPH · routing"),
            Line::from(vec![
                key("e"),
                dim(" edit params  "),
                key("arrows · hjkl"),
                dim(" node / layer  "),
                key("Enter"),
                dim(" nested graph"),
            ]),
            Line::from(vec![
                key("Esc"),
                dim(" up one nest (when inside)  "),
                dim("edit: "),
                key("←→"),
                dim(" coarse  "),
                key("Shift+←→"),
                dim(" fine  "),
                key("+/-"),
                dim(" fine"),
            ]),
            Line::from(vec![
                dim("bottom: "),
                key("IN|OUT"),
                dim(" scope follows node (graph view)"),
            ]),
            Line::from(vec![]),
            head("ROADMAP"),
            Line::from(vec![
                dim("More modal editors (piano-roll, samples, arrange, …) — see "),
                key("docs/tui-editor-roadmap.md"),
                dim(" in the repo."),
            ]),
        ];

        let p = Paragraph::new(lines)
            .style(Style::new().fg(theme::FG).bg(theme::BG))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);

        p.render(inner, buf);
    }
}

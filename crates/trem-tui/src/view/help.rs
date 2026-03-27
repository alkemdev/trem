//! Sidebar help pane (`?`) with mode-local key groups.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget, Wrap};

fn dim(s: &str) -> Span<'_> {
    Span::styled(s, Style::new().fg(theme::DIM).bg(theme::PANEL))
}

fn dim_owned(s: String) -> Span<'static> {
    Span::styled(s, Style::new().fg(theme::DIM).bg(theme::PANEL))
}

fn key(s: &str) -> Span<'_> {
    Span::styled(s, theme::shell_badge().bg(theme::PANEL))
}

fn head(s: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {} ", s),
            Style::new()
                .fg(theme::ACCENT)
                .bg(theme::PANEL)
                .add_modifier(Modifier::BOLD),
        ),
        dim(" ───────────────────────────────────────"),
    ])
}

/// Sidebar HELP pane: mode-specific shortcuts and global chords.
pub struct HelpOverlay<'a> {
    pub project_mode: bool,
    pub zone: &'a str,
    pub mode: &'a str,
    pub tool: &'a str,
}

impl Widget for HelpOverlay<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::LEFT)
            .border_style(theme::border())
            .title_style(theme::title())
            .title(Line::from(vec![
                Span::raw(" HELP "),
                dim_owned(format!(
                    "· zone {} · mode {} · tool {} · i returns to info ",
                    self.zone, self.mode, self.tool
                )),
            ]))
            .style(theme::panel())
            .padding(Padding::horizontal(1));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 20 || inner.height < 4 {
            return;
        }

        let lines: Vec<Line> = if self.zone == "ROL" {
            vec![
                head("GLOBAL"),
                Line::from(vec![
                    key("Tab"),
                    dim(" cycle rol mode  "),
                    key("Space"),
                    dim(" play/pause  "),
                    key("Shift+Enter"),
                    dim(" fullscreen"),
                ]),
                Line::from(vec![
                    key("s"),
                    dim(" re-sync  "),
                    key("i"),
                    dim(" info pane  "),
                    key("?"),
                    dim(" hide help  "),
                    key("Esc"),
                    dim(" apply + back"),
                ]),
                Line::from(vec![]),
                head("ROL"),
                Line::from(vec![
                    dim("mode: "),
                    key(self.mode),
                    dim("  tool: "),
                    key(self.tool),
                ]),
                Line::from(vec![
                    dim("Pan "),
                    key("hjkl"),
                    dim(" move viewport  "),
                    key("Shift"),
                    dim(" coarse"),
                ]),
                Line::from(vec![
                    dim("Jump "),
                    key("hjkl"),
                    dim(" note-neighbor  "),
                    key("Shift"),
                    dim(" extend selection"),
                ]),
                Line::from(vec![
                    dim("Edit "),
                    key("hjkl"),
                    dim(" move selection  "),
                    key("Ctrl+←/→"),
                    dim(" snap note"),
                ]),
                Line::from(vec![
                    dim("Attr "),
                    key("hl"),
                    dim(" field  "),
                    key("jk"),
                    dim(" adjust field"),
                ]),
                Line::from(vec![]),
                head("SELECT"),
                Line::from(vec![
                    key("f / b"),
                    dim(" next / prev in time  "),
                    key("Ctrl+a"),
                    dim(" select all"),
                ]),
                Line::from(vec![]),
                head("EDIT"),
                Line::from(vec![
                    key("n"),
                    dim(" new  "),
                    key("d"),
                    dim(" duplicate  "),
                    key("Del"),
                    dim(" delete"),
                ]),
                Line::from(vec![
                    key("[ ]"),
                    dim(" duration  "),
                    key("+ / -"),
                    dim(" semitone  "),
                    key("z / x"),
                    dim(" zoom"),
                ]),
                Line::from(vec![
                    key("1 / 2"),
                    dim(" velocity  "),
                    key("e / r"),
                    dim(" voice  "),
                    key("g / a"),
                    dim(" center / fit"),
                ]),
            ]
        } else if self.zone == "PRJ" || self.project_mode {
            vec![
                head("GLOBAL"),
                Line::from(vec![
                    key("Tab"),
                    dim(" overview ↔ graph  "),
                    key("Space"),
                    dim(" play/pause  "),
                    key("Shift+Enter"),
                    dim(" fullscreen"),
                ]),
                Line::from(vec![
                    key("+/-"),
                    dim(" tempo  "),
                    key("?"),
                    dim(" help pane  "),
                    key("i"),
                    dim(" info pane"),
                ]),
                Line::from(vec![
                    dim("Ctrl+"),
                    key("S/O"),
                    dim(" save / reload package  "),
                    key("C/Q"),
                    dim(" quit"),
                ]),
                Line::from(vec![]),
                head("OVERVIEW · scene root"),
                Line::from(vec![
                    key("Enter"),
                    dim(" open selected block  "),
                    key("arrows · hjkl"),
                    dim(" move lanes / blocks"),
                ]),
                Line::from(vec![
                    dim("Clip blocks open "),
                    key("Tab"),
                    dim(" graph view. Enter opens ROL."),
                ]),
                Line::from(vec![]),
                head("GRF · performer / bus"),
                Line::from(vec![
                    key("arrows · hjkl"),
                    dim(" move between nodes  "),
                    key("Tab"),
                    dim(" back to overview"),
                ]),
                Line::from(vec![]),
            ]
        } else {
            vec![
                head("GLOBAL"),
                Line::from(vec![
                    key("Tab"),
                    dim(" next editor  "),
                    key("Space"),
                    dim(" play/pause  "),
                    key("Shift+Enter"),
                    dim(" fullscreen"),
                ]),
                Line::from(vec![
                    key("?"),
                    dim(" help pane  "),
                    key("i"),
                    dim(" info pane"),
                ]),
                Line::from(vec![
                    key("`"),
                    dim(" panel off ↔ scope ↔ spectrum  "),
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
                    key("Enter"),
                    dim(" piano roll · this voice column (Esc apply+back)  "),
                    key("e"),
                    dim(" grid note edit"),
                ]),
                Line::from(vec![
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
                head("GRF · routing"),
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
                    dim("panel: "),
                    key("`"),
                    dim(" cycles off/scope/spectrum · graph panel follows node"),
                ]),
                Line::from(vec![]),
            ]
        };

        let p = Paragraph::new(lines)
            .style(Style::new().fg(theme::FG).bg(theme::PANEL))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);

        p.render(inner, buf);
    }
}

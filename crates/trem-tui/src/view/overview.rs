//! Root scene overview widget.

use crate::project::parse_beat_expr;
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Padding, Widget};
use std::collections::BTreeMap;
use trem_project::{BlockContent, ClipDocument, SceneDocument};

const LABEL_W: u16 = 16;

/// Timeline-style scene overview: lanes on the left, blocks across time.
pub struct OverviewView<'a> {
    pub scene: &'a SceneDocument,
    pub clips: &'a BTreeMap<String, ClipDocument>,
    pub selected_lane: usize,
    pub selected_block: usize,
    pub beat_position: f64,
    pub playing: bool,
}

impl<'a> Widget for OverviewView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = format!(" Overview · {} ", self.scene.scene.name);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(title, theme::title()))
            .padding(Padding::new(1, 1, 0, 0))
            .style(Style::new().bg(theme::BG));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width <= LABEL_W + 6 || inner.height < 4 {
            return;
        }

        let timeline_x = inner.x + LABEL_W + 1;
        let timeline_w = inner.width.saturating_sub(LABEL_W + 1);
        if timeline_w == 0 {
            return;
        }

        let playhead_col = playhead_col(self.scene, self.beat_position, self.playing, timeline_w);
        draw_ruler(
            self.scene,
            timeline_x,
            timeline_w,
            inner.y,
            playhead_col,
            buf,
        );

        for (lane_idx, lane) in self.scene.lanes.iter().enumerate() {
            let y = inner.y + 1 + lane_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let is_lane_selected = lane_idx == self.selected_lane;
            let lane_style = if is_lane_selected {
                Style::new()
                    .fg(theme::ACCENT)
                    .bg(theme::BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme::FG).bg(theme::BG)
            };

            buf.set_stringn(
                inner.x,
                y,
                format!("{:<width$}", lane.label, width = LABEL_W as usize),
                LABEL_W as usize,
                lane_style,
            );
            buf.set_stringn(
                timeline_x - 1,
                y,
                "|",
                1,
                Style::new().fg(theme::DIM).bg(theme::BG),
            );
            for dx in 0..timeline_w {
                let x = timeline_x + dx;
                let mut style = Style::new().fg(theme::DIM).bg(theme::BG);
                if playhead_col == Some(dx) {
                    style = style.fg(theme::ACCENT).add_modifier(Modifier::BOLD);
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(if dx % 4 == 0 { ":" } else { "-" });
                    cell.set_style(style);
                }
            }

            for (block_idx, block) in lane.blocks.iter().enumerate() {
                let start = parse_beat_expr(&block.start).unwrap_or_default();
                let length = parse_beat_expr(&block.length)
                    .unwrap_or_else(|| num_rational::Rational64::from_integer(1));
                let x0 = beat_to_col(self.scene, start, timeline_w);
                let x1 = beat_to_col(self.scene, start + length, timeline_w).max(x0 + 1);
                let width = x1.saturating_sub(x0).min(timeline_w.saturating_sub(x0));
                if width == 0 {
                    continue;
                }

                let selected = lane_idx == self.selected_lane && block_idx == self.selected_block;
                let active = self.playing && block_is_active(self.scene, block, self.beat_position);
                let style = if selected && active {
                    Style::new()
                        .fg(theme::BG)
                        .bg(theme::GREEN)
                        .add_modifier(Modifier::BOLD)
                } else if selected {
                    Style::new()
                        .fg(theme::BG)
                        .bg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else if active {
                    Style::new()
                        .fg(theme::FG)
                        .bg(theme::HIGHLIGHT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(theme::FG).bg(theme::SURFACE)
                };
                let x = timeline_x + x0;
                buf.set_stringn(x, y, " ".repeat(width as usize), width as usize, style);

                match &block.content {
                    BlockContent::Clip { clip } => {
                        if let Some(doc) = self.clips.get(clip) {
                            let preview = clip_preview(doc, width);
                            for (idx, ch) in preview.into_iter().enumerate() {
                                if let Some(cell) = buf.cell_mut((x + idx as u16, y)) {
                                    cell.set_symbol(ch);
                                    cell.set_style(style);
                                }
                            }
                        }
                    }
                    _ => {
                        let fill = if active { "+" } else { "=" };
                        buf.set_stringn(x, y, fill.repeat(width as usize), width as usize, style);
                    }
                }

                let label = block_label(block);
                if width > 6 {
                    buf.set_stringn(x + 1, y, label, width.saturating_sub(2) as usize, style);
                }

                if let Some(col) = playhead_col {
                    if col >= x0 && col < x0 + width {
                        if let Some(cell) = buf.cell_mut((timeline_x + col, y)) {
                            cell.set_symbol("|");
                            cell.set_style(
                                style
                                    .fg(theme::BG)
                                    .bg(theme::YELLOW)
                                    .add_modifier(Modifier::BOLD),
                            );
                        }
                    }
                }
            }
        }
    }
}

fn draw_ruler(
    scene: &SceneDocument,
    x: u16,
    width: u16,
    y: u16,
    playhead_col: Option<u16>,
    buf: &mut Buffer,
) {
    let total = parse_beat_expr(&scene.scene.timeline_beats)
        .filter(|beats| *beats > num_rational::Rational64::from_integer(0))
        .unwrap_or_else(|| num_rational::Rational64::from_integer(16));

    for beat in 0..=*total.numer().max(&0) {
        let col = beat_to_col(scene, num_rational::Rational64::from_integer(beat), width);
        if col >= width {
            continue;
        }
        let tick_x = x + col;
        buf.set_stringn(tick_x, y, "|", 1, Style::new().fg(theme::DIM).bg(theme::BG));
        if tick_x + 1 < x + width {
            buf.set_stringn(
                tick_x + 1,
                y,
                format!("{}", beat + 1),
                width.saturating_sub(col + 1) as usize,
                Style::new().fg(theme::DIM).bg(theme::BG),
            );
        }
    }

    if let Some(col) = playhead_col {
        let tick_x = x + col.min(width.saturating_sub(1));
        buf.set_stringn(
            tick_x,
            y,
            "v",
            1,
            Style::new()
                .fg(theme::YELLOW)
                .bg(theme::BG)
                .add_modifier(Modifier::BOLD),
        );
    }
}

fn beat_to_col(scene: &SceneDocument, beat: num_rational::Rational64, width: u16) -> u16 {
    let total = parse_beat_expr(&scene.scene.timeline_beats)
        .filter(|beats| *beats > num_rational::Rational64::from_integer(0))
        .unwrap_or_else(|| num_rational::Rational64::from_integer(16));
    let rel = (beat * num_rational::Rational64::from_integer(width as i64)) / total;
    (*rel.numer())
        .div_euclid(*rel.denom())
        .clamp(0, i64::from(width)) as u16
}

fn playhead_col(
    scene: &SceneDocument,
    beat_position: f64,
    playing: bool,
    width: u16,
) -> Option<u16> {
    if !playing || width == 0 {
        return None;
    }
    let total = parse_beat_expr(&scene.scene.timeline_beats)
        .filter(|beats| *beats > num_rational::Rational64::from_integer(0))
        .unwrap_or_else(|| num_rational::Rational64::from_integer(16));
    let total_f = *total.numer() as f64 / *total.denom() as f64;
    let beat = beat_position.rem_euclid(total_f.max(1e-9));
    let col = ((beat / total_f.max(1e-9)) * f64::from(width)).floor() as i64;
    Some(col.clamp(0, i64::from(width.saturating_sub(1))) as u16)
}

fn block_is_active(
    scene: &SceneDocument,
    block: &trem_project::BlockSpec,
    beat_position: f64,
) -> bool {
    let total = parse_beat_expr(&scene.scene.timeline_beats)
        .filter(|beats| *beats > num_rational::Rational64::from_integer(0))
        .unwrap_or_else(|| num_rational::Rational64::from_integer(16));
    let total_f = *total.numer() as f64 / *total.denom() as f64;
    let beat = beat_position.rem_euclid(total_f.max(1e-9));
    let start = parse_beat_expr(&block.start)
        .map(|beats| *beats.numer() as f64 / *beats.denom() as f64)
        .unwrap_or(0.0);
    let length = parse_beat_expr(&block.length)
        .map(|beats| *beats.numer() as f64 / *beats.denom() as f64)
        .unwrap_or(1.0);
    beat >= start && beat < start + length
}

fn clip_preview(doc: &ClipDocument, width: u16) -> Vec<&'static str> {
    let mut chars = vec![" "; width as usize];
    let total = parse_beat_expr(&doc.clip.length_beats)
        .filter(|beats| *beats > num_rational::Rational64::from_integer(0))
        .unwrap_or_else(|| num_rational::Rational64::from_integer(1));
    let total_f = *total.numer() as f64 / *total.denom() as f64;
    for note in &doc.notes {
        let start = parse_beat_expr(&note.start)
            .map(|beats| *beats.numer() as f64 / *beats.denom() as f64)
            .unwrap_or(0.0);
        let length = parse_beat_expr(&note.length)
            .map(|beats| *beats.numer() as f64 / *beats.denom() as f64)
            .unwrap_or(0.25);
        let x0 = ((start / total_f.max(1e-9)) * f64::from(width)).floor() as usize;
        let x1 = (((start + length) / total_f.max(1e-9)) * f64::from(width)).ceil() as usize;
        let x0 = x0.min(chars.len().saturating_sub(1));
        let x1 = x1.max(x0 + 1).min(chars.len());
        chars[x0] = "*";
        for ch in chars.iter_mut().take(x1).skip(x0 + 1) {
            if *ch == " " {
                *ch = "-";
            }
        }
    }
    chars
}

fn block_label(block: &trem_project::BlockSpec) -> &str {
    match &block.content {
        BlockContent::Clip { .. } => block.name.as_str(),
        BlockContent::Graph { .. } => block.name.as_str(),
        BlockContent::Sample { .. } => "sample",
        BlockContent::Midi { .. } => "midi",
        BlockContent::Marker { text } => text.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn overview_renders_lane_labels() {
        let area = Rect::new(0, 0, 100, 10);
        let mut buf = Buffer::empty(area);
        OverviewView {
            scene: &trem_project::SceneDocument::easybeat(),
            clips: &std::collections::BTreeMap::new(),
            selected_lane: 0,
            selected_block: 0,
            beat_position: 0.0,
            playing: false,
        }
        .render(area, &mut buf);
        let rendered = buf
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Kick"));
        assert!(rendered.contains("Bass"));
    }
}

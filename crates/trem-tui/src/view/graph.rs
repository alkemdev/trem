use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Widget};
use std::collections::{HashMap, HashSet};
use trem::graph::{ParamDescriptor, ParamUnit};

const NAME_W: u16 = 12;
const GAP_W: u16 = 4;
const LAYER_W: u16 = NAME_W + GAP_W;
const ROW_SPACING: u16 = 2;

const DIR_UP: u8 = 1;
const DIR_DOWN: u8 = 2;
const DIR_LEFT: u8 = 4;
const DIR_RIGHT: u8 = 8;

fn dir_to_char(d: u8) -> char {
    match d {
        d if d == DIR_UP | DIR_DOWN => '\u{2502}',
        d if d == DIR_LEFT | DIR_RIGHT => '\u{2500}',
        d if d == DIR_LEFT | DIR_DOWN => '\u{2510}',
        d if d == DIR_LEFT | DIR_UP => '\u{2518}',
        d if d == DIR_RIGHT | DIR_DOWN => '\u{250c}',
        d if d == DIR_RIGHT | DIR_UP => '\u{2514}',
        d if d == DIR_UP | DIR_DOWN | DIR_RIGHT => '\u{251c}',
        d if d == DIR_UP | DIR_DOWN | DIR_LEFT => '\u{2524}',
        d if d == DIR_LEFT | DIR_RIGHT | DIR_DOWN => '\u{252c}',
        d if d == DIR_LEFT | DIR_RIGHT | DIR_UP => '\u{2534}',
        d if d == DIR_UP | DIR_DOWN | DIR_LEFT | DIR_RIGHT => '\u{253c}',
        d if d == DIR_RIGHT => '\u{2574}',
        _ => ' ',
    }
}

struct NodeLayout {
    x: u16,
    y: u16,
}

struct GraphLayout {
    positions: Vec<NodeLayout>,
    unique_edges: Vec<(usize, usize)>,
}

fn compute_layout(nodes: &[(u32, String)], edges: &[(u32, u16, u32, u16)]) -> GraphLayout {
    let n = nodes.len();
    if n == 0 {
        return GraphLayout {
            positions: vec![],
            unique_edges: vec![],
        };
    }

    let id_to_idx: HashMap<u32, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();

    let mut unique_edges = Vec::new();
    let mut seen = HashSet::new();
    for &(src, _, dst, _) in edges {
        if let (Some(&si), Some(&di)) = (id_to_idx.get(&src), id_to_idx.get(&dst)) {
            if seen.insert((si, di)) {
                unique_edges.push((si, di));
            }
        }
    }

    let mut depths = vec![0usize; n];
    let mut changed = true;
    while changed {
        changed = false;
        for &(si, di) in &unique_edges {
            let new_d = depths[si] + 1;
            if depths[di] < new_d {
                depths[di] = new_d;
                changed = true;
            }
        }
    }

    let max_depth = depths.iter().max().copied().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![vec![]; max_depth + 1];
    for (i, &d) in depths.iter().enumerate() {
        layers[d].push(i);
    }

    let mut positions: Vec<NodeLayout> = (0..n).map(|_| NodeLayout { x: 0, y: 0 }).collect();

    for (pos, &ni) in layers[0].iter().enumerate() {
        positions[ni] = NodeLayout {
            x: 0,
            y: pos as u16 * ROW_SPACING,
        };
    }

    for depth in 1..=max_depth {
        for &ni in &layers[depth] {
            let source_ys: Vec<u16> = unique_edges
                .iter()
                .filter(|&&(_, d)| d == ni)
                .map(|&(s, _)| positions[s].y)
                .collect();

            let y = if source_ys.is_empty() {
                0
            } else {
                let sum: u16 = source_ys.iter().sum();
                sum / source_ys.len() as u16
            };

            positions[ni] = NodeLayout {
                x: depth as u16 * LAYER_W,
                y,
            };
        }

        let mut layer_nodes: Vec<usize> = layers[depth].clone();
        layer_nodes.sort_by_key(|&ni| positions[ni].y);

        let mut last_y: i32 = -(ROW_SPACING as i32);
        for &ni in &layer_nodes {
            if (positions[ni].y as i32) < last_y + ROW_SPACING as i32 {
                positions[ni].y = (last_y + ROW_SPACING as i32) as u16;
            }
            last_y = positions[ni].y as i32;
        }
    }

    GraphLayout {
        positions,
        unique_edges,
    }
}

pub struct GraphViewWidget<'a> {
    pub nodes: &'a [(u32, String)],
    pub edges: &'a [(u32, u16, u32, u16)],
    pub selected: usize,
    pub params: Option<&'a [ParamDescriptor]>,
    pub param_values: Option<&'a [f64]>,
    pub param_cursor: Option<usize>,
}

impl<'a> Widget for GraphViewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Graph ", theme::title()))
            .padding(Padding::new(1, 1, 0, 0))
            .style(Style::new().bg(theme::BG));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 6 {
            return;
        }

        let layout = compute_layout(self.nodes, self.edges);
        if layout.positions.is_empty() {
            return;
        }

        let param_count = self.params.map_or(0, |p| p.len());
        let detail_h: u16 = (3 + param_count as u16).min(inner.height / 2).max(3);
        let graph_h = inner.height.saturating_sub(detail_h + 1);

        let sel_pos = &layout.positions[self.selected];
        let scroll_x = if sel_pos.x + NAME_W > inner.width {
            sel_pos.x.saturating_sub(inner.width / 3)
        } else {
            0
        };
        let scroll_y = if sel_pos.y >= graph_h {
            sel_pos.y.saturating_sub(graph_h / 2)
        } else {
            0
        };

        let connected: HashSet<usize> = layout
            .unique_edges
            .iter()
            .filter(|&&(s, d)| s == self.selected || d == self.selected)
            .flat_map(|&(s, d)| [s, d])
            .collect();

        let outgoing: HashSet<usize> = layout.unique_edges.iter().map(|&(s, _)| s).collect();

        let sel_style = Style::new()
            .fg(theme::ACCENT)
            .bg(theme::HIGHLIGHT)
            .add_modifier(Modifier::BOLD);
        let conn_style = Style::new()
            .fg(theme::GREEN)
            .bg(theme::BG)
            .add_modifier(Modifier::BOLD);
        let node_style = Style::new().fg(theme::NOTE_COLOR).bg(theme::BG);
        let wire_dim = Style::new().fg(theme::MUTED).bg(theme::BG);
        let wire_hi = Style::new().fg(theme::GREEN).bg(theme::BG);

        // --- Build route map ---
        let mut routes: HashMap<(u16, u16), (u8, bool)> = HashMap::new();

        let add_dir =
            |routes: &mut HashMap<(u16, u16), (u8, bool)>, x: u16, y: u16, d: u8, hi: bool| {
                let entry = routes.entry((x, y)).or_insert((0, false));
                entry.0 |= d;
                entry.1 |= hi;
            };

        for &(si, di) in &layout.unique_edges {
            let sy = layout.positions[si].y;
            let dy = layout.positions[di].y;
            let sx = layout.positions[si].x;
            let dx = layout.positions[di].x;
            let hi = si == self.selected || di == self.selected;

            let src_end = sx + NAME_W;

            if sy == dy {
                for x in src_end..dx {
                    add_dir(&mut routes, x, sy, DIR_LEFT | DIR_RIGHT, hi);
                }
            } else {
                let route_x = dx.saturating_sub(2);

                for x in src_end..route_x {
                    add_dir(&mut routes, x, sy, DIR_LEFT | DIR_RIGHT, hi);
                }

                let going_down = sy < dy;
                let enter = if going_down { DIR_DOWN } else { DIR_UP };
                add_dir(&mut routes, route_x, sy, DIR_LEFT | enter, hi);

                let (min_y, max_y) = (sy.min(dy), sy.max(dy));
                for y in (min_y + 1)..max_y {
                    add_dir(&mut routes, route_x, y, DIR_UP | DIR_DOWN, hi);
                }

                let exit = if going_down { DIR_UP } else { DIR_DOWN };
                add_dir(&mut routes, route_x, dy, exit | DIR_RIGHT, hi);

                for x in (route_x + 1)..dx {
                    add_dir(&mut routes, x, dy, DIR_LEFT | DIR_RIGHT, hi);
                }
            }
        }

        // --- Render routes ---
        for (&(gx, gy), &(dirs, hi)) in &routes {
            let rx = match gx.checked_sub(scroll_x) {
                Some(x) => x + inner.x,
                None => continue,
            };
            let ry = match gy.checked_sub(scroll_y) {
                Some(y) => y + inner.y,
                None => continue,
            };
            if rx >= inner.x + inner.width || ry >= inner.y + graph_h {
                continue;
            }
            let ch = dir_to_char(dirs);
            let style = if hi { wire_hi } else { wire_dim };
            buf.set_string(rx, ry, &ch.to_string(), style);
        }

        // --- Render nodes + connecting arms ---
        for (i, (_, name)) in self.nodes.iter().enumerate() {
            let pos = &layout.positions[i];
            let rx = match pos.x.checked_sub(scroll_x) {
                Some(x) => x + inner.x,
                None => continue,
            };
            let ry = match pos.y.checked_sub(scroll_y) {
                Some(y) => y + inner.y,
                None => continue,
            };
            if rx >= inner.x + inner.width || ry >= inner.y + graph_h {
                continue;
            }

            let style = if i == self.selected {
                sel_style
            } else if connected.contains(&i) {
                conn_style
            } else {
                node_style
            };

            let name_chars: usize = name.chars().count().min(NAME_W as usize);
            let truncated: String = name.chars().take(NAME_W as usize).collect();

            let avail = (inner.x + inner.width).saturating_sub(rx) as usize;
            let clipped: String = truncated.chars().take(avail).collect();
            buf.set_string(rx, ry, &clipped, style);

            // Draw connecting arm (─) from end of name to route start
            if outgoing.contains(&i) {
                let arm_style = if i == self.selected {
                    wire_hi
                } else {
                    wire_dim
                };
                let arm_start = rx + name_chars as u16;
                let arm_end = rx + NAME_W;
                for ax in arm_start..arm_end {
                    if ax < inner.x + inner.width {
                        buf.set_string(ax, ry, "\u{2500}", arm_style);
                    }
                }
            }
        }

        // --- Separator ---
        let sep_y = inner.y + graph_h;
        if sep_y < inner.y + inner.height {
            let dim = Style::new().fg(theme::MUTED).bg(theme::BG);
            for x in inner.x..inner.x + inner.width {
                buf.set_string(x, sep_y, "\u{2500}", dim);
            }
        }

        // --- Detail panel ---
        let det_y = sep_y + 1;
        if det_y >= inner.y + inner.height {
            return;
        }
        let det_bottom = inner.y + inner.height;
        let mut py = det_y;

        let sel_name = &self.nodes[self.selected].1;
        let dim = Style::new().fg(theme::DIM).bg(theme::BG);
        let val_s = theme::value();

        // Node name
        let name_line = Line::from(vec![
            Span::styled(" \u{25c6} ", sel_style),
            Span::styled(sel_name.as_str(), sel_style),
        ]);
        buf.set_line(inner.x, py, &name_line, inner.width);
        py += 1;

        // Connections
        if py < det_bottom {
            let inputs: Vec<&str> = layout
                .unique_edges
                .iter()
                .filter(|&&(_, d)| d == self.selected)
                .map(|&(s, _)| self.nodes[s].1.as_str())
                .collect();
            let outputs: Vec<&str> = layout
                .unique_edges
                .iter()
                .filter(|&&(s, _)| s == self.selected)
                .map(|&(_, d)| self.nodes[d].1.as_str())
                .collect();

            let in_str = if inputs.is_empty() {
                "\u{2014}".to_string()
            } else {
                inputs.join(" \u{2192} ")
            };
            let out_str = if outputs.is_empty() {
                "\u{2014}".to_string()
            } else {
                outputs.join(" \u{2192} ")
            };
            let conn_line = Line::from(vec![
                Span::styled("   \u{2190} ", dim),
                Span::styled(in_str, val_s),
                Span::styled("   \u{2192} ", dim),
                Span::styled(out_str, val_s),
            ]);
            buf.set_line(inner.x, py, &conn_line, inner.width);
            py += 1;
        }

        // --- Parameters ---
        if let (Some(params), Some(values)) = (self.params, self.param_values) {
            if params.is_empty() {
                return;
            }

            py += 1;
            if py >= det_bottom {
                return;
            }

            let bar_w = 10usize;

            for (i, (desc, &value)) in params.iter().zip(values.iter()).enumerate() {
                if py >= det_bottom {
                    break;
                }

                let is_sel = self.param_cursor == Some(i);

                let marker = if is_sel { "\u{25b8} " } else { "  " };
                let name_w = 11;
                let name_fmt = format!("{:<w$}", desc.name, w = name_w);

                let range = desc.max - desc.min;
                let norm = if range > 0.0 {
                    ((value - desc.min) / range).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let filled = (norm * bar_w as f64).round() as usize;
                let filled = filled.min(bar_w);

                let bar_on = "\u{2501}".repeat(filled);
                let bar_off = "\u{2500}".repeat(bar_w - filled);

                let knob = if is_sel { "\u{25cf}" } else { "\u{2502}" };

                let display_val = format_param_value(value, desc);

                let (marker_s, name_s, bar_on_s, bar_off_s, knob_s, val_s_row) = if is_sel {
                    (
                        Style::new().fg(theme::ACCENT).bg(theme::BG),
                        Style::new()
                            .fg(theme::FG)
                            .bg(theme::BG)
                            .add_modifier(Modifier::BOLD),
                        Style::new().fg(theme::GREEN).bg(theme::BG),
                        Style::new().fg(theme::MUTED).bg(theme::BG),
                        Style::new()
                            .fg(theme::GREEN)
                            .bg(theme::BG)
                            .add_modifier(Modifier::BOLD),
                        Style::new().fg(theme::NOTE_COLOR).bg(theme::BG),
                    )
                } else {
                    (
                        dim,
                        dim,
                        Style::new().fg(theme::DIM).bg(theme::BG),
                        Style::new().fg(theme::MUTED).bg(theme::BG),
                        Style::new().fg(theme::DIM).bg(theme::BG),
                        Style::new().fg(theme::FG).bg(theme::BG),
                    )
                };

                let line = Line::from(vec![
                    Span::styled(marker, marker_s),
                    Span::styled(name_fmt, name_s),
                    Span::styled(bar_on, bar_on_s),
                    Span::styled(knob, knob_s),
                    Span::styled(bar_off, bar_off_s),
                    Span::styled(format!(" {:<10}", display_val), val_s_row),
                ]);
                buf.set_line(inner.x, py, &line, inner.width);
                py += 1;
            }
        }
    }
}

fn format_param_value(value: f64, desc: &ParamDescriptor) -> String {
    match desc.unit {
        ParamUnit::Hertz => {
            if value >= 10000.0 {
                format!("{:.1}k Hz", value / 1000.0)
            } else if value >= 1000.0 {
                format!("{:.2}k Hz", value / 1000.0)
            } else {
                format!("{:.0} Hz", value)
            }
        }
        ParamUnit::Milliseconds => {
            if value >= 100.0 {
                format!("{:.0} ms", value)
            } else {
                format!("{:.1} ms", value)
            }
        }
        ParamUnit::Seconds => format!("{:.2} s", value),
        ParamUnit::Decibels => {
            if value >= 0.0 {
                format!("+{:.1} dB", value)
            } else {
                format!("{:.1} dB", value)
            }
        }
        ParamUnit::Percent => format!("{:.0}%", value * 100.0),
        ParamUnit::Semitones => format!("{:.1} st", value),
        ParamUnit::Octaves => format!("{:.2} oct", value),
        ParamUnit::Linear => {
            if value.abs() >= 100.0 {
                format!("{:.0}", value)
            } else if value.abs() >= 1.0 {
                format!("{:.2}", value)
            } else {
                format!("{:.3}", value)
            }
        }
    }
}

/// Compute graph layer depths and layer groupings for navigation.
pub fn compute_graph_nav(
    nodes: &[(u32, String)],
    edges: &[(u32, u16, u32, u16)],
) -> (Vec<usize>, Vec<Vec<usize>>) {
    let n = nodes.len();
    if n == 0 {
        return (vec![], vec![]);
    }

    let id_to_idx: HashMap<u32, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();

    let mut seen = HashSet::new();
    let mut unique_edges = Vec::new();
    for &(src, _, dst, _) in edges {
        if let (Some(&si), Some(&di)) = (id_to_idx.get(&src), id_to_idx.get(&dst)) {
            if seen.insert((si, di)) {
                unique_edges.push((si, di));
            }
        }
    }

    let mut depths = vec![0usize; n];
    let mut changed = true;
    while changed {
        changed = false;
        for &(si, di) in &unique_edges {
            let new_d = depths[si] + 1;
            if depths[di] < new_d {
                depths[di] = new_d;
                changed = true;
            }
        }
    }

    let max_depth = depths.iter().max().copied().unwrap_or(0);
    let mut layers = vec![vec![]; max_depth + 1];
    for (i, &d) in depths.iter().enumerate() {
        layers[d].push(i);
    }

    (depths, layers)
}

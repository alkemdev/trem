//! Audio graph visualization widget.
//!
//! Renders the processing graph as a layered left-to-right diagram with
//! connection lines, node highlighting, a breadcrumb trail for nested graphs,
//! and an inline parameter editing panel.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Widget};
use std::collections::{HashMap, HashSet};
use trem::graph::{Edge, GroupHint, ParamDescriptor, ParamGroup, ParamUnit};

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

fn compute_layout(nodes: &[(u32, String)], edges: &[Edge]) -> GraphLayout {
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
    for e in edges {
        if let (Some(&si), Some(&di)) = (id_to_idx.get(&e.src_node), id_to_idx.get(&e.dst_node)) {
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

/// Main graph-view widget. Draws the processing DAG as a layered diagram
/// with connection lines, node labels, and an optional parameter editing pane.
pub struct GraphViewWidget<'a> {
    pub nodes: &'a [(u32, String)],
    pub edges: &'a [Edge],
    pub selected: usize,
    pub params: Option<&'a [ParamDescriptor]>,
    pub param_values: Option<&'a [f64]>,
    pub param_groups: Option<&'a [ParamGroup]>,
    pub param_cursor: Option<usize>,
    pub breadcrumb: &'a [String],
    pub has_children: &'a [bool],
}

impl<'a> Widget for GraphViewWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if let Some(current) = self.breadcrumb.last() {
            format!(" Graph · {} ", current)
        } else {
            " Graph ".to_string()
        };
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(title, theme::title()))
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

        let has_params = self.params.map_or(false, |p| !p.is_empty());
        let detail_h: u16 = if has_params {
            (inner.height / 2).max(6)
        } else {
            3u16.min(inner.height / 3)
        };
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

            let has_child = self.has_children.get(i).copied().unwrap_or(false);
            let suffix = if has_child { " >" } else { "" };
            let display_name = format!(
                "{}{}",
                name.chars()
                    .take(NAME_W as usize - suffix.len())
                    .collect::<String>(),
                suffix
            );
            let name_chars: usize = display_name.chars().count().min(NAME_W as usize);
            let avail = (inner.x + inner.width).saturating_sub(rx) as usize;
            let clipped: String = display_name.chars().take(avail).collect();
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

        // --- Detail panel (scrollable) ---
        let det_y = sep_y + 1;
        if det_y >= inner.y + inner.height {
            return;
        }
        let viewport_h = (inner.y + inner.height).saturating_sub(det_y) as usize;
        if viewport_h == 0 {
            return;
        }

        let sel_name = &self.nodes[self.selected].1;
        let dim = Style::new().fg(theme::DIM).bg(theme::BG);
        let val_s = theme::value();

        // --- Build virtual row list ---
        let mut rows: Vec<DetailRow> = Vec::new();

        // Node name
        rows.push(DetailRow::NodeHeader(Line::from(vec![
            Span::styled(" \u{25c6} ", sel_style),
            Span::styled(sel_name.as_str(), sel_style),
        ])));

        // Connections
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
        rows.push(DetailRow::Static(Line::from(vec![
            Span::styled("   \u{2190} ", dim),
            Span::styled(in_str, val_s),
            Span::styled("   \u{2192} ", dim),
            Span::styled(out_str, val_s),
        ])));

        if let (Some(params), Some(values)) = (self.params, self.param_values) {
            if !params.is_empty() {
                rows.push(DetailRow::Blank);

                let groups = self.param_groups.unwrap_or(&[]);
                let group_name_s = Style::new()
                    .fg(theme::YELLOW)
                    .bg(theme::BG)
                    .add_modifier(Modifier::BOLD);
                let spark_s = Style::new().fg(theme::GREEN).bg(theme::BG);

                let mut sections: Vec<(Option<&ParamGroup>, Vec<(usize, &ParamDescriptor, f64)>)> =
                    Vec::new();

                if groups.is_empty() {
                    let items: Vec<_> = params
                        .iter()
                        .zip(values.iter())
                        .enumerate()
                        .map(|(i, (d, &v))| (i, d, v))
                        .collect();
                    sections.push((None, items));
                } else {
                    for g in groups {
                        let items: Vec<_> = params
                            .iter()
                            .zip(values.iter())
                            .enumerate()
                            .filter(|(_, (d, _))| d.group == Some(g.id))
                            .map(|(i, (d, &v))| (i, d, v))
                            .collect();
                        if !items.is_empty() {
                            sections.push((Some(g), items));
                        }
                    }
                    let ungrouped: Vec<_> = params
                        .iter()
                        .zip(values.iter())
                        .enumerate()
                        .filter(|(_, (d, _))| d.group.is_none())
                        .map(|(i, (d, &v))| (i, d, v))
                        .collect();
                    if !ungrouped.is_empty() {
                        sections.push((None, ungrouped));
                    }
                }

                for (group, items) in &sections {
                    if let Some(g) = group {
                        let preview = match g.hint {
                            GroupHint::Envelope => {
                                let a = find_group_value(items, "Attack").unwrap_or(0.01);
                                let d = find_group_value(items, "Decay").unwrap_or(0.1);
                                let s = find_group_value(items, "Sustain").unwrap_or(0.7);
                                let r = find_group_value(items, "Release").unwrap_or(0.3);
                                Some(envelope_sparkline(a, d, s, r))
                            }
                            GroupHint::Filter => {
                                let freq = find_group_value_unit(items, ParamUnit::Hertz)
                                    .unwrap_or(1000.0);
                                let q = find_group_value_by_suffix(items, "Q")
                                    .or_else(|| find_group_value_by_suffix(items, "Resonance"))
                                    .unwrap_or(0.707);
                                Some(filter_sparkline(freq, q))
                            }
                            _ => None,
                        };

                        let mut spans = vec![
                            Span::styled(" \u{2576} ", dim),
                            Span::styled(g.name, group_name_s),
                            Span::styled(" ", dim),
                        ];
                        if let Some(sparkline) = preview {
                            spans.push(Span::styled(sparkline, spark_s));
                        }
                        rows.push(DetailRow::Static(Line::from(spans)));
                    }

                    for &(flat_i, desc, value) in items {
                        rows.push(DetailRow::Param {
                            flat_index: flat_i,
                            desc,
                            value,
                        });
                    }
                }
            }
        }

        let total_rows = rows.len();

        // --- Compute scroll offset to keep selected param visible ---
        let selected_row = rows.iter().position(|r| match r {
            DetailRow::Param { flat_index, .. } => self.param_cursor == Some(*flat_index),
            _ => false,
        });

        let scroll = detail_panel_scroll(total_rows, viewport_h, selected_row, 2);

        // --- Render visible rows ---
        let scroll_above = scroll > 0;
        let scroll_below = scroll + viewport_h < total_rows;

        let render_start = if scroll_above { 1 } else { 0 };
        let render_end = if scroll_below {
            viewport_h.saturating_sub(1)
        } else {
            viewport_h
        };

        let scroll_ind_s = Style::new().fg(theme::MUTED).bg(theme::BG);

        if scroll_above {
            let above = scroll;
            let line = Line::from(Span::styled(
                format!("  \u{25b2} {} more", above),
                scroll_ind_s,
            ));
            buf.set_line(inner.x, det_y, &line, inner.width);
        }

        for vi in render_start..render_end {
            let ri = scroll + vi;
            if ri >= total_rows {
                break;
            }
            let py = det_y + vi as u16;
            match &rows[ri] {
                DetailRow::NodeHeader(line) | DetailRow::Static(line) => {
                    buf.set_line(inner.x, py, line, inner.width);
                }
                DetailRow::Blank => {}
                DetailRow::Param {
                    flat_index,
                    desc,
                    value,
                } => {
                    render_param_row(
                        buf,
                        inner.x,
                        py,
                        inner.width,
                        desc,
                        *value,
                        self.param_cursor == Some(*flat_index),
                    );
                }
            }
        }

        if scroll_below {
            let below = total_rows - (scroll + viewport_h);
            let line = Line::from(Span::styled(
                format!("  \u{25bc} {} more", below),
                scroll_ind_s,
            ));
            buf.set_line(inner.x, det_y + viewport_h as u16 - 1, &line, inner.width);
        }
    }
}

enum DetailRow<'a> {
    NodeHeader(Line<'a>),
    Static(Line<'a>),
    Blank,
    Param {
        flat_index: usize,
        desc: &'a ParamDescriptor,
        value: f64,
    },
}

fn find_group_value(items: &[(usize, &ParamDescriptor, f64)], name_contains: &str) -> Option<f64> {
    items
        .iter()
        .find(|(_, d, _)| d.name.contains(name_contains))
        .map(|&(_, _, v)| v)
}

fn find_group_value_unit(items: &[(usize, &ParamDescriptor, f64)], unit: ParamUnit) -> Option<f64> {
    items
        .iter()
        .find(|(_, d, _)| d.unit == unit)
        .map(|&(_, _, v)| v)
}

fn find_group_value_by_suffix(
    items: &[(usize, &ParamDescriptor, f64)],
    suffix: &str,
) -> Option<f64> {
    items
        .iter()
        .find(|(_, d, _)| d.name.ends_with(suffix))
        .map(|&(_, _, v)| v)
}

const BARS: [char; 8] = [
    '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}',
];

fn sparkline_from_values(vals: &[f64]) -> String {
    let max = vals.iter().cloned().fold(0.0f64, f64::max).max(1e-9);
    vals.iter()
        .map(|&v| {
            let norm = (v / max).clamp(0.0, 1.0);
            let idx = ((norm * 7.0).round() as usize).min(7);
            BARS[idx]
        })
        .collect()
}

/// Compute a 16-sample ADSR envelope shape and render as a sparkline.
fn envelope_sparkline(attack: f64, decay: f64, sustain: f64, release: f64) -> String {
    let n = 16usize;
    let total = attack + decay + 0.15 + release;
    let a_cols = ((attack / total) * n as f64).round().clamp(1.0, 5.0) as usize;
    let d_cols = ((decay / total) * n as f64).round().clamp(1.0, 5.0) as usize;
    let r_cols = ((release / total) * n as f64).round().clamp(1.0, 5.0) as usize;
    let s_cols = n.saturating_sub(a_cols + d_cols + r_cols).max(1);

    let mut vals = Vec::with_capacity(n);
    for i in 0..a_cols {
        vals.push((i + 1) as f64 / a_cols as f64);
    }
    for i in 0..d_cols {
        let t = (i + 1) as f64 / d_cols as f64;
        vals.push(1.0 - t * (1.0 - sustain));
    }
    for _ in 0..s_cols {
        vals.push(sustain);
    }
    for i in 0..r_cols {
        let t = (i + 1) as f64 / r_cols as f64;
        vals.push(sustain * (1.0 - t));
    }
    vals.truncate(n);
    sparkline_from_values(&vals)
}

/// Compute a simple lowpass frequency response and render as a sparkline.
fn filter_sparkline(cutoff_hz: f64, q: f64) -> String {
    let n = 16;
    let mut vals = vec![0.0; n];
    for i in 0..n {
        let freq = 20.0 * (20000.0 / 20.0f64).powf(i as f64 / (n - 1) as f64);
        let ratio = freq / cutoff_hz;
        let mag = 1.0
            / (1.0 + ratio.powi(4) - 2.0 * ratio.powi(2) + ratio.powi(2) / (q * q))
                .sqrt()
                .max(0.01);
        vals[i] = mag.min(2.0);
    }
    sparkline_from_values(&vals)
}

fn render_param_row(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    desc: &ParamDescriptor,
    value: f64,
    is_sel: bool,
) {
    let bar_w = 10usize;
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

    let dim = Style::new().fg(theme::DIM).bg(theme::BG);
    let (marker_s, name_s, bar_on_s, bar_off_s, knob_s, val_s) = if is_sel {
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
        Span::styled(format!(" {:<10}", display_val), val_s),
    ]);
    buf.set_line(x, y, &line, width);
}

pub(crate) fn format_param_value(value: f64, desc: &ParamDescriptor) -> String {
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

/// Scroll offset for a detail list so `selected_row` stays visible when possible,
/// biased ~`margin` rows from the top. Pure and safe when `min > max` for `clamp`.
pub fn detail_panel_scroll(
    total_rows: usize,
    viewport_h: usize,
    selected_row: Option<usize>,
    margin: usize,
) -> usize {
    let Some(sel_row) = selected_row else {
        return 0;
    };
    if total_rows <= viewport_h {
        return 0;
    }
    let max_scroll = total_rows.saturating_sub(viewport_h);
    let min_scroll = sel_row.saturating_sub(viewport_h.saturating_sub(1));
    let mut s = sel_row.saturating_sub(margin);
    if s < min_scroll {
        s = min_scroll;
    }
    if s > max_scroll {
        s = max_scroll;
    }
    s.min(max_scroll).max(min_scroll.min(max_scroll))
}

/// Compute graph layer depths and layer groupings for navigation.
pub fn compute_graph_nav(nodes: &[(u32, String)], edges: &[Edge]) -> (Vec<usize>, Vec<Vec<usize>>) {
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
    for e in edges {
        if let (Some(&si), Some(&di)) = (id_to_idx.get(&e.src_node), id_to_idx.get(&e.dst_node)) {
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

#[cfg(test)]
mod detail_scroll_tests {
    use super::detail_panel_scroll;

    #[test]
    fn no_selection_no_scroll() {
        assert_eq!(detail_panel_scroll(10, 3, None, 2), 0);
    }

    #[test]
    fn fits_viewport_scroll_zero() {
        assert_eq!(detail_panel_scroll(3, 10, Some(2), 2), 0);
    }

    #[test]
    fn never_panics_when_viewport_smaller_than_content() {
        // Regression: bad clamp(min,max) when min > max
        for total in 4..20 {
            for vp in 2..total {
                for sel in 0..total {
                    let _ = detail_panel_scroll(total, vp, Some(sel), 2);
                }
            }
        }
    }
}

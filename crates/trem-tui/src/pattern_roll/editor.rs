//! Fullscreen piano roll while editing a [`Clip`] from the step grid.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use num_rational::Rational64;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use trem::grid::Grid;
use trem::math::Rational;
use trem::pitch::Scale;
use trem::rung::{BeatTime, Clip, ClipNote, RungFile};
use trem_rta::{Bridge, Command};

use super::convert::apply_clip_to_grid_column;

#[inline]
fn beat_step() -> Rational64 {
    Rational64::new(1, 16)
}

#[inline]
fn min_dur() -> Rational64 {
    Rational64::new(1, 64)
}

#[derive(Clone, Copy, Debug)]
struct Viewport {
    origin_beat: Rational64,
    width_beats: Rational64,
    top_class: i32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            origin_beat: Rational64::from_integer(0),
            width_beats: Rational64::from_integer(4),
            top_class: 72,
        }
    }
}

impl Viewport {
    fn clamp_width(&mut self) {
        let min_w = Rational64::new(1, 16);
        let max_w = Rational64::from_integer(10_000);
        if self.width_beats < min_w {
            self.width_beats = min_w;
        }
        if self.width_beats > max_w {
            self.width_beats = max_w;
        }
    }

    fn zoom_in(&mut self) {
        self.width_beats = self.width_beats * Rational64::new(2, 3);
        self.clamp_width();
    }

    fn zoom_out(&mut self) {
        self.width_beats = self.width_beats * Rational64::new(3, 2);
        self.clamp_width();
    }

    fn pan_time(&mut self, delta: Rational64) {
        self.origin_beat = self.origin_beat + delta;
    }

    fn col_range(self, col: u16, cols: u16) -> (Rational64, Rational64) {
        if cols == 0 {
            return (self.origin_beat, self.origin_beat);
        }
        let c = Rational64::from_integer(cols as i64);
        let t0 = self.origin_beat + (self.width_beats * Rational64::from_integer(col as i64)) / c;
        let t1 =
            self.origin_beat + (self.width_beats * Rational64::from_integer((col + 1) as i64)) / c;
        (t0, t1)
    }
}

fn clip_bounds(clip: &Clip) -> Option<(Rational64, Rational64, i32, i32)> {
    if clip.notes.is_empty() {
        return None;
    }
    let mut t_min = None::<Rational64>;
    let mut t_max = None::<Rational64>;
    let mut c_min = i32::MAX;
    let mut c_max = i32::MIN;
    for n in &clip.notes {
        let a = n.t_on.rational();
        let b = n.t_off.rational();
        t_min = Some(t_min.map_or(a, |m| m.min(a)));
        t_max = Some(t_max.map_or(b, |m| m.max(b)));
        c_min = c_min.min(n.class);
        c_max = c_max.max(n.class);
    }
    Some((t_min.unwrap(), t_max.unwrap(), c_min, c_max))
}

fn fit_all(clip: &Clip, vp: &mut Viewport) {
    if let Some((t_min, t_max, _c_min, c_max)) = clip_bounds(clip) {
        let span = (t_max - t_min).max(Rational64::new(1, 4));
        let pad = span / Rational64::from_integer(10);
        vp.origin_beat = t_min - pad;
        vp.width_beats = (t_max - t_min) + pad * Rational64::from_integer(2);
        vp.width_beats = vp.width_beats.max(Rational64::new(1, 4));
        vp.clamp_width();
        vp.top_class = c_max + 2;
    } else {
        *vp = Viewport::default();
    }
}

fn center_on_selected(clip: &Clip, selected: usize, grid_rows: u16, vp: &mut Viewport) {
    let Some(n) = clip.notes.get(selected) else {
        return;
    };
    let mid = (n.t_on.rational() + n.t_off.rational()) / Rational64::from_integer(2);
    vp.origin_beat = mid - vp.width_beats / Rational64::from_integer(2);
    let half = (grid_rows / 2) as i32;
    vp.top_class = n.class + half.max(1);
}

fn cell_state(
    class: i32,
    t0: Rational64,
    t1: Rational64,
    clip: &Clip,
    selected: usize,
) -> (char, bool) {
    let mut best: Option<(usize, bool)> = None;
    for (i, n) in clip.notes.iter().enumerate() {
        if n.class != class {
            continue;
        }
        if n.t_off.rational() <= t0 || n.t_on.rational() >= t1 {
            continue;
        }
        let sel = i == selected;
        match best {
            None => best = Some((i, sel)),
            Some((_, was_sel)) => {
                if sel && !was_sel {
                    best = Some((i, sel));
                }
            }
        }
    }
    match best {
        Some((_, true)) => ('█', true),
        Some(_) => ('▓', false),
        None => ('·', false),
    }
}

/// Map a **floating** beat (e.g. engine playhead) to a column; must stay in sync with `col_range`.
fn playhead_col_f64(vp: Viewport, beat: f64, cols: u16) -> Option<u16> {
    if cols == 0 {
        return None;
    }
    let origin = rational_to_f64(vp.origin_beat);
    let width = rational_to_f64(vp.width_beats);
    if width <= 0.0 {
        return None;
    }
    if beat < origin || beat >= origin + width {
        return None;
    }
    let rel = beat - origin;
    let col = ((rel / width) * f64::from(cols)).floor() as i64;
    let col = col.clamp(0, i64::from(cols.saturating_sub(1))) as u16;
    Some(col)
}

fn beat_tick_col(vp: Viewport, beat: Rational64, cols: u16) -> Option<u16> {
    if cols == 0 || vp.width_beats <= Rational64::from_integer(0) {
        return None;
    }
    if beat < vp.origin_beat || beat >= vp.origin_beat + vp.width_beats {
        return None;
    }
    let rel = beat - vp.origin_beat;
    let x = (rel * Rational64::from_integer(cols as i64)) / vp.width_beats;
    let num = *x.numer();
    let den = *x.denom();
    if den == 0 {
        return None;
    }
    let col = num
        .div_euclid(den)
        .clamp(0, i64::from(cols.saturating_sub(1)));
    Some(col as u16)
}

fn ruler_line(vp: Viewport, cols: u16, playhead_col: Option<u16>) -> Line<'static> {
    if cols == 0 {
        return Line::from("");
    }
    let mut chars = vec![' '; cols as usize];
    let start = floor_beat(vp.origin_beat);
    let end = ceil_beat(vp.origin_beat + vp.width_beats) + 1;
    for b in start..=end {
        let bt = Rational64::from_integer(b);
        if let Some(col) = beat_tick_col(vp, bt, cols) {
            let i = col as usize;
            if i < chars.len() {
                chars[i] = if b.rem_euclid(4) == 0 { ':' } else { '|' };
            }
        }
    }
    let tick_st = Style::default().fg(Color::DarkGray);
    let ph_st = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let spans: Vec<Span<'static>> = (0..cols as usize)
        .map(|i| {
            let col = i as u16;
            let (c, st) = if playhead_col == Some(col) {
                ('▼', ph_st)
            } else {
                (chars[i], tick_st)
            };
            Span::styled(c.to_string(), st)
        })
        .collect();
    Line::from(spans)
}

fn floor_beat(r: Rational64) -> i64 {
    let n = *r.numer();
    let d = *r.denom();
    if d == 0 {
        return 0;
    }
    n.div_euclid(d)
}

fn ceil_beat(r: Rational64) -> i64 {
    let n = *r.numer();
    let d = *r.denom();
    if d == 0 {
        return 0;
    }
    (n + d - 1).div_euclid(d)
}

fn render_roll_row(
    class: i32,
    vp: Viewport,
    cols: u16,
    clip: &Clip,
    selected: usize,
    playhead_col: Option<u16>,
) -> Line<'static> {
    let mut spans = Vec::new();
    for col in 0..cols {
        let (t0, t1) = vp.col_range(col, cols);
        let (ch, sel) = cell_state(class, t0, t1, clip, selected);
        let mut st = if sel {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else if ch == '▓' {
            Style::default().fg(Color::LightBlue)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        if Some(col) == playhead_col {
            st = st.bg(Color::Rgb(45, 42, 68));
        }
        spans.push(Span::styled(ch.to_string(), st));
    }
    Line::from(spans)
}

fn rational_to_f64(r: Rational64) -> f64 {
    let n = *r.numer() as f64;
    let d = *r.denom() as f64;
    if d == 0.0 {
        0.0
    } else {
        n / d
    }
}

fn sync_clip_length(clip: &mut Clip, pattern_end_floor: BeatTime) {
    let max_off = clip.notes.iter().map(|n| n.t_off).max();
    clip.length_beats = Some(match max_off {
        Some(m) => m.max(pattern_end_floor),
        None => pattern_end_floor,
    });
}

fn clamp_duration(n: &mut ClipNote) {
    if n.t_off.rational() <= n.t_on.rational() {
        n.t_off = BeatTime(n.t_on.rational() + min_dur());
    }
}

/// Outcome of a key press in the pattern roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternRollOutcome {
    Stay,
    /// Merge clip into grid and exit fullscreen roll.
    CloseApply,
}

pub struct PatternRoll {
    pub clip: Clip,
    pub selected: usize,
    pub dirty: bool,
    /// Step grid column (voice lane) being edited.
    pub grid_column: u32,
    /// Bridge voice id for that column (for status line).
    pub lane_voice: u32,
    vp: Viewport,
    did_autofit: bool,
    loop_beats_floor: BeatTime,
    /// Grid snapshot at open; other lanes stay as-is for preview merge.
    preview_base: Grid,
    scale: Scale,
    voice_ids: Vec<u32>,
    reference_hz: f64,
    swing: f64,
}

impl PatternRoll {
    pub fn new(
        clip: Clip,
        grid_column: u32,
        pattern_rows: u32,
        lane_voice: u32,
        preview_base: Grid,
        scale: Scale,
        voice_ids: Vec<u32>,
        reference_hz: f64,
        swing: f64,
    ) -> Self {
        let loop_beats_floor = BeatTime(Rational64::from_integer(pattern_rows as i64));
        Self {
            clip,
            selected: 0,
            dirty: false,
            grid_column,
            lane_voice,
            vp: Viewport::default(),
            did_autofit: false,
            loop_beats_floor,
            preview_base,
            scale,
            voice_ids,
            reference_hz,
            swing,
        }
    }

    pub fn validate_for_apply(&self) -> Result<(), String> {
        let mut c = self.clip.clone();
        sync_clip_length(&mut c, self.loop_beats_floor);
        RungFile::new(c).validate().map_err(|e| e.to_string())
    }

    pub fn push_preview(&mut self, bridge: &mut Bridge, bpm: f64, sample_rate: f64) {
        sync_clip_length(&mut self.clip, self.loop_beats_floor);
        let mut g = self.preview_base.clone();
        apply_clip_to_grid_column(
            &self.clip,
            &mut g,
            &self.scale,
            self.reference_hz,
            &self.voice_ids,
            self.grid_column,
        );
        let beats = Rational::integer(g.rows as i64);
        let events = trem::render::grid_to_timed_events(
            &g,
            beats,
            bpm,
            sample_rate,
            &self.scale,
            self.reference_hz,
            &self.voice_ids,
            self.swing,
        );
        bridge.send(Command::LoadEvents(events));
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        bridge: &mut Bridge,
        bpm: f64,
        sample_rate: f64,
        playing: &mut bool,
        engine_pattern_active: &mut bool,
    ) -> PatternRollOutcome {
        if key.kind != KeyEventKind::Press {
            return PatternRollOutcome::Stay;
        }

        if !self.clip.notes.is_empty() {
            self.selected = self.selected.min(self.clip.notes.len() - 1);
        } else {
            self.selected = 0;
        }

        match key.code {
            KeyCode::Esc => PatternRollOutcome::CloseApply,
            KeyCode::Char(' ') => {
                *playing = !*playing;
                if *playing {
                    *engine_pattern_active = true;
                    self.push_preview(bridge, bpm, sample_rate);
                    bridge.send(Command::Play);
                } else {
                    bridge.send(Command::Pause);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                // Re-sync preview only (grid apply is Esc).
                self.push_preview(bridge, bpm, sample_rate);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('f') => {
                if !self.clip.notes.is_empty() {
                    self.selected = (self.selected + 1).min(self.clip.notes.len() - 1);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('b') => {
                self.selected = self.selected.saturating_sub(1);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.vp
                    .pan_time(-self.vp.width_beats / Rational64::from_integer(8));
                PatternRollOutcome::Stay
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.vp
                    .pan_time(self.vp.width_beats / Rational64::from_integer(8));
                PatternRollOutcome::Stay
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.vp.top_class += 1;
                PatternRollOutcome::Stay
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.vp.top_class -= 1;
                PatternRollOutcome::Stay
            }
            KeyCode::Char('z') => {
                self.vp.zoom_in();
                PatternRollOutcome::Stay
            }
            KeyCode::Char('x') => {
                self.vp.zoom_out();
                PatternRollOutcome::Stay
            }
            KeyCode::Char('g') => {
                // grid_rows approximated; center still works
                center_on_selected(&self.clip, self.selected, 20, &mut self.vp);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('a') => {
                fit_all(&self.clip, &mut self.vp);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.class = n.class.wrapping_add(1);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.class = n.class.wrapping_sub(1);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char(']') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.t_off = BeatTime(n.t_off.rational() + beat_step());
                    clamp_duration(n);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('[') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.t_off = BeatTime(n.t_off.rational() - beat_step());
                    clamp_duration(n);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('.') | KeyCode::Char('>') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.t_on = BeatTime(n.t_on.rational() + beat_step());
                    clamp_duration(n);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char(',') | KeyCode::Char('<') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.t_on = BeatTime(n.t_on.rational() - beat_step());
                    clamp_duration(n);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('1') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.velocity = (n.velocity - 0.05).clamp(0.0, 1.0);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('2') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.velocity = (n.velocity + 0.05).clamp(0.0, 1.0);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('e') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.voice = n.voice.saturating_sub(1);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('r') => {
                if let Some(n) = self.clip.notes.get_mut(self.selected) {
                    n.voice = (n.voice + 1).min(127);
                    self.dirty = true;
                    self.push_preview(bridge, bpm, sample_rate);
                }
                PatternRollOutcome::Stay
            }
            _ => PatternRollOutcome::Stay,
        }
    }

    /// First-frame autofit (call once after construction with terminal height).
    pub fn autofit_if_needed(&mut self, _term_h: u16) {
        if !self.did_autofit && !self.clip.notes.is_empty() {
            fit_all(&self.clip, &mut self.vp);
            self.did_autofit = true;
        }
    }

    /// `beat_position` / `loop_beats` come from global transport; playhead wraps to pattern length.
    pub fn draw(
        &mut self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        transport: Line<'_>,
        beat_position: f64,
        loop_beats: f64,
    ) {
        self.autofit_if_needed(area.height);

        if !self.clip.notes.is_empty() {
            self.selected = self.selected.min(self.clip.notes.len() - 1);
        } else {
            self.selected = 0;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(8),
                Constraint::Length(2),
                Constraint::Length(2),
            ])
            .split(area);

        let title = if self.dirty { "* " } else { "" };
        let header = Paragraph::new(Line::from(vec![
            title.into(),
            "pattern roll (fullscreen) ".into(),
            format!("lane {} · voice {}  ", self.grid_column, self.lane_voice).cyan(),
            "Esc apply & close ".cyan(),
            "  ".into(),
            format!("{} notes", self.clip.notes.len()).dim(),
            "  ".into(),
            format!(
                "span {:.2} beats  [{:.3} … {:.3})",
                rational_to_f64(self.vp.width_beats),
                rational_to_f64(self.vp.origin_beat),
                rational_to_f64(self.vp.origin_beat + self.vp.width_beats)
            )
            .dim(),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(header, chunks[0]);

        frame.render_widget(
            Paragraph::new(transport).block(Block::default().borders(Borders::BOTTOM)),
            chunks[1],
        );

        let body = chunks[2];
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" MIDI piano roll — class ↑  time → ");
        let inner = block.inner(body);
        frame.render_widget(block, body);

        let cols_lr = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(6), Constraint::Min(8)])
            .split(inner);

        let left_col = cols_lr[0];
        let right_col = cols_lr[1];

        let rows_rl = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(4)])
            .split(right_col);

        let ruler_area = rows_rl[0];
        let grid_area = rows_rl[1];
        let roll_cols = grid_area.width.max(1);
        let grid_h = grid_area.height.max(1);
        let vp = self.vp;

        let loop_len = loop_beats.max(1e-9);
        let playhead_beat = beat_position.rem_euclid(loop_len);
        let playhead_col = playhead_col_f64(vp, playhead_beat, roll_cols);

        let ruler = Paragraph::new(ruler_line(vp, ruler_area.width.max(1), playhead_col));
        frame.render_widget(ruler, ruler_area);

        let clip_ref = &self.clip;
        let sel = self.selected;
        let lines: Vec<Line> = (0..grid_h)
            .map(|row| {
                let class = vp.top_class - row as i32;
                render_roll_row(class, vp, roll_cols, clip_ref, sel, playhead_col)
            })
            .collect();
        frame.render_widget(Paragraph::new(Text::from(lines)), grid_area);

        let rows_left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(4)])
            .split(left_col);

        let label_head =
            Paragraph::new(Span::styled("class", Style::default().fg(Color::DarkGray)))
                .alignment(Alignment::Right);
        frame.render_widget(label_head, rows_left[0]);

        let label_lines: Vec<Line> = (0..grid_h)
            .map(|row| {
                let class = vp.top_class - row as i32;
                Line::from(Span::styled(
                    format!("{:>5}", class),
                    Style::default().fg(Color::Green),
                ))
            })
            .collect();
        let labels = Paragraph::new(Text::from(label_lines))
            .alignment(Alignment::Right)
            .block(Block::default().borders(Borders::RIGHT));
        frame.render_widget(labels, rows_left[1]);

        let detail = if let Some(n) = self.clip.notes.get(self.selected) {
            let dur = n.t_off.rational() - n.t_on.rational();
            format!(
                "sel #{}  class {:>4}  t_on {}  t_off {}  Δ {}/{}  voice {}  vel {:.2}",
                self.selected,
                n.class,
                n.t_on,
                n.t_off,
                dur.numer(),
                dur.denom(),
                n.voice,
                n.velocity
            )
        } else {
            "(no notes)".to_string()
        };
        let sel_line = Paragraph::new(Line::from(vec![detail.white()]))
            .block(Block::default().borders(Borders::ALL).title(" selection "));
        frame.render_widget(sel_line, chunks[3]);

        let help = Paragraph::new(Line::from(vec![
            "hl time  kj rows  zx zoom  bf note  g center  a fit  +/- class  [] t_off  ,. t_on  12 vel  er voice  Space play  s re-sync audio  Esc apply+close"
                .yellow(),
        ]));
        frame.render_widget(help, chunks[4]);
    }
}

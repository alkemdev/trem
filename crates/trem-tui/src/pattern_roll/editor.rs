//! Piano roll while editing a [`Clip`] from the step grid.

use crate::theme;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use num_rational::Rational64;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::collections::BTreeSet;
use trem::event::{GraphEvent, TimedEvent};
use trem::grid::Grid;
use trem::math::Rational;
use trem::pitch::{Pitch, Scale};
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

#[inline]
fn snap_step() -> Rational64 {
    Rational64::new(1, 16)
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
        self.width_beats = snap_width(self.width_beats);
        self.origin_beat = snap_origin(self.origin_beat);
    }

    fn zoom_in(&mut self) {
        self.width_beats = self.width_beats / Rational64::from_integer(2);
        self.clamp_width();
    }

    fn zoom_out(&mut self) {
        self.width_beats = self.width_beats * Rational64::from_integer(2);
        self.clamp_width();
    }

    fn pan_time(&mut self, delta: Rational64) {
        self.origin_beat = snap_origin(self.origin_beat + delta);
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
        let start = Rational64::from_integer(floor_beat(t_min));
        let end = Rational64::from_integer(ceil_beat(t_max));
        let span = (end - start).max(Rational64::new(1, 1));
        vp.origin_beat = snap_origin(start);
        vp.width_beats = snap_width(span.max(Rational64::from_integer(4)));
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
    vp.origin_beat = snap_origin(mid - vp.width_beats / Rational64::from_integer(2));
    let half = (grid_rows / 2) as i32;
    vp.top_class = n.class + half.max(1);
}

fn cell_state(
    class: i32,
    t0: Rational64,
    t1: Rational64,
    clip: &Clip,
    primary: usize,
    selected: &BTreeSet<usize>,
) -> (char, bool, bool) {
    let mut best: Option<(usize, bool, bool)> = None;
    for (i, n) in clip.notes.iter().enumerate() {
        if n.class != class {
            continue;
        }
        if n.t_off.rational() <= t0 || n.t_on.rational() >= t1 {
            continue;
        }
        let is_primary = i == primary;
        let is_selected = selected.contains(&i);
        match best {
            None => best = Some((i, is_primary, is_selected)),
            Some((_, was_primary, was_selected)) => {
                if (is_primary && !was_primary)
                    || (is_selected && !was_selected && !is_primary && !was_primary)
                {
                    best = Some((i, is_primary, is_selected));
                }
            }
        }
    }
    match best {
        Some((_, true, _)) => ('█', true, true),
        Some((_, false, true)) => ('▒', false, true),
        Some(_) => ('▓', false, false),
        None => ('·', false, false),
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
    let tick_st = theme::label();
    let ph_st = Style::default()
        .fg(theme::ACCENT)
        .bg(theme::SURFACE)
        .add_modifier(Modifier::BOLD);
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
    primary: usize,
    selected: &BTreeSet<usize>,
    playhead_col: Option<u16>,
    scale: &Scale,
    reference_hz: f64,
) -> Line<'static> {
    let mut spans = Vec::new();
    let root_row = class.rem_euclid(12) == 0;
    let scale_row = is_scale_pitch(class, scale, reference_hz);
    for col in 0..cols {
        let (t0, t1) = vp.col_range(col, cols);
        let (ch, is_primary, is_selected) = cell_state(class, t0, t1, clip, primary, selected);
        let mut st = if is_primary {
            Style::default()
                .fg(theme::YELLOW)
                .bg(theme::PRIMARY_BG)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(theme::NOTE_COLOR)
                .bg(theme::SELECTED_BG)
                .add_modifier(Modifier::BOLD)
        } else if ch == '▓' {
            Style::default().fg(theme::NOTE_COLOR)
        } else if root_row {
            Style::default().fg(theme::DIM).bg(theme::GRID_ROOT)
        } else if scale_row {
            Style::default().fg(theme::DIM).bg(theme::GRID_SCALE)
        } else {
            theme::label()
        };
        if Some(col) == playhead_col {
            st = st.bg(theme::PLAYHEAD);
        }
        let ch = if ch == '·' && root_row { ':' } else { ch };
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

fn rational_abs(r: Rational64) -> Rational64 {
    if r < Rational64::from_integer(0) {
        -r
    } else {
        r
    }
}

fn f64_to_rational(v: f64) -> Rational64 {
    Rational64::new((v * 1024.0).round() as i64, 1024)
}

fn note_name(class: i32) -> String {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let midi = class.clamp(0, 127);
    let name = names[(midi.rem_euclid(12)) as usize];
    let octave = midi.div_euclid(12) - 1;
    format!("{name}{octave}")
}

fn midi_from_scale(scale: &Scale, degree: i32, octave: i32, reference_hz: f64) -> i32 {
    let pitch = scale.resolve(degree);
    let hz = Pitch(pitch.0 + octave as f64).to_hz(reference_hz);
    (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32
}

fn nearest_scale_pitch(midi: i32, scale: &Scale, reference_hz: f64) -> i32 {
    let mut best = midi.clamp(0, 127);
    let mut best_err = i32::MAX;
    for degree in -24..48 {
        for octave in -3..8 {
            let cand = midi_from_scale(scale, degree, octave, reference_hz).clamp(0, 127);
            let err = (cand - midi).abs();
            if err < best_err {
                best = cand;
                best_err = err;
            }
        }
    }
    best
}

fn is_scale_pitch(midi: i32, scale: &Scale, reference_hz: f64) -> bool {
    nearest_scale_pitch(midi, scale, reference_hz) == midi.clamp(0, 127)
}

fn next_scale_pitch(midi: i32, direction: i32, scale: &Scale, reference_hz: f64) -> i32 {
    let start = (midi + direction).clamp(0, 127);
    if direction >= 0 {
        for cand in start..=127 {
            if is_scale_pitch(cand, scale, reference_hz) {
                return cand;
            }
        }
        127
    } else {
        for cand in (0..=start).rev() {
            if is_scale_pitch(cand, scale, reference_hz) {
                return cand;
            }
        }
        0
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

fn beat_to_samples(beats: Rational64, bpm: f64, sample_rate: f64) -> usize {
    let beat_f = rational_to_f64(beats);
    let seconds = beat_f * 60.0 / bpm.max(1e-6);
    (seconds * sample_rate).round().max(0.0) as usize
}

fn midi_to_hz(midi: i32) -> f64 {
    let note = midi.clamp(0, 127) as f64;
    440.0 * 2.0_f64.powf((note - 69.0) / 12.0)
}

fn append_roll_clip_events(
    events: &mut Vec<TimedEvent>,
    clip: &Clip,
    block_start: Rational64,
    _default_voice: u32,
    bpm: f64,
    sample_rate: f64,
) {
    for note in &clip.notes {
        let voice = note.voice;
        let note_start = block_start + note.t_on.rational();
        let note_end = block_start + note.t_off.rational().max(note.t_on.rational() + min_dur());
        let on = beat_to_samples(note_start, bpm, sample_rate);
        let off = beat_to_samples(note_end, bpm, sample_rate).max(on.saturating_add(1));
        events.push(TimedEvent {
            sample_offset: on,
            event: GraphEvent::NoteOn {
                frequency: midi_to_hz(note.class),
                velocity: note.velocity.clamp(0.0, 1.0),
                voice,
            },
        });
        events.push(TimedEvent {
            sample_offset: off,
            event: GraphEvent::NoteOff { voice },
        });
    }
}

fn snap_origin(origin: Rational64) -> Rational64 {
    let step_f = rational_to_f64(snap_step());
    f64_to_rational((rational_to_f64(origin) / step_f).round() * step_f)
}

fn snap_width(width: Rational64) -> Rational64 {
    let candidates = [
        Rational64::new(1, 16),
        Rational64::new(1, 8),
        Rational64::new(1, 4),
        Rational64::new(1, 2),
        Rational64::from_integer(1),
        Rational64::from_integer(2),
        Rational64::from_integer(4),
        Rational64::from_integer(8),
        Rational64::from_integer(16),
        Rational64::from_integer(32),
        Rational64::from_integer(64),
        Rational64::from_integer(128),
        Rational64::from_integer(256),
    ];
    let width_f = rational_to_f64(width);
    candidates
        .into_iter()
        .min_by(|a, b| {
            let da = (rational_to_f64(*a) - width_f).abs();
            let db = (rational_to_f64(*b) - width_f).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(width)
}

/// Outcome of a key press in the pattern roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternRollOutcome {
    Stay,
    /// Merge clip into grid and exit fullscreen roll.
    CloseApply,
}

/// Audio preview source while editing the roll.
pub enum PatternRollPreview {
    /// Legacy step-grid preview for non-project editing.
    Grid(Grid),
    /// Exact clip timing on top of the rest of the authored scene.
    ProjectClip {
        background_events: Vec<TimedEvent>,
        block_start: Rational64,
        loop_beats: Rational64,
    },
}

/// ROL submode: navigation-first camera/jump/edit flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolMode {
    Pan,
    Jump,
    Edit,
    Attr,
}

impl RolMode {
    const ALL: [RolMode; 4] = [RolMode::Pan, RolMode::Jump, RolMode::Edit, RolMode::Attr];

    fn label(self) -> &'static str {
        match self {
            RolMode::Pan => "PAN",
            RolMode::Jump => "JUMP",
            RolMode::Edit => "EDIT",
            RolMode::Attr => "ATTR",
        }
    }

    fn intent(self) -> &'static str {
        match self {
            RolMode::Pan => "camera travel",
            RolMode::Jump => "note-relative nav",
            RolMode::Edit => "move and reshape",
            RolMode::Attr => "per-note attrs",
        }
    }

    fn cycle(self, delta: i32) -> Self {
        let len = Self::ALL.len() as i32;
        let idx = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0) as i32;
        Self::ALL[(idx + delta).rem_euclid(len) as usize]
    }
}

/// Focused per-note attribute while in `ATTR`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrField {
    Velocity,
    Voice,
    Duration,
}

impl AttrField {
    const ALL: [AttrField; 3] = [AttrField::Velocity, AttrField::Voice, AttrField::Duration];

    fn label(self) -> &'static str {
        match self {
            AttrField::Velocity => "VEL",
            AttrField::Voice => "VOICE",
            AttrField::Duration => "DUR",
        }
    }

    fn tool_label(self) -> &'static str {
        match self {
            AttrField::Velocity => "velocity",
            AttrField::Voice => "voice",
            AttrField::Duration => "duration",
        }
    }

    fn cycle(self, delta: i32) -> Self {
        let len = Self::ALL.len() as i32;
        let idx = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0) as i32;
        Self::ALL[(idx + delta).rem_euclid(len) as usize]
    }
}

pub struct PatternRoll {
    pub clip: Clip,
    primary: usize,
    selected: BTreeSet<usize>,
    pub dirty: bool,
    /// Step grid column (voice lane) being edited.
    pub grid_column: u32,
    /// Bridge voice id for that column (for status line).
    pub lane_voice: u32,
    vp: Viewport,
    did_autofit: bool,
    loop_beats_floor: BeatTime,
    preview: PatternRollPreview,
    scale: Scale,
    voice_ids: Vec<u32>,
    reference_hz: f64,
    swing: f64,
    visible_rows: u16,
    mode: RolMode,
    attr_field: AttrField,
}

impl PatternRoll {
    pub fn new(
        clip: Clip,
        grid_column: u32,
        loop_beats_floor: Rational64,
        lane_voice: u32,
        preview: PatternRollPreview,
        scale: Scale,
        voice_ids: Vec<u32>,
        reference_hz: f64,
        swing: f64,
    ) -> Self {
        Self {
            clip,
            primary: 0,
            selected: BTreeSet::from([0usize]),
            dirty: false,
            grid_column,
            lane_voice,
            vp: Viewport::default(),
            did_autofit: false,
            loop_beats_floor: BeatTime(loop_beats_floor),
            preview,
            scale,
            voice_ids,
            reference_hz,
            swing,
            visible_rows: 20,
            mode: RolMode::Pan,
            attr_field: AttrField::Velocity,
        }
    }

    pub fn mode_label(&self) -> &'static str {
        self.mode.label()
    }

    pub fn mode_intent(&self) -> &'static str {
        self.mode.intent()
    }

    pub fn attr_label(&self) -> &'static str {
        self.attr_field.label()
    }

    pub fn tool_label(&self) -> &'static str {
        match self.mode {
            RolMode::Pan => "pan",
            RolMode::Jump => "note-jump",
            RolMode::Edit => "move",
            RolMode::Attr => self.attr_field.tool_label(),
        }
    }

    pub fn selection_len(&self) -> usize {
        if self.clip.notes.is_empty() {
            0
        } else {
            self.selected.len().min(self.clip.notes.len()).max(1)
        }
    }

    pub fn primary_note(&self) -> Option<&ClipNote> {
        self.clip.notes.get(self.primary)
    }

    pub fn primary_index(&self) -> Option<usize> {
        self.clip.notes.get(self.primary).map(|_| self.primary)
    }

    pub fn validate_for_apply(&self) -> Result<(), String> {
        let mut c = self.clip.clone();
        sync_clip_length(&mut c, self.loop_beats_floor);
        RungFile::new(c).validate().map_err(|e| e.to_string())
    }

    pub fn push_preview(&mut self, bridge: &mut Bridge, bpm: f64, sample_rate: f64) {
        sync_clip_length(&mut self.clip, self.loop_beats_floor);
        match &self.preview {
            PatternRollPreview::Grid(preview_base) => {
                let mut g = preview_base.clone();
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
                let loop_beats = self
                    .clip
                    .length_beats
                    .map(|beats| beats.rational())
                    .unwrap_or_else(|| Rational64::from_integer(g.rows as i64));
                let loop_len = beat_to_samples(loop_beats, bpm, sample_rate);
                bridge.send(Command::LoadEvents { events, loop_len });
            }
            PatternRollPreview::ProjectClip {
                background_events,
                block_start,
                loop_beats,
            } => {
                let mut events = background_events.clone();
                append_roll_clip_events(
                    &mut events,
                    &self.clip,
                    *block_start,
                    self.lane_voice,
                    bpm,
                    sample_rate,
                );
                events.sort_by(trem::event::cmp_timed_event_delivery);
                let loop_len = beat_to_samples(*loop_beats, bpm, sample_rate);
                bridge.send(Command::LoadEvents { events, loop_len });
            }
        }
    }

    fn normalize_selection(&mut self) {
        let len = self.clip.notes.len();
        if len == 0 {
            self.primary = 0;
            self.selected.clear();
            return;
        }
        self.primary = self.primary.min(len - 1);
        self.selected = self
            .selected
            .iter()
            .copied()
            .filter(|idx| *idx < len)
            .collect();
        if self.selected.is_empty() {
            self.selected.insert(self.primary);
        }
        if !self.selected.contains(&self.primary) {
            self.primary = *self.selected.iter().next().unwrap_or(&0);
        }
    }

    fn replace_selection(&mut self, index: usize) {
        self.selected.clear();
        self.selected.insert(index);
        self.primary = index;
    }

    fn select_note(&mut self, index: usize, extend: bool) {
        if extend {
            self.selected.insert(index);
            self.primary = index;
        } else {
            self.replace_selection(index);
        }
    }

    fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    fn time_order_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.clip.notes.len()).collect();
        indices.sort_by(|a, b| {
            self.clip.notes[*a]
                .t_on
                .rational()
                .cmp(&self.clip.notes[*b].t_on.rational())
                .then_with(|| self.clip.notes[*a].class.cmp(&self.clip.notes[*b].class))
                .then_with(|| a.cmp(b))
        });
        indices
    }

    fn select_next_in_time(&mut self, delta: i32, extend: bool) {
        let ordered = self.time_order_indices();
        if ordered.is_empty() {
            return;
        }
        let pos = ordered
            .iter()
            .position(|idx| *idx == self.primary)
            .unwrap_or(0) as i32;
        let next = ordered[(pos + delta).rem_euclid(ordered.len() as i32) as usize];
        self.select_note(next, extend);
    }

    fn jump_pitch_neighbor(&mut self, direction: i32, extend: bool) {
        let Some(current) = self.primary_note() else {
            return;
        };
        let cur_start = current.t_on.rational();
        let cur_class = current.class;
        let candidate = self
            .clip
            .notes
            .iter()
            .enumerate()
            .filter(|(idx, note)| {
                *idx != self.primary
                    && if direction > 0 {
                        note.class > cur_class
                    } else {
                        note.class < cur_class
                    }
            })
            .min_by(|(a_idx, a), (b_idx, b)| {
                let a_key = (
                    (a.class - cur_class).abs(),
                    rational_abs(a.t_on.rational() - cur_start),
                    (*a_idx as i32 - self.primary as i32).abs(),
                );
                let b_key = (
                    (b.class - cur_class).abs(),
                    rational_abs(b.t_on.rational() - cur_start),
                    (*b_idx as i32 - self.primary as i32).abs(),
                );
                a_key.cmp(&b_key)
            })
            .map(|(idx, _)| idx)
            .or_else(|| {
                self.clip
                    .notes
                    .iter()
                    .enumerate()
                    .filter(|(_, note)| {
                        if direction > 0 {
                            note.class <= cur_class
                        } else {
                            note.class >= cur_class
                        }
                    })
                    .min_by_key(|(idx, note)| {
                        (
                            rational_abs(note.t_on.rational() - cur_start),
                            (*idx as i32 - self.primary as i32).abs(),
                        )
                    })
                    .map(|(idx, _)| idx)
            });
        if let Some(idx) = candidate {
            self.select_note(idx, extend);
        }
    }

    fn pan_pitch(&mut self, delta: i32) {
        self.vp.top_class += delta;
    }

    fn pan_time(&mut self, coarse: bool, direction: i32) {
        let step = if coarse {
            self.vp.width_beats / Rational64::from_integer(2)
        } else {
            self.vp.width_beats / Rational64::from_integer(8)
        };
        self.vp
            .pan_time(step * Rational64::from_integer(direction as i64));
    }

    fn move_selected_pitch(&mut self, direction: i32, coarse: bool) {
        for idx in self.selected_indices() {
            if let Some(note) = self.clip.notes.get_mut(idx) {
                note.class = if coarse {
                    (note.class + direction * 12).clamp(0, 127)
                } else {
                    next_scale_pitch(note.class, direction, &self.scale, self.reference_hz)
                };
                self.dirty = true;
            }
        }
    }

    fn transpose_selected_semitone(&mut self, direction: i32) {
        for idx in self.selected_indices() {
            if let Some(note) = self.clip.notes.get_mut(idx) {
                note.class = (note.class + direction).clamp(0, 127);
                self.dirty = true;
            }
        }
    }

    fn move_selected_time(&mut self, delta: Rational64) {
        let indices = self.selected_indices();
        if indices.is_empty() {
            return;
        }
        let min_on = indices
            .iter()
            .filter_map(|idx| self.clip.notes.get(*idx))
            .map(|note| note.t_on.rational())
            .min()
            .unwrap_or_else(|| Rational64::from_integer(0));
        let delta = if min_on + delta < Rational64::from_integer(0) {
            -min_on
        } else {
            delta
        };
        for idx in indices {
            if let Some(note) = self.clip.notes.get_mut(idx) {
                let length = note.t_off.rational() - note.t_on.rational();
                let new_on = note.t_on.rational() + delta;
                note.t_on = BeatTime(new_on);
                note.t_off = BeatTime(new_on + length.max(min_dur()));
                self.dirty = true;
            }
        }
    }

    fn snap_selection_to_neighbor(&mut self, direction: i32) {
        let Some(primary) = self.primary_note() else {
            return;
        };
        let current = primary.t_on.rational();
        let target = self
            .clip
            .notes
            .iter()
            .enumerate()
            .filter(|(idx, note)| {
                !self.selected.contains(idx)
                    && if direction > 0 {
                        note.t_on.rational() > current
                    } else {
                        note.t_on.rational() < current
                    }
            })
            .min_by_key(|(_, note)| rational_abs(note.t_on.rational() - current))
            .map(|(_, note)| note.t_on.rational());
        if let Some(target) = target {
            self.move_selected_time(target - current);
        }
    }

    fn resize_selected(&mut self, delta: Rational64) {
        for idx in self.selected_indices() {
            if let Some(note) = self.clip.notes.get_mut(idx) {
                note.t_off = BeatTime(note.t_off.rational() + delta);
                clamp_duration(note);
                self.dirty = true;
            }
        }
    }

    fn adjust_velocity(&mut self, delta: f64) {
        for idx in self.selected_indices() {
            if let Some(note) = self.clip.notes.get_mut(idx) {
                note.velocity = (note.velocity + delta).clamp(0.0, 1.0);
                self.dirty = true;
            }
        }
    }

    fn adjust_voice(&mut self, delta: i32) {
        for idx in self.selected_indices() {
            if let Some(note) = self.clip.notes.get_mut(idx) {
                note.voice = if delta < 0 {
                    note.voice.saturating_sub(delta.unsigned_abs())
                } else {
                    note.voice.saturating_add(delta as u32).min(127)
                };
                self.dirty = true;
            }
        }
    }

    fn adjust_attr_field(&mut self, direction: i32, coarse: bool) {
        match self.attr_field {
            AttrField::Velocity => {
                self.adjust_velocity(if coarse { 0.15 } else { 0.05 } * direction as f64)
            }
            AttrField::Voice => self.adjust_voice(if coarse { direction * 4 } else { direction }),
            AttrField::Duration => {
                let step = if coarse {
                    Rational64::from_integer(direction as i64)
                } else {
                    beat_step() * Rational64::from_integer(direction as i64)
                };
                self.resize_selected(step);
            }
        }
    }

    fn delete_selected(&mut self) {
        let mut indices = self.selected_indices();
        if indices.is_empty() {
            return;
        }
        indices.sort_unstable();
        let fallback = indices[0].saturating_sub(1);
        for idx in indices.into_iter().rev() {
            if idx < self.clip.notes.len() {
                self.clip.notes.remove(idx);
            }
        }
        self.dirty = true;
        self.selected.clear();
        if !self.clip.notes.is_empty() {
            let next = fallback.min(self.clip.notes.len() - 1);
            self.replace_selection(next);
        }
    }

    fn duplicate_selected(&mut self) {
        let mut new_indices = Vec::new();
        for idx in self.selected_indices() {
            if let Some(note) = self.clip.notes.get(idx).cloned() {
                let mut dup = note;
                dup.t_on = BeatTime(dup.t_on.rational() + Rational64::from_integer(1));
                dup.t_off = BeatTime(dup.t_off.rational() + Rational64::from_integer(1));
                self.clip.notes.push(dup);
                new_indices.push(self.clip.notes.len() - 1);
                self.dirty = true;
            }
        }
        if let Some(last) = new_indices.last().copied() {
            self.selected = new_indices.into_iter().collect();
            self.primary = last;
        }
    }

    fn insert_note(&mut self) {
        let (start, class, velocity, voice) = self
            .primary_note()
            .map(|note| {
                (
                    note.t_on.rational() + beat_step(),
                    next_scale_pitch(note.class, 1, &self.scale, self.reference_hz),
                    note.velocity,
                    note.voice,
                )
            })
            .unwrap_or_else(|| {
                let center_beat =
                    self.vp.origin_beat + self.vp.width_beats / Rational64::from_integer(4);
                let center_class = self.vp.top_class - 6;
                (
                    center_beat.max(Rational64::from_integer(0)),
                    nearest_scale_pitch(center_class, &self.scale, self.reference_hz),
                    0.8,
                    self.lane_voice,
                )
            });
        self.clip.notes.push(ClipNote {
            id: None,
            class,
            t_on: BeatTime(start),
            t_off: BeatTime(start + Rational64::new(1, 4)),
            voice,
            velocity,
            meta: Default::default(),
        });
        let idx = self.clip.notes.len().saturating_sub(1);
        self.replace_selection(idx);
        self.dirty = true;
    }

    fn ensure_selected_visible(&mut self) {
        let grid_rows = self.visible_rows.max(4);
        if let Some((note_class, note_on, note_off)) = self
            .primary_note()
            .map(|note| (note.class, note.t_on.rational(), note.t_off.rational()))
        {
            let pitch_margin = 1;
            if note_class >= self.vp.top_class - pitch_margin {
                self.vp.top_class = note_class + pitch_margin;
            }
            let bottom = self.vp.top_class - grid_rows as i32 + 1;
            if note_class <= bottom + pitch_margin {
                self.vp.top_class = note_class + grid_rows as i32 / 2;
            }
            let left = self.vp.origin_beat;
            let right = self.vp.origin_beat + self.vp.width_beats;
            if note_on < left || note_off > right {
                center_on_selected(&self.clip, self.primary, grid_rows, &mut self.vp);
            }
        }
    }

    fn commit_preview(
        &mut self,
        bridge: &mut Bridge,
        bpm: f64,
        sample_rate: f64,
    ) -> PatternRollOutcome {
        self.ensure_selected_visible();
        self.push_preview(bridge, bpm, sample_rate);
        PatternRollOutcome::Stay
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
        self.normalize_selection();
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Esc => PatternRollOutcome::CloseApply,
            KeyCode::Tab => {
                self.mode = self.mode.cycle(1);
                PatternRollOutcome::Stay
            }
            KeyCode::BackTab => {
                self.mode = self.mode.cycle(-1);
                PatternRollOutcome::Stay
            }
            KeyCode::Delete | KeyCode::Backspace => {
                self.delete_selected();
                self.push_preview(bridge, bpm, sample_rate);
                PatternRollOutcome::Stay
            }
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
                self.select_next_in_time(1, shift);
                self.ensure_selected_visible();
                PatternRollOutcome::Stay
            }
            KeyCode::Char('b') => {
                self.select_next_in_time(-1, shift);
                self.ensure_selected_visible();
                PatternRollOutcome::Stay
            }
            KeyCode::Char('a') if ctrl => {
                self.selected = (0..self.clip.notes.len()).collect();
                if !self.clip.notes.is_empty() {
                    self.primary = self.primary.min(self.clip.notes.len() - 1);
                }
                PatternRollOutcome::Stay
            }
            KeyCode::Char('a') => {
                fit_all(&self.clip, &mut self.vp);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('g') => {
                center_on_selected(
                    &self.clip,
                    self.primary,
                    self.visible_rows.max(4),
                    &mut self.vp,
                );
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
            KeyCode::Char('n') => {
                self.insert_note();
                self.commit_preview(bridge, bpm, sample_rate)
            }
            KeyCode::Char('d') => {
                self.duplicate_selected();
                self.commit_preview(bridge, bpm, sample_rate)
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.transpose_selected_semitone(1);
                self.commit_preview(bridge, bpm, sample_rate)
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                self.transpose_selected_semitone(-1);
                self.commit_preview(bridge, bpm, sample_rate)
            }
            KeyCode::Char(']') => {
                self.resize_selected(beat_step());
                self.commit_preview(bridge, bpm, sample_rate)
            }
            KeyCode::Char('[') => {
                self.resize_selected(-beat_step());
                self.commit_preview(bridge, bpm, sample_rate)
            }
            KeyCode::Char('1') => {
                self.adjust_velocity(-0.05);
                self.push_preview(bridge, bpm, sample_rate);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('2') => {
                self.adjust_velocity(0.05);
                self.push_preview(bridge, bpm, sample_rate);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('e') => {
                self.adjust_voice(-1);
                self.push_preview(bridge, bpm, sample_rate);
                PatternRollOutcome::Stay
            }
            KeyCode::Char('r') => {
                self.adjust_voice(1);
                self.push_preview(bridge, bpm, sample_rate);
                PatternRollOutcome::Stay
            }
            KeyCode::Left | KeyCode::Char('h') => match self.mode {
                RolMode::Pan => {
                    self.pan_time(shift, -1);
                    PatternRollOutcome::Stay
                }
                RolMode::Jump => {
                    self.select_next_in_time(-1, shift);
                    self.ensure_selected_visible();
                    PatternRollOutcome::Stay
                }
                RolMode::Edit => {
                    if ctrl {
                        self.snap_selection_to_neighbor(-1);
                    } else {
                        let step = if shift {
                            Rational64::from_integer(-1)
                        } else {
                            -beat_step()
                        };
                        self.move_selected_time(step);
                    }
                    self.commit_preview(bridge, bpm, sample_rate)
                }
                RolMode::Attr => {
                    self.attr_field = self.attr_field.cycle(-1);
                    PatternRollOutcome::Stay
                }
            },
            KeyCode::Right | KeyCode::Char('l') => match self.mode {
                RolMode::Pan => {
                    self.pan_time(shift, 1);
                    PatternRollOutcome::Stay
                }
                RolMode::Jump => {
                    self.select_next_in_time(1, shift);
                    self.ensure_selected_visible();
                    PatternRollOutcome::Stay
                }
                RolMode::Edit => {
                    if ctrl {
                        self.snap_selection_to_neighbor(1);
                    } else {
                        let step = if shift {
                            Rational64::from_integer(1)
                        } else {
                            beat_step()
                        };
                        self.move_selected_time(step);
                    }
                    self.commit_preview(bridge, bpm, sample_rate)
                }
                RolMode::Attr => {
                    self.attr_field = self.attr_field.cycle(1);
                    PatternRollOutcome::Stay
                }
            },
            KeyCode::Up | KeyCode::Char('k') => match self.mode {
                RolMode::Pan => {
                    self.pan_pitch(if shift { 12 } else { 1 });
                    PatternRollOutcome::Stay
                }
                RolMode::Jump => {
                    self.jump_pitch_neighbor(1, shift);
                    self.ensure_selected_visible();
                    PatternRollOutcome::Stay
                }
                RolMode::Edit => {
                    self.move_selected_pitch(1, shift);
                    self.commit_preview(bridge, bpm, sample_rate)
                }
                RolMode::Attr => {
                    self.adjust_attr_field(1, shift);
                    self.push_preview(bridge, bpm, sample_rate);
                    PatternRollOutcome::Stay
                }
            },
            KeyCode::Down | KeyCode::Char('j') => match self.mode {
                RolMode::Pan => {
                    self.pan_pitch(if shift { -12 } else { -1 });
                    PatternRollOutcome::Stay
                }
                RolMode::Jump => {
                    self.jump_pitch_neighbor(-1, shift);
                    self.ensure_selected_visible();
                    PatternRollOutcome::Stay
                }
                RolMode::Edit => {
                    self.move_selected_pitch(-1, shift);
                    self.commit_preview(bridge, bpm, sample_rate)
                }
                RolMode::Attr => {
                    self.adjust_attr_field(-1, shift);
                    self.push_preview(bridge, bpm, sample_rate);
                    PatternRollOutcome::Stay
                }
            },
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
        playing: bool,
        beat_position: f64,
        loop_beats: f64,
    ) {
        self.autofit_if_needed(area.height);
        self.normalize_selection();

        let title = if self.dirty {
            format!(
                " * ROL · {} · {} · pitch ↑ time → ",
                self.mode.label(),
                self.attr_field.label()
            )
        } else {
            format!(
                " ROL · {} · {} · pitch ↑ time → ",
                self.mode.label(),
                self.attr_field.label()
            )
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title_style(theme::title())
            .style(theme::panel())
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let cols_lr = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(8), Constraint::Min(8)])
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
        self.visible_rows = grid_h;

        let loop_len = loop_beats.max(1e-9);
        let playhead_beat = beat_position.rem_euclid(loop_len);
        if playing
            && (playhead_beat < rational_to_f64(self.vp.origin_beat)
                || playhead_beat >= rational_to_f64(self.vp.origin_beat + self.vp.width_beats))
        {
            self.vp.origin_beat = f64_to_rational(
                (playhead_beat - rational_to_f64(self.vp.width_beats) * 0.25).max(0.0),
            );
        }
        let vp = self.vp;
        let playhead_col = playhead_col_f64(vp, playhead_beat, roll_cols);

        let ruler = Paragraph::new(ruler_line(vp, ruler_area.width.max(1), playhead_col));
        frame.render_widget(ruler, ruler_area);

        let clip_ref = &self.clip;
        let primary = self.primary;
        let selected = &self.selected;
        let lines: Vec<Line> = (0..grid_h)
            .map(|row| {
                let class = vp.top_class - row as i32;
                render_roll_row(
                    class,
                    vp,
                    roll_cols,
                    clip_ref,
                    primary,
                    selected,
                    playhead_col,
                    &self.scale,
                    self.reference_hz,
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(Text::from(lines)), grid_area);

        let rows_left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(4)])
            .split(left_col);

        let label_head =
            Paragraph::new(Span::styled("pitch", theme::label())).alignment(Alignment::Right);
        frame.render_widget(label_head, rows_left[0]);

        let label_lines: Vec<Line> = (0..grid_h)
            .map(|row| {
                let class = vp.top_class - row as i32;
                let style = if class.rem_euclid(12) == 0 {
                    Style::default()
                        .fg(theme::YELLOW)
                        .add_modifier(Modifier::BOLD)
                } else if is_scale_pitch(class, &self.scale, self.reference_hz) {
                    Style::default().fg(theme::GREEN)
                } else {
                    theme::label()
                };
                Line::from(Span::styled(
                    format!("{:>7}", format!("{} {}", note_name(class), class)),
                    style,
                ))
            })
            .collect();
        let labels = Paragraph::new(Text::from(label_lines))
            .alignment(Alignment::Right)
            .block(Block::default().borders(Borders::RIGHT));
        frame.render_widget(labels, rows_left[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;
    use trem::pitch::Tuning;
    use trem_rta::create_bridge;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn test_roll() -> PatternRoll {
        let clip = Clip {
            notes: vec![
                ClipNote {
                    id: None,
                    class: 60,
                    t_on: BeatTime(Rational64::from_integer(0)),
                    t_off: BeatTime(Rational64::new(1, 2)),
                    voice: 0,
                    velocity: 0.6,
                    meta: Default::default(),
                },
                ClipNote {
                    id: None,
                    class: 64,
                    t_on: BeatTime(Rational64::from_integer(1)),
                    t_off: BeatTime(Rational64::new(3, 2)),
                    voice: 0,
                    velocity: 0.7,
                    meta: Default::default(),
                },
                ClipNote {
                    id: None,
                    class: 67,
                    t_on: BeatTime(Rational64::from_integer(2)),
                    t_off: BeatTime(Rational64::new(5, 2)),
                    voice: 0,
                    velocity: 0.8,
                    meta: Default::default(),
                },
            ],
            length_beats: Some(BeatTime(Rational64::from_integer(4))),
        };
        PatternRoll::new(
            clip,
            0,
            Rational64::from_integer(4),
            0,
            PatternRollPreview::Grid(Grid::new(4, 1)),
            Tuning::edo12().to_scale(),
            vec![0],
            440.0,
            0.0,
        )
    }

    #[test]
    fn tab_cycles_rol_modes() {
        let mut roll = test_roll();
        let (mut bridge, _) = create_bridge(32);
        let mut playing = false;
        let mut engine = false;
        assert_eq!(roll.mode_label(), "PAN");
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        assert_eq!(roll.mode_label(), "JUMP");
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        assert_eq!(roll.mode_label(), "EDIT");
    }

    #[test]
    fn jump_mode_shift_extends_selection() {
        let mut roll = test_roll();
        let (mut bridge, _) = create_bridge(32);
        let mut playing = false;
        let mut engine = false;
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        roll.handle_key(
            press(KeyCode::Char('l'), KeyModifiers::SHIFT),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        assert_eq!(roll.selection_len(), 2);
        assert_eq!(roll.primary_index(), Some(1));
    }

    #[test]
    fn edit_mode_ctrl_right_snaps_to_next_note() {
        let mut roll = test_roll();
        let (mut bridge, _) = create_bridge(32);
        let mut playing = false;
        let mut engine = false;
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        roll.handle_key(
            press(KeyCode::Right, KeyModifiers::CONTROL),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        assert_eq!(
            roll.primary_note().map(|note| note.t_on.rational()),
            Some(Rational64::from_integer(1))
        );
    }

    #[test]
    fn attr_mode_adjusts_velocity() {
        let mut roll = test_roll();
        let (mut bridge, _) = create_bridge(32);
        let mut playing = false;
        let mut engine = false;
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        roll.handle_key(
            press(KeyCode::Tab, KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        let before = roll.primary_note().map(|note| note.velocity).unwrap_or(0.0);
        roll.handle_key(
            press(KeyCode::Char('k'), KeyModifiers::NONE),
            &mut bridge,
            120.0,
            44_100.0,
            &mut playing,
            &mut engine,
        );
        let after = roll.primary_note().map(|note| note.velocity).unwrap_or(0.0);
        assert!(after > before);
    }
}

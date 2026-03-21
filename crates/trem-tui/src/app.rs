//! Main TUI application: grid, views, transport, and [`trem_cpal::Bridge`] integration.
//!
//! [`App::run`] is the event loop (draw, input, non-blocking audio poll).

use crate::input::{self, Action, Mode, View};
use crate::project::ProjectData;
use crate::view::graph::GraphViewWidget;
use crate::view::info::InfoView;
use crate::view::pattern::PatternView;
use crate::view::scope::ScopeView;
use crate::view::transport::TransportView;

use trem::event::NoteEvent;
use trem::graph::ParamDescriptor;
use trem::math::Rational;
use trem::pitch::Pitch;
use trem_cpal::{Bridge, Command, Notification};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const GATE_PRESETS: [(i64, u64); 4] = [(1, 4), (1, 2), (3, 4), (7, 8)];

fn cycle_gate(current: Rational) -> Rational {
    for (i, &(n, d)) in GATE_PRESETS.iter().enumerate() {
        if current == Rational::new(n, d) {
            let next = GATE_PRESETS[(i + 1) % GATE_PRESETS.len()];
            return Rational::new(next.0, next.1);
        }
    }
    Rational::new(1, 4)
}

/// Mutable state for the full terminal UI: pattern/graph views, audio bridge, and layout data.
pub struct App {
    pub grid: trem::grid::Grid,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub mode: Mode,
    pub view: View,
    pub bpm: f64,
    pub playing: bool,
    pub beat_position: f64,
    pub current_play_row: Option<u32>,
    pub scale: trem::pitch::Scale,
    pub scale_name: String,
    pub octave: i32,
    pub bridge: Bridge,
    pub scope_buf: Vec<f32>,
    pub peak_l: f32,
    pub peak_r: f32,
    pub should_quit: bool,
    pub instrument_names: Vec<String>,
    pub voice_ids: Vec<u32>,
    pub graph_nodes: Vec<(u32, String)>,
    pub graph_edges: Vec<(u32, u16, u32, u16)>,
    pub graph_cursor: usize,
    pub graph_depths: Vec<usize>,
    pub graph_layers: Vec<Vec<usize>>,
    pub graph_params: Vec<Vec<ParamDescriptor>>,
    pub graph_param_values: Vec<Vec<f64>>,
    pub param_cursor: usize,
    pub swing: f64,
    pub euclidean_k: u32,
    pub undo_stack: Vec<Vec<Option<NoteEvent>>>,
    pub redo_stack: Vec<Vec<Option<NoteEvent>>>,
    rng_state: u64,
    preview_note_off: Option<(u32, Instant)>,
}

impl App {
    /// Initial pattern view, scale metadata, and per-column voice IDs for [`trem_cpal::Command::NoteOn`].
    pub fn new(
        grid: trem::grid::Grid,
        scale: trem::pitch::Scale,
        scale_name: String,
        bridge: Bridge,
        instrument_names: Vec<String>,
        voice_ids: Vec<u32>,
    ) -> Self {
        Self {
            grid,
            cursor_row: 0,
            cursor_col: 0,
            mode: Mode::Normal,
            view: View::Pattern,
            bpm: 120.0,
            playing: false,
            beat_position: 0.0,
            current_play_row: None,
            scale,
            scale_name,
            octave: 0,
            bridge,
            scope_buf: Vec::new(),
            peak_l: 0.0,
            peak_r: 0.0,
            should_quit: false,
            instrument_names,
            voice_ids,
            graph_nodes: Vec::new(),
            graph_edges: Vec::new(),
            graph_cursor: 0,
            graph_depths: Vec::new(),
            graph_layers: Vec::new(),
            graph_params: Vec::new(),
            graph_param_values: Vec::new(),
            param_cursor: 0,
            swing: 0.0,
            euclidean_k: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            rng_state: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            preview_note_off: None,
        }
    }

    /// Attaches node/edge/param snapshots for the graph editor (from the host graph).
    pub fn with_graph_info(
        mut self,
        nodes: Vec<(u32, String)>,
        edges: Vec<(u32, u16, u32, u16)>,
        params: Vec<(Vec<ParamDescriptor>, Vec<f64>)>,
    ) -> Self {
        let (depths, layers) = crate::view::graph::compute_graph_nav(&nodes, &edges);
        self.graph_nodes = nodes;
        self.graph_edges = edges;
        self.graph_depths = depths;
        self.graph_layers = layers;
        self.graph_params = params.iter().map(|(d, _)| d.clone()).collect();
        self.graph_param_values = params.into_iter().map(|(_, v)| v).collect();
        self
    }

    /// Applies one [`Action`] from input: updates state and sends [`Command`]s to the audio bridge as needed.
    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::CycleView => {
                self.view = self.view.next();
                self.mode = Mode::Normal;
            }
            Action::ToggleEdit => {
                self.mode = match self.mode {
                    Mode::Normal => {
                        self.param_cursor = 0;
                        Mode::Edit
                    }
                    Mode::Edit => Mode::Normal,
                };
            }
            Action::TogglePlay => {
                self.playing = !self.playing;
                if self.playing {
                    self.send_pattern();
                    self.bridge.send(Command::Play);
                } else {
                    self.bridge.send(Command::Stop);
                    self.current_play_row = None;
                }
            }
            Action::MoveUp => match (&self.view, &self.mode) {
                (View::Pattern, _) => {
                    self.cursor_col = self.cursor_col.saturating_sub(1);
                }
                (View::Graph, Mode::Normal) => self.graph_move_up(),
                (View::Graph, Mode::Edit) => {
                    self.param_cursor = self.param_cursor.saturating_sub(1);
                }
            },
            Action::MoveDown => match (&self.view, &self.mode) {
                (View::Pattern, _) => {
                    if self.cursor_col < self.grid.columns.saturating_sub(1) {
                        self.cursor_col += 1;
                    }
                }
                (View::Graph, Mode::Normal) => self.graph_move_down(),
                (View::Graph, Mode::Edit) => {
                    let max = self.current_node_param_count().saturating_sub(1);
                    if self.param_cursor < max {
                        self.param_cursor += 1;
                    }
                }
            },
            Action::MoveLeft => match (&self.view, &self.mode) {
                (View::Pattern, _) => {
                    self.cursor_row = self.cursor_row.saturating_sub(1);
                }
                (View::Graph, Mode::Normal) => self.graph_move_left(),
                (View::Graph, Mode::Edit) => self.adjust_param(-0.01),
            },
            Action::MoveRight => match (&self.view, &self.mode) {
                (View::Pattern, _) => {
                    if self.cursor_row < self.grid.rows.saturating_sub(1) {
                        self.cursor_row += 1;
                    }
                }
                (View::Graph, Mode::Normal) => self.graph_move_right(),
                (View::Graph, Mode::Edit) => self.adjust_param(0.01),
            },
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::SaveProject => {
                let path = PathBuf::from("project.trem.json");
                let data = ProjectData::from_app(self);
                if let Err(e) = crate::project::save(&path, &data) {
                    eprintln!("save error: {e}");
                }
            }
            Action::LoadProject => {
                let path = PathBuf::from("project.trem.json");
                match crate::project::load(&path) {
                    Ok(data) => {
                        self.push_undo();
                        data.apply_to_app(self);
                        if self.playing {
                            self.send_pattern();
                        }
                    }
                    Err(e) => eprintln!("load error: {e}"),
                }
            }
            Action::SwingUp => {
                self.swing = (self.swing + 0.05).min(0.9);
                if self.playing {
                    self.send_pattern();
                }
            }
            Action::SwingDown => {
                self.swing = (self.swing - 0.05).max(0.0);
                if self.playing {
                    self.send_pattern();
                }
            }
            Action::GateCycle => {
                if self.view == View::Pattern {
                    if let Some(note) = self.grid.get(self.cursor_row, self.cursor_col).cloned() {
                        self.push_undo();
                        let new_gate = cycle_gate(note.gate);
                        let mut updated = note;
                        updated.gate = new_gate;
                        self.grid
                            .set(self.cursor_row, self.cursor_col, Some(updated));
                        if self.playing {
                            self.send_pattern();
                        }
                    }
                }
            }
            Action::NoteInput(degree) => {
                if self.view != View::Pattern {
                    return;
                }
                self.push_undo();
                let event = NoteEvent::new(degree, self.octave, Rational::new(3, 4));
                let voice_id = self
                    .voice_ids
                    .get(self.cursor_col as usize)
                    .copied()
                    .unwrap_or(0);

                if let Some((old_voice, _)) = self.preview_note_off.take() {
                    self.bridge.send(Command::NoteOff { voice: old_voice });
                }

                let pitch = self.scale.resolve(degree);
                let freq = Pitch(pitch.0 + self.octave as f64).to_hz(440.0);
                let vel = event.velocity.to_f64();
                self.bridge.send(Command::NoteOn {
                    frequency: freq,
                    velocity: vel,
                    voice: voice_id,
                });
                self.preview_note_off = Some((voice_id, Instant::now()));

                self.grid.set(self.cursor_row, self.cursor_col, Some(event));

                if self.cursor_row < self.grid.rows.saturating_sub(1) {
                    self.cursor_row += 1;
                } else {
                    self.cursor_row = 0;
                    if self.cursor_col < self.grid.columns.saturating_sub(1) {
                        self.cursor_col += 1;
                    }
                }
            }
            Action::DeleteNote => {
                if self.view != View::Pattern {
                    return;
                }
                self.push_undo();
                self.grid.set(self.cursor_row, self.cursor_col, None);
            }
            Action::OctaveUp => self.octave = (self.octave + 1).min(9),
            Action::OctaveDown => self.octave = (self.octave - 1).max(-4),
            Action::BpmUp => {
                if self.view == View::Graph && self.mode == Mode::Edit {
                    self.adjust_param(0.001);
                } else {
                    self.bpm = (self.bpm + 1.0).min(300.0);
                    self.bridge.send(Command::SetBpm(self.bpm));
                }
            }
            Action::BpmDown => {
                if self.view == View::Graph && self.mode == Mode::Edit {
                    self.adjust_param(-0.001);
                } else {
                    self.bpm = (self.bpm - 1.0).max(20.0);
                    self.bridge.send(Command::SetBpm(self.bpm));
                }
            }
            Action::EuclideanFill => {
                if self.view == View::Pattern {
                    self.push_undo();
                    self.euclidean_k = (self.euclidean_k + 1) % (self.grid.rows + 1);
                    let pattern = trem::euclidean::euclidean(self.euclidean_k, self.grid.rows);
                    let template = NoteEvent::new(0, self.octave, Rational::new(3, 4));
                    self.grid
                        .fill_euclidean(self.cursor_col, &pattern, template);
                    if self.playing {
                        self.send_pattern();
                    }
                }
            }
            Action::RandomizeVoice => {
                if self.view == View::Pattern {
                    self.push_undo();
                    self.randomize_current_voice();
                    if self.playing {
                        self.send_pattern();
                    }
                }
            }
            Action::ReverseVoice => {
                if self.view == View::Pattern {
                    self.push_undo();
                    self.grid.reverse_voice(self.cursor_col);
                    if self.playing {
                        self.send_pattern();
                    }
                }
            }
            Action::ShiftVoiceLeft => {
                if self.view == View::Pattern {
                    self.push_undo();
                    self.grid.shift_voice(self.cursor_col, -1);
                    if self.playing {
                        self.send_pattern();
                    }
                }
            }
            Action::ShiftVoiceRight => {
                if self.view == View::Pattern {
                    self.push_undo();
                    self.grid.shift_voice(self.cursor_col, 1);
                    if self.playing {
                        self.send_pattern();
                    }
                }
            }
            Action::VelocityUp => {
                if self.view == View::Pattern {
                    self.push_undo();
                    self.adjust_note_velocity(Rational::new(1, 8));
                    if self.playing {
                        self.send_pattern();
                    }
                }
            }
            Action::VelocityDown => {
                if self.view == View::Pattern {
                    self.push_undo();
                    self.adjust_note_velocity(Rational::new(-1, 8));
                    if self.playing {
                        self.send_pattern();
                    }
                }
            }
        }
    }

    fn current_node_param_count(&self) -> usize {
        self.graph_params
            .get(self.graph_cursor)
            .map_or(0, |p| p.len())
    }

    fn adjust_param(&mut self, fraction: f64) {
        let params = match self.graph_params.get(self.graph_cursor) {
            Some(p) if !p.is_empty() => p,
            _ => return,
        };
        let desc = match params.get(self.param_cursor) {
            Some(d) => d,
            None => return,
        };
        let values = match self.graph_param_values.get_mut(self.graph_cursor) {
            Some(v) => v,
            None => return,
        };
        let range = desc.max - desc.min;
        let step = range * fraction;
        let old = values[self.param_cursor];
        let new_val = (old + step).clamp(desc.min, desc.max);
        values[self.param_cursor] = new_val;

        let node_id = self.graph_nodes[self.graph_cursor].0;
        self.bridge.send(Command::SetParam {
            node: node_id,
            param_id: desc.id,
            value: new_val,
        });
    }

    fn next_rng(&mut self) -> u64 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.rng_state
    }

    fn randomize_current_voice(&mut self) {
        let col = self.cursor_col;
        let scale_len = self.scale.len() as i32;
        for row in 0..self.grid.rows {
            let r = self.next_rng();
            if r % 100 < 40 {
                let degree = (self.next_rng() % scale_len.max(1) as u64) as i32;
                let vel_n = (self.next_rng() % 6 + 2) as i64; // 2..8
                let event = NoteEvent::new(degree, self.octave, Rational::new(vel_n, 8));
                self.grid.set(row, col, Some(event));
            } else {
                self.grid.set(row, col, None);
            }
        }
    }

    fn adjust_note_velocity(&mut self, delta: Rational) {
        if let Some(note) = self.grid.get(self.cursor_row, self.cursor_col).cloned() {
            let new_vel = note.velocity + delta;
            let clamped = if new_vel.to_f64() < 0.0625 {
                Rational::new(1, 16)
            } else if new_vel.to_f64() > 1.0 {
                Rational::new(1, 1)
            } else {
                new_vel
            };
            let mut updated = note;
            updated.velocity = clamped;
            self.grid
                .set(self.cursor_row, self.cursor_col, Some(updated));
        }
    }

    fn graph_move_up(&mut self) {
        if self.graph_depths.is_empty() {
            return;
        }
        let depth = self.graph_depths[self.graph_cursor];
        let layer = &self.graph_layers[depth];
        if let Some(pos) = layer.iter().position(|&i| i == self.graph_cursor) {
            if pos > 0 {
                self.graph_cursor = layer[pos - 1];
            }
        }
    }

    fn graph_move_down(&mut self) {
        if self.graph_depths.is_empty() {
            return;
        }
        let depth = self.graph_depths[self.graph_cursor];
        let layer = &self.graph_layers[depth];
        if let Some(pos) = layer.iter().position(|&i| i == self.graph_cursor) {
            if pos + 1 < layer.len() {
                self.graph_cursor = layer[pos + 1];
            }
        }
    }

    fn graph_move_right(&mut self) {
        let current_id = self.graph_nodes[self.graph_cursor].0;
        let mut seen = HashSet::new();
        for &(src, _, dst, _) in &self.graph_edges {
            if src == current_id && seen.insert(dst) {
                if let Some(idx) = self.graph_nodes.iter().position(|(id, _)| *id == dst) {
                    self.graph_cursor = idx;
                    return;
                }
            }
        }
    }

    fn graph_move_left(&mut self) {
        let current_id = self.graph_nodes[self.graph_cursor].0;
        let mut seen = HashSet::new();
        for &(src, _, dst, _) in &self.graph_edges {
            if dst == current_id && seen.insert(src) {
                if let Some(idx) = self.graph_nodes.iter().position(|(id, _)| *id == src) {
                    self.graph_cursor = idx;
                    return;
                }
            }
        }
    }

    /// Drains pending [`Notification`]s and timed preview note-off; call each frame from the UI loop.
    pub fn poll_audio(&mut self) {
        // Handle preview note release
        if let Some((voice, time)) = self.preview_note_off {
            if time.elapsed() > Duration::from_millis(120) {
                self.bridge.send(Command::NoteOff { voice });
                self.preview_note_off = None;
            }
        }

        while let Some(notif) = self.bridge.try_recv() {
            match notif {
                Notification::Position { beat } => {
                    self.beat_position = beat;
                    let total_beats = self.grid.rows as f64;
                    if total_beats > 0.0 {
                        let row = (beat % total_beats) as u32;
                        self.current_play_row = Some(row.min(self.grid.rows.saturating_sub(1)));
                    }
                }
                Notification::ScopeData(data) => {
                    self.scope_buf = data;
                }
                Notification::Meter { peak_l, peak_r } => {
                    self.peak_l = peak_l;
                    self.peak_r = peak_r;
                }
                Notification::Stopped => {
                    self.playing = false;
                    self.current_play_row = None;
                }
            }
        }
    }

    fn send_pattern(&mut self) {
        let beats = Rational::integer(self.grid.rows as i64);
        let events = trem::render::grid_to_timed_events(
            &self.grid,
            beats,
            self.bpm,
            44100.0,
            &self.scale,
            440.0,
            &self.voice_ids,
            self.swing,
        );
        self.bridge.send(Command::LoadEvents(events));
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.grid.cells.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(self.grid.cells.clone());
            self.grid.cells = snapshot;
            if self.playing {
                self.send_pattern();
            }
        }
    }

    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(self.grid.cells.clone());
            self.grid.cells = snapshot;
            if self.playing {
                self.send_pattern();
            }
        }
    }

    /// Lays out transport, sidebar, main view (pattern or graph), and scope into `frame`.
    pub fn draw(&self, frame: &mut ratatui::Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(4),
                Constraint::Length(5),
            ])
            .split(frame.area());

        frame.render_widget(
            TransportView {
                bpm: self.bpm,
                beat_position: self.beat_position,
                playing: self.playing,
                mode: &self.mode,
                view: &self.view,
                scale_name: &self.scale_name,
                octave: self.octave,
                swing: self.swing,
            },
            outer[0],
        );

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(26), Constraint::Min(20)])
            .split(outer[1]);

        let note_at_cursor = self.grid.get(self.cursor_row, self.cursor_col);

        frame.render_widget(
            InfoView {
                mode: &self.mode,
                view: &self.view,
                octave: self.octave,
                cursor_step: self.cursor_row,
                cursor_voice: self.cursor_col,
                grid_steps: self.grid.rows,
                grid_voices: self.grid.columns,
                note_at_cursor,
                scale: &self.scale,
                scale_name: &self.scale_name,
                peak_l: self.peak_l,
                peak_r: self.peak_r,
                instrument_names: &self.instrument_names,
                swing: self.swing,
                euclidean_k: self.euclidean_k,
                undo_depth: self.undo_stack.len(),
            },
            middle[0],
        );

        match self.view {
            View::Pattern => {
                frame.render_widget(
                    PatternView {
                        grid: &self.grid,
                        cursor_row: self.cursor_row,
                        cursor_col: self.cursor_col,
                        current_play_row: self.current_play_row,
                        mode: &self.mode,
                        scale: &self.scale,
                        instrument_names: &self.instrument_names,
                    },
                    middle[1],
                );
            }
            View::Graph => {
                let params = self.graph_params.get(self.graph_cursor);
                let values = self.graph_param_values.get(self.graph_cursor);
                frame.render_widget(
                    GraphViewWidget {
                        nodes: &self.graph_nodes,
                        edges: &self.graph_edges,
                        selected: self.graph_cursor,
                        params: params.map(|p| p.as_slice()),
                        param_values: values.map(|v| v.as_slice()),
                        param_cursor: if self.mode == Mode::Edit {
                            Some(self.param_cursor)
                        } else {
                            None
                        },
                    },
                    middle[1],
                );
            }
        }

        frame.render_widget(
            ScopeView {
                samples: &self.scope_buf,
            },
            outer[2],
        );
    }

    /// Terminal main loop until quit: render, handle keys, poll notifications.
    pub fn run<B>(mut self, terminal: &mut ratatui::Terminal<B>) -> anyhow::Result<()>
    where
        B: ratatui::backend::Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Release {
                        if let Some(action) = input::handle_key(key, &self.mode) {
                            self.handle_action(action);
                        }
                    }
                }
            }

            self.poll_audio();

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }
}

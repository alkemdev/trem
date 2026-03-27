//! Main TUI application: grid, views, transport, and [`trem_rta::Bridge`] integration.
//!
//! [`App::run`] is the event loop (draw, input, non-blocking audio poll).

use crate::focus::{EscBehavior, FocusFrame, FocusStack};
use crate::input::{self, Action, BottomPane, Editor, InputContext, Mode};
use crate::project::{self, ProjectWorkspace};
use crate::view::context::ContextPanel;
use crate::view::fullscreen::FullscreenHud;
use crate::view::graph::GraphViewWidget;
use crate::view::help::HelpOverlay;
use crate::view::overview::OverviewView;
use crate::view::pattern::PatternView;
use crate::view::perf::HostStatsSnapshot;
use crate::view::scope::ScopeView;
use crate::view::spectrum::{SpectrumAnalyzerState, SpectrumView};
use crate::view::status::StatusBar;
use crate::view::transport::TransportView;

use num_rational::Rational64;
use trem::event::NoteEvent;
use trem::graph::{Edge, GraphSnapshot, ParamDescriptor};
use trem::math::Rational;
use trem::pitch::Pitch;
use trem::pitch::Tuning;
use trem_rta::{Bridge, Command, Notification, ScopeFocus};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Clear;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessesToUpdate, System};

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

fn fullscreen_overlay_rect(area: Rect) -> Rect {
    let max_width = area.width.saturating_sub(2).max(1);
    let width = ((area.width as u32 * 2) / 5) as u16;
    let width = width.clamp(24, 38).min(max_width);
    let height = area.height.saturating_sub(2).max(3);
    Rect::new(
        area.x + area.width.saturating_sub(width),
        area.y + 1.min(area.height.saturating_sub(1)),
        width,
        height,
    )
}

/// Mutable state for the full terminal UI: pattern/graph views, audio bridge, and layout data.
pub struct App {
    pub grid: trem::grid::Grid,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub mode: Mode,
    pub editor: Editor,
    /// Right sidebar HELP pane (`?` toggles, `i` returns to INFO).
    pub help_open: bool,
    /// Shared shell collapse: canvas-first view with a minimal exit HUD.
    pub fullscreen: bool,
    pub bpm: f64,
    pub playing: bool,
    /// After the first **Play**, pattern edits sync to the audio thread even while **paused**
    /// (playhead is held until **Play** again).
    engine_pattern_active: bool,
    pub beat_position: f64,
    pub current_play_row: Option<u32>,
    pub scale: trem::pitch::Scale,
    pub scale_name: String,
    pub octave: i32,
    pub bridge: Bridge,
    /// Master output (post–FX), interleaved stereo — waveform / spectrum.
    pub scope_master: Vec<f32>,
    /// Instrument submix (pre–master bus), same layout — graph view **IN** preview.
    pub scope_graph_in: Vec<f32>,
    /// This-process CPU / RSS refreshed ~2× per second for the info panel.
    pub host_stats: HostStatsSnapshot,
    sys: System,
    host_stats_last_refresh: Instant,
    /// Peak-decay time constant for spectrum bars (ms); lower = snappier, higher = longer “tail”.
    pub spectrum_fall_ms: f64,
    spectrum_analyzer_in: SpectrumAnalyzerState,
    spectrum_analyzer_out: SpectrumAnalyzerState,
    pub peak_l: f32,
    pub peak_r: f32,
    pub should_quit: bool,
    pub instrument_names: Vec<String>,
    pub voice_ids: Vec<u32>,
    pub graph_nodes: Vec<(u32, String)>,
    pub graph_node_descriptions: Vec<String>,
    pub graph_edges: Vec<Edge>,
    pub graph_cursor: usize,
    pub graph_depths: Vec<usize>,
    pub graph_layers: Vec<Vec<usize>>,
    pub graph_params: Vec<Vec<ParamDescriptor>>,
    pub graph_param_values: Vec<Vec<f64>>,
    pub graph_param_groups: Vec<Vec<trem::graph::ParamGroup>>,
    pub param_cursor: usize,
    pub swing: f64,
    pub euclidean_k: u32,
    pub undo_stack: Vec<Vec<Option<NoteEvent>>>,
    pub redo_stack: Vec<Vec<Option<NoteEvent>>>,
    rng_state: u64,
    preview_note_off: Option<(u32, Instant)>,
    pub bottom_pane: BottomPane,
    /// Path into nested graphs for the graph editor (empty = root).
    pub graph_path: Vec<u32>,
    /// Stack of saved cursor positions when diving into nested graphs.
    pub graph_stack: Vec<GraphFrame>,
    /// Tracks which nodes have inner graphs for visual indicators.
    pub graph_has_children: Vec<bool>,
    /// Breadcrumb labels for the navigation path.
    pub graph_breadcrumb: Vec<String>,
    /// Pregenerated inner-graph snapshots from the host (`Graph::nested_ui_snapshots`), keyed by
    /// the same path the UI uses after [`Self::enter_nested_graph`] (e.g. `[lead_id]`).
    nested_graph_snapshots: HashMap<Vec<u32>, GraphSnapshot>,
    /// MIDI piano roll from SEQ (**Enter** in navigate mode).
    pattern_roll: Option<crate::pattern_roll::PatternRoll>,
    /// Loaded `trem-project` workspace when running the rebuilt project shell.
    project_workspace: Option<ProjectWorkspace>,
    /// Current lane selection in the root scene overview.
    overview_lane: usize,
    /// Current block selection on the selected lane.
    overview_block: usize,
    /// Clip id being edited in the fullscreen roll.
    project_roll_clip_id: Option<String>,
}

/// Saved state when diving into a nested graph node.
#[derive(Clone, Debug)]
pub struct GraphFrame {
    pub nodes: Vec<(u32, String)>,
    pub edges: Vec<Edge>,
    pub cursor: usize,
    pub params: Vec<Vec<ParamDescriptor>>,
    pub param_values: Vec<Vec<f64>>,
    pub param_groups: Vec<Vec<trem::graph::ParamGroup>>,
    pub depths: Vec<usize>,
    pub layers: Vec<Vec<usize>>,
    pub has_children: Vec<bool>,
    pub node_descriptions: Vec<String>,
}

struct ShellContext {
    frames: Vec<FocusFrame>,
    focus_path: String,
    esc_hint: Option<String>,
    selection_summary: String,
    actions_summary: String,
    info_lines: Vec<String>,
    zone: String,
    mode: String,
    tool: String,
    project_mode: bool,
    project_name: Option<String>,
}

impl App {
    /// Initial pattern view, scale metadata, and per-column voice IDs for [`trem_rta::Command::NoteOn`].
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
            editor: Editor::Pattern,
            help_open: false,
            fullscreen: false,
            bpm: 120.0,
            playing: false,
            engine_pattern_active: false,
            beat_position: 0.0,
            current_play_row: None,
            scale,
            scale_name,
            octave: 0,
            bridge,
            scope_master: Vec::new(),
            scope_graph_in: Vec::new(),
            host_stats: HostStatsSnapshot::default(),
            sys: System::new(),
            host_stats_last_refresh: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
            spectrum_fall_ms: 18.0,
            spectrum_analyzer_in: SpectrumAnalyzerState::new(18.0),
            spectrum_analyzer_out: SpectrumAnalyzerState::new(18.0),
            peak_l: 0.0,
            peak_r: 0.0,
            should_quit: false,
            instrument_names,
            voice_ids,
            graph_nodes: Vec::new(),
            graph_node_descriptions: Vec::new(),
            graph_edges: Vec::new(),
            graph_cursor: 0,
            graph_depths: Vec::new(),
            graph_layers: Vec::new(),
            graph_params: Vec::new(),
            graph_param_values: Vec::new(),
            graph_param_groups: Vec::new(),
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
            bottom_pane: BottomPane::Hidden,
            graph_path: Vec::new(),
            graph_stack: Vec::new(),
            graph_has_children: Vec::new(),
            graph_breadcrumb: Vec::new(),
            nested_graph_snapshots: HashMap::new(),
            pattern_roll: None,
            project_workspace: None,
            overview_lane: 0,
            overview_block: 0,
            project_roll_clip_id: None,
        }
    }

    /// Boots the rebuilt TUI around a `trem-project` workspace instead of the old hardcoded demo.
    pub fn from_workspace(workspace: ProjectWorkspace, bridge: Bridge) -> Self {
        let rows = workspace.timeline_beats_u32().max(1);
        let cols = workspace.lane_count().max(1) as u32;
        let instrument_names = workspace
            .scene
            .lanes
            .iter()
            .map(|lane| lane.label.clone())
            .collect();
        let voice_ids = project::lane_voice_ids(&workspace.scene);
        let mut app = Self::new(
            trem::grid::Grid::new(rows, cols),
            Tuning::edo12().to_scale(),
            "12-EDO".into(),
            bridge,
            instrument_names,
            voice_ids,
        );
        app.load_workspace(workspace);
        app
    }

    fn project_mode(&self) -> bool {
        self.project_workspace.is_some()
    }

    fn project_name(&self) -> Option<&str> {
        self.project_workspace
            .as_ref()
            .map(ProjectWorkspace::project_name)
    }

    fn load_workspace(&mut self, workspace: ProjectWorkspace) {
        let rows = workspace.timeline_beats_u32().max(1);
        let cols = workspace.lane_count().max(1) as u32;
        self.grid = trem::grid::Grid::new(rows, cols);
        self.instrument_names = workspace
            .scene
            .lanes
            .iter()
            .map(|lane| lane.label.clone())
            .collect();
        self.voice_ids = project::lane_voice_ids(&workspace.scene);
        self.bpm = workspace.tempo_bpm();
        self.playing = false;
        self.engine_pattern_active = false;
        self.beat_position = 0.0;
        self.current_play_row = None;
        self.mode = Mode::Normal;
        self.editor = Editor::Pattern;
        self.pattern_roll = None;
        self.project_roll_clip_id = None;
        self.bottom_pane = BottomPane::Hidden;
        self.project_workspace = Some(workspace);
        self.overview_lane = 0;
        self.overview_block = 0;
        self.graph_path.clear();
        self.graph_stack.clear();
        self.graph_breadcrumb.clear();
        self.normalize_overview_selection();
        self.load_project_graph_selection();
        self.sync_project_scene();
    }

    fn sync_project_scene(&mut self) {
        let Some(workspace) = self.project_workspace.as_ref() else {
            return;
        };
        let scene = project::Scene::from_workspace(workspace);
        self.bpm = scene.bpm;
        self.bridge.send(Command::SetBpm(scene.bpm));
        self.bridge.send(Command::LoadEvents {
            events: scene.events,
            loop_len: scene.loop_len,
        });
    }

    fn normalize_overview_selection(&mut self) {
        let Some(workspace) = self.project_workspace.as_ref() else {
            self.overview_lane = 0;
            self.overview_block = 0;
            return;
        };
        if workspace.scene.lanes.is_empty() {
            self.overview_lane = 0;
            self.overview_block = 0;
            return;
        }
        self.overview_lane = self
            .overview_lane
            .min(workspace.scene.lanes.len().saturating_sub(1));
        let block_count = workspace
            .scene
            .lanes
            .get(self.overview_lane)
            .map(|lane| lane.blocks.len())
            .unwrap_or(0);
        self.overview_block = if block_count == 0 {
            0
        } else {
            self.overview_block.min(block_count.saturating_sub(1))
        };
    }

    fn move_overview_vertical(&mut self, delta: i32) {
        let Some(workspace) = self.project_workspace.as_ref() else {
            return;
        };
        let lane_count = workspace.scene.lanes.len() as i32;
        if lane_count == 0 {
            return;
        }
        self.overview_lane = (self.overview_lane as i32 + delta).clamp(0, lane_count - 1) as usize;
        self.normalize_overview_selection();
    }

    fn move_overview_horizontal(&mut self, delta: i32) {
        let Some(workspace) = self.project_workspace.as_ref() else {
            return;
        };
        let Some(lane) = workspace.scene.lanes.get(self.overview_lane) else {
            return;
        };
        let block_count = lane.blocks.len() as i32;
        if block_count == 0 {
            self.overview_block = 0;
            return;
        }
        self.overview_block =
            (self.overview_block as i32 + delta).clamp(0, block_count - 1) as usize;
    }

    fn open_project_clip_roll(&mut self) {
        let Some(workspace) = self.project_workspace.as_ref() else {
            return;
        };
        let Some(block) = workspace.block(self.overview_lane, self.overview_block) else {
            return;
        };
        let Some(clip) = workspace
            .clip_for_selection(self.overview_lane, self.overview_block)
            .cloned()
        else {
            return;
        };
        let Some(preview_ctx) =
            project::clip_preview_context(workspace, self.overview_lane, self.overview_block)
        else {
            return;
        };
        let lane_voice = self
            .voice_ids
            .get(self.overview_lane)
            .copied()
            .unwrap_or(self.overview_lane as u32);
        let roll_clip = project::clip_document_to_roll_clip(&clip, lane_voice);
        let loop_beats_floor = project::parse_beat_expr(&block.length)
            .or_else(|| roll_clip.length_beats.map(|beats| beats.rational()))
            .unwrap_or_else(|| Rational64::from_integer(self.grid.rows as i64));
        let mut roll = crate::pattern_roll::PatternRoll::new(
            roll_clip,
            self.overview_lane as u32,
            loop_beats_floor,
            lane_voice,
            crate::pattern_roll::PatternRollPreview::ProjectClip {
                background_events: preview_ctx.background_events,
                block_start: preview_ctx.block_start,
                loop_beats: preview_ctx.loop_beats,
            },
            self.scale.clone(),
            self.voice_ids.clone(),
            440.0,
            self.swing,
        );
        roll.push_preview(&mut self.bridge, self.bpm, 44100.0);
        self.project_roll_clip_id = Some(clip.clip.id.clone());
        self.pattern_roll = Some(roll);
    }

    fn load_project_graph_selection(&mut self) {
        let Some(workspace) = self.project_workspace.as_ref() else {
            return;
        };
        let Some(graph) =
            workspace.graph_view_for_selection(self.overview_lane, self.overview_block)
        else {
            self.graph_nodes.clear();
            self.graph_edges.clear();
            self.graph_depths.clear();
            self.graph_layers.clear();
            self.graph_params.clear();
            self.graph_param_values.clear();
            self.graph_param_groups.clear();
            self.graph_has_children.clear();
            self.graph_node_descriptions.clear();
            self.graph_breadcrumb.clear();
            self.graph_cursor = 0;
            self.param_cursor = 0;
            return;
        };
        let (depths, layers) = crate::view::graph::compute_graph_nav(&graph.nodes, &graph.edges);
        self.graph_nodes = graph.nodes;
        self.graph_edges = graph.edges;
        self.graph_depths = depths;
        self.graph_layers = layers;
        self.graph_params = vec![Vec::new(); self.graph_nodes.len()];
        self.graph_param_values = vec![Vec::new(); self.graph_nodes.len()];
        self.graph_param_groups = vec![Vec::new(); self.graph_nodes.len()];
        self.graph_has_children = vec![false; self.graph_nodes.len()];
        self.graph_node_descriptions = vec![String::new(); self.graph_nodes.len()];
        self.graph_breadcrumb = vec![graph.name];
        self.graph_cursor = 0;
        self.param_cursor = 0;
    }

    fn graph_can_enter_nested(&self) -> bool {
        matches!(self.editor, Editor::Graph)
            && self
                .graph_has_children
                .get(self.graph_cursor)
                .copied()
                .unwrap_or(false)
    }

    fn focus_stack(&self) -> FocusStack {
        let mut stack = FocusStack::new();
        stack.push(FocusFrame::project(
            self.project_name().unwrap_or("Project").to_string(),
        ));
        if self.project_mode() {
            if let Some(workspace) = self.project_workspace.as_ref() {
                stack.push(FocusFrame::surface(
                    "scene",
                    format!("Scene {}", workspace.scene.scene.name),
                ));
                if let Some(lane) = workspace.lane(self.overview_lane) {
                    stack.push(FocusFrame::surface("lane", format!("Lane {}", lane.label)));
                    if let Some(block) = workspace.block(self.overview_lane, self.overview_block) {
                        stack.push(FocusFrame::surface(
                            "block",
                            format!("Block {}", block.name),
                        ));
                    }
                }
            }
            stack.push(FocusFrame::surface(
                "editor",
                match self.editor {
                    Editor::Pattern => "Overview",
                    Editor::Graph => "Graph",
                },
            ));
        } else {
            stack.push(FocusFrame::editor(self.editor));
        }

        if self.editor == Editor::Graph && !self.project_mode() {
            for label in &self.graph_breadcrumb {
                stack.push(FocusFrame::surface("inner-graph", label.clone()));
            }
        } else if self.editor == Editor::Graph && self.project_mode() {
            if let Some(current) = self.graph_breadcrumb.last() {
                stack.push(FocusFrame::surface("graph-doc", current.clone()));
            }
        }

        if self.mode == Mode::Edit {
            let label = match self.editor {
                Editor::Pattern if self.project_mode() => "Overview",
                Editor::Pattern => "Step Edit",
                Editor::Graph => "Param Edit",
            };
            stack.push(FocusFrame::surface("mode", label));
        }

        if let Some(roll) = &self.pattern_roll {
            let label = if self.project_mode() {
                self.project_roll_clip_id
                    .as_deref()
                    .map(|id| format!("ROL {}", id))
                    .unwrap_or_else(|| format!("ROL {}", roll.grid_column))
            } else {
                format!("ROL {}", roll.grid_column)
            };
            stack.push(FocusFrame::surface("roll", label));
        }

        let esc_behavior = if self.pattern_roll.is_some() {
            EscBehavior::ApplyAndBack
        } else if self.mode == Mode::Edit
            || (self.editor == Editor::Graph && !self.graph_path.is_empty())
        {
            EscBehavior::Back
        } else {
            EscBehavior::None
        };
        stack.with_esc_behavior(esc_behavior)
    }

    fn shell_selection_summary(&self) -> String {
        if let Some(roll) = &self.pattern_roll {
            let project_context = if self.project_mode() {
                self.project_workspace
                    .as_ref()
                    .and_then(|workspace| {
                        let lane = workspace.lane(self.overview_lane)?;
                        let block = workspace.block(self.overview_lane, self.overview_block)?;
                        let clip = self.project_roll_clip_id.as_deref().unwrap_or("clip");
                        Some(format!(
                            "lane {} · block {} · clip {}",
                            lane.label, block.name, clip
                        ))
                    })
                    .unwrap_or_else(|| format!("track {}", roll.grid_column))
            } else {
                format!("track {}", roll.grid_column)
            };
            return match (roll.primary_index(), roll.primary_note()) {
                (Some(primary_idx), Some(note)) => {
                    let dur = note.t_off.rational() - note.t_on.rational();
                    format!(
                        "{} · {} sel · primary #{} class {} Δ {}/{} vel {:.2}",
                        project_context,
                        roll.selection_len(),
                        primary_idx,
                        note.class,
                        dur.numer(),
                        dur.denom(),
                        note.velocity
                    )
                }
                _ => format!("{project_context} · empty clip"),
            };
        }

        if self.project_mode() {
            return match self.editor {
                Editor::Pattern => {
                    let Some(workspace) = self.project_workspace.as_ref() else {
                        return "no project loaded".into();
                    };
                    let Some(lane) = workspace.lane(self.overview_lane) else {
                        return "no lane selected".into();
                    };
                    match workspace.block(self.overview_lane, self.overview_block) {
                        Some(block) => {
                            let kind = match &block.content {
                                trem_project::BlockContent::Clip { .. } => "clip",
                                trem_project::BlockContent::Graph { .. } => "graph",
                                trem_project::BlockContent::Sample { .. } => "sample",
                                trem_project::BlockContent::Midi { .. } => "midi",
                                trem_project::BlockContent::Marker { .. } => "marker",
                            };
                            format!("lane {} · {} · {}", lane.label, block.name, kind)
                        }
                        None => format!("lane {} · empty", lane.label),
                    }
                }
                Editor::Graph => self
                    .graph_nodes
                    .get(self.graph_cursor)
                    .map(|(_, name)| format!("node {name}"))
                    .unwrap_or_else(|| "graph empty".into()),
            };
        }

        match self.editor {
            Editor::Pattern => match self.grid.get(self.cursor_row, self.cursor_col) {
                Some(note) => format!(
                    "step {} voice {} note {}{} vel {:.2}",
                    self.cursor_row,
                    self.cursor_col,
                    note.degree,
                    if note.octave == 0 {
                        String::new()
                    } else {
                        format!(" oct {}", note.octave)
                    },
                    note.velocity.to_f64()
                ),
                None => format!("step {} voice {} empty", self.cursor_row, self.cursor_col),
            },
            Editor::Graph => {
                let node = self
                    .graph_nodes
                    .get(self.graph_cursor)
                    .map(|(_, name)| name.as_str())
                    .unwrap_or("root");
                if self.mode == Mode::Edit {
                    if let (Some(params), Some(values)) = (
                        self.graph_params.get(self.graph_cursor),
                        self.graph_param_values.get(self.graph_cursor),
                    ) {
                        if let (Some(param), Some(value)) =
                            (params.get(self.param_cursor), values.get(self.param_cursor))
                        {
                            return format!(
                                "node {} · {} {}",
                                node,
                                param.name,
                                crate::view::graph::format_param_value(*value, param)
                            );
                        }
                    }
                }
                format!("node {node}")
            }
        }
    }

    fn shell_actions_summary(&self) -> String {
        if let Some(roll) = &self.pattern_roll {
            return format!(
                "ROL {} · Tab mode · hjkl by mode · Shift extend/coarse · Ctrl+←/→ snap · Shift+Enter full · i info · ? help",
                roll.mode_label()
            );
        }

        if self.project_mode() {
            return match self.editor {
                Editor::Pattern => {
                    "Space play · Enter open · Shift+Enter full · Tab graph · hjkl move · Ctrl+S save · Ctrl+O reload · i info · ? help".into()
                }
                Editor::Graph => {
                    "Space play · Shift+Enter full · hjkl move · Tab overview · Ctrl+S save · Ctrl+O reload · i info · ? help".into()
                }
            };
        }

        match (self.editor, self.mode) {
            (Editor::Pattern, Mode::Normal) => {
                "Enter roll · Shift+Enter full · e edit · Tab switch · ` panel · i info · ? help"
                    .into()
            }
            (Editor::Pattern, Mode::Edit) => {
                "z-m notes · a gate · Shift+Enter full · Del clear · ` panel · i info · ? help"
                    .into()
            }
            (Editor::Graph, Mode::Normal) => {
                let mut parts = vec!["h/j/k/l move".to_string(), "e params".to_string()];
                if self.graph_can_enter_nested() {
                    parts.push("Enter focus".to_string());
                }
                parts.push("Shift+Enter full".to_string());
                parts.push("` panel".to_string());
                parts.push("i info".to_string());
                parts.push("? help".to_string());
                parts.join(" · ")
            }
            (Editor::Graph, Mode::Edit) => {
                "←/→ adjust · Shift+←/→ fine · Shift+Enter full · ` panel · i info · ? help".into()
            }
        }
    }

    fn current_zone_mode_tool(&self) -> (String, String, String) {
        if let Some(roll) = &self.pattern_roll {
            return (
                "ROL".into(),
                roll.mode_label().into(),
                roll.tool_label().into(),
            );
        }

        if self.project_mode() {
            return match self.editor {
                Editor::Pattern => ("PRJ".into(), "VIEW".into(), "block-focus".into()),
                Editor::Graph => (
                    "GRF".into(),
                    if self.mode == Mode::Edit {
                        "PARAM".into()
                    } else {
                        "VIEW".into()
                    },
                    if self.mode == Mode::Edit {
                        "param".into()
                    } else {
                        "node-focus".into()
                    },
                ),
            };
        }

        match self.editor {
            Editor::Pattern => (
                "SEQ".into(),
                self.mode.label().into(),
                if self.mode == Mode::Edit {
                    "note-paint".into()
                } else {
                    "step-focus".into()
                },
            ),
            Editor::Graph => (
                "GRF".into(),
                self.mode.label().into(),
                if self.mode == Mode::Edit {
                    "param".into()
                } else {
                    "node-focus".into()
                },
            ),
        }
    }

    fn build_shell_context(&self, focus_stack: &FocusStack) -> ShellContext {
        let (zone, mode, tool) = self.current_zone_mode_tool();
        ShellContext {
            frames: focus_stack.frames().to_vec(),
            focus_path: focus_stack.breadcrumb(),
            esc_hint: focus_stack.esc_hint(),
            selection_summary: self.shell_selection_summary(),
            actions_summary: self.shell_actions_summary(),
            info_lines: self.sidebar_info_lines(),
            zone,
            mode,
            tool,
            project_mode: self.project_mode(),
            project_name: self.project_name().map(str::to_string),
        }
    }

    fn pattern_roll_loop_beats(&self) -> Option<f64> {
        let roll = self.pattern_roll.as_ref()?;
        Some(
            roll.clip
                .length_beats
                .map(|beats| *beats.rational().numer() as f64 / *beats.rational().denom() as f64)
                .or_else(|| {
                    self.project_workspace.as_ref().and_then(|workspace| {
                        project::parse_beat_expr(&workspace.scene.scene.timeline_beats)
                            .map(|beats| *beats.numer() as f64 / *beats.denom() as f64)
                    })
                })
                .unwrap_or(self.grid.rows as f64),
        )
    }

    fn bottom_panel_height(&self) -> u16 {
        if self.fullscreen || self.project_mode() {
            return 0;
        }
        match (self.editor, self.bottom_pane) {
            (_, BottomPane::Hidden) => 0,
            (Editor::Graph, _) => 6,
            (Editor::Pattern, _) => 5,
        }
    }

    fn sidebar_info_lines(&self) -> Vec<String> {
        if let Some(roll) = &self.pattern_roll {
            let mut lines = vec![
                format!("rol {} · {}", roll.mode_label(), roll.mode_intent()),
                format!("attr {}", roll.attr_label()),
                format!("selection {} note(s)", roll.selection_len()),
            ];
            if let Some(note) = roll.primary_note() {
                let dur = note.t_off.rational() - note.t_on.rational();
                lines.push(format!(
                    "primary {} @ {} + {}/{}",
                    note.class,
                    note.t_on,
                    dur.numer(),
                    dur.denom()
                ));
                lines.push(format!("voice {} · vel {:.2}", note.voice, note.velocity));
            }
            return lines;
        }

        if self.project_mode() {
            let mut lines = vec!["pane info".to_string()];
            if let Some(workspace) = self.project_workspace.as_ref() {
                lines.push(format!("project {}", workspace.project_name()));
                lines.push(format!("scene {}", workspace.scene.scene.name));
            }
            return lines;
        }

        vec![
            format!("editor {}", self.editor.title()),
            format!("mode {}", self.mode.label()),
            format!("scale {}", self.scale_name),
        ]
    }

    fn close_pattern_roll_apply(&mut self) {
        let Some(roll) = self.pattern_roll.take() else {
            return;
        };
        if let Err(e) = roll.validate_for_apply() {
            eprintln!("trem: pattern roll invalid ({e}); fix note times before closing.");
            self.pattern_roll = Some(roll);
            return;
        }
        if let (Some(workspace), Some(clip_id)) = (
            self.project_workspace.as_mut(),
            self.project_roll_clip_id.take(),
        ) {
            if let Some(template) = workspace.clips.get(&clip_id).cloned() {
                let updated = project::roll_clip_to_document(&template, &roll.clip);
                workspace.replace_clip(updated);
            }
            self.sync_project_scene();
            return;
        }
        let col = roll.grid_column;
        self.push_undo();
        crate::pattern_roll::apply_clip_to_grid_column(
            &roll.clip,
            &mut self.grid,
            &self.scale,
            440.0,
            &self.voice_ids,
            col,
        );
        drop(roll);
        self.send_pattern();
    }

    /// Attaches node/edge/param snapshots for the graph editor (from the host graph).
    pub fn with_graph_info(
        mut self,
        nodes: Vec<(u32, String)>,
        edges: Vec<Edge>,
        params: Vec<(Vec<ParamDescriptor>, Vec<f64>, Vec<trem::graph::ParamGroup>)>,
    ) -> Self {
        let (depths, layers) = crate::view::graph::compute_graph_nav(&nodes, &edges);
        self.graph_nodes = nodes;
        self.graph_edges = edges;
        self.graph_depths = depths;
        self.graph_layers = layers;
        self.graph_params = params.iter().map(|(d, _, _)| d.clone()).collect();
        self.graph_param_values = params.iter().map(|(_, v, _)| v.clone()).collect();
        self.graph_param_groups = params.into_iter().map(|(_, _, g)| g).collect();
        self.graph_has_children = vec![false; self.graph_nodes.len()];
        self
    }

    /// Sets one-line descriptions per graph node (shown in info help).
    pub fn set_node_descriptions(&mut self, descriptions: Vec<String>) {
        self.graph_node_descriptions = descriptions;
    }

    /// Marks which nodes have inner (nested) graphs for the graph view indicator.
    pub fn set_node_children(&mut self, has_children: Vec<bool>) {
        self.graph_has_children = has_children;
    }

    /// Supplies snapshots for nested graph levels so **Graph › Enter** shows nodes and parameters.
    pub fn with_nested_graph_snapshots(
        mut self,
        snapshots: HashMap<Vec<u32>, GraphSnapshot>,
    ) -> Self {
        self.nested_graph_snapshots = snapshots;
        self
    }

    fn refresh_host_stats(&mut self) {
        if self.host_stats_last_refresh.elapsed() < Duration::from_millis(520) {
            return;
        }
        self.host_stats_last_refresh = Instant::now();
        self.sys.refresh_cpu_usage();
        let pid = Pid::from_u32(std::process::id());
        self.sys
            .refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
        if let Some(p) = self.sys.process(pid) {
            // Smoothed: raw `cpu_usage` is per refresh window and can spike (UI redraw + audio).
            let raw = p.cpu_usage();
            let prev = self.host_stats.process_cpu_pct;
            const SMOOTH: f32 = 0.22;
            self.host_stats.process_cpu_pct = if prev <= f32::EPSILON {
                raw
            } else {
                prev * (1.0 - SMOOTH) + raw * SMOOTH
            };
            self.host_stats.process_rss_mb = p.memory() / 1024 / 1024;
        } else {
            self.host_stats.process_cpu_pct = 0.0;
            self.host_stats.process_rss_mb = 0;
        }
    }

    /// Rebuilds graph editor state from a host [`GraphSnapshot`] (nodes, edges, params, layout).
    fn load_graph_from_snapshot(&mut self, snap: &GraphSnapshot) {
        let nodes: Vec<(u32, String)> = snap.nodes.iter().map(|n| (n.id, n.name.clone())).collect();
        let edges = snap.edges.clone();
        let (depths, layers) = crate::view::graph::compute_graph_nav(&nodes, &edges);
        self.graph_nodes = nodes;
        self.graph_edges = edges;
        self.graph_depths = depths;
        self.graph_layers = layers;
        self.graph_params = snap.nodes.iter().map(|n| n.params.clone()).collect();
        self.graph_param_values = snap.nodes.iter().map(|n| n.param_values.clone()).collect();
        self.graph_param_groups = snap.nodes.iter().map(|n| n.param_groups.clone()).collect();
        self.graph_has_children = snap.nodes.iter().map(|n| n.has_children).collect();
        self.graph_node_descriptions = vec![String::new(); self.graph_nodes.len()];
        self.graph_cursor = 0;
        self.param_cursor = 0;
    }

    /// Tells the audio thread which signal to show in the bottom **IN | OUT** previews.
    pub fn sync_scope_focus(&mut self) {
        if self.project_mode() {
            return;
        }
        match self.editor {
            Editor::Pattern => {
                self.bridge
                    .send(Command::SetScopeFocus(ScopeFocus::PatchBuses));
            }
            Editor::Graph => {
                if let Some(&(nid, _)) = self.graph_nodes.get(self.graph_cursor) {
                    self.bridge
                        .send(Command::SetScopeFocus(ScopeFocus::GraphNode {
                            graph_path: self.graph_path.clone(),
                            node: nid,
                        }));
                } else {
                    self.bridge
                        .send(Command::SetScopeFocus(ScopeFocus::PatchBuses));
                }
            }
        }
    }

    /// Applies one [`Action`] from input: updates state and sends [`Command`]s to the audio bridge as needed.
    pub fn handle_action(&mut self, action: Action) {
        let sync_scope = matches!(
            &action,
            Action::CycleEditor
                | Action::EnterGraph
                | Action::ExitGraph
                | Action::MoveUp
                | Action::MoveDown
                | Action::MoveLeft
                | Action::MoveRight
                | Action::LoadProject
        );
        match action {
            Action::Quit => self.should_quit = true,
            Action::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                if !self.fullscreen {
                    self.help_open = false;
                }
            }
            Action::ToggleHelp => {
                self.help_open = !self.help_open;
            }
            Action::ShowInfoPane => self.help_open = false,
            Action::CycleEditor => {
                self.editor = self.editor.next();
                self.mode = Mode::Normal;
                if self.project_mode() && self.editor == Editor::Graph {
                    self.load_project_graph_selection();
                }
            }
            Action::ToggleEdit => {
                if self.project_mode() {
                    return;
                }
                self.mode = match self.mode {
                    Mode::Normal => {
                        self.param_cursor = 0;
                        Mode::Edit
                    }
                    Mode::Edit => Mode::Normal,
                };
            }
            Action::OpenPatternRoll => {
                if self.project_mode() {
                    if self.editor != Editor::Pattern || self.pattern_roll.is_some() {
                        return;
                    }
                    let is_graph_block = self
                        .project_workspace
                        .as_ref()
                        .and_then(|workspace| {
                            workspace.block(self.overview_lane, self.overview_block)
                        })
                        .is_some_and(|block| {
                            matches!(block.content, trem_project::BlockContent::Graph { .. })
                        });
                    if is_graph_block {
                        self.editor = Editor::Graph;
                        self.load_project_graph_selection();
                    } else {
                        self.open_project_clip_roll();
                    }
                    return;
                }
                if self.editor != Editor::Pattern
                    || self.mode != Mode::Normal
                    || self.pattern_roll.is_some()
                {
                    return;
                }
                let col = self.cursor_col.min(self.grid.columns.saturating_sub(1));
                let lane_voice = self.voice_ids.get(col as usize).copied().unwrap_or(col);
                let clip = crate::pattern_roll::clip_from_grid_column(
                    &self.grid,
                    &self.scale,
                    440.0,
                    self.voice_ids.as_slice(),
                    col,
                );
                let mut roll = crate::pattern_roll::PatternRoll::new(
                    clip,
                    col,
                    Rational64::from_integer(self.grid.rows as i64),
                    lane_voice,
                    crate::pattern_roll::PatternRollPreview::Grid(self.grid.clone()),
                    self.scale.clone(),
                    self.voice_ids.clone(),
                    440.0,
                    self.swing,
                );
                roll.push_preview(&mut self.bridge, self.bpm, 44100.0);
                self.pattern_roll = Some(roll);
            }
            Action::TogglePlay => {
                if self.project_mode() {
                    self.playing = !self.playing;
                    if self.playing {
                        self.engine_pattern_active = true;
                        self.bridge.send(Command::Play);
                    } else {
                        self.bridge.send(Command::Pause);
                    }
                    return;
                }
                self.playing = !self.playing;
                if self.playing {
                    self.engine_pattern_active = true;
                    self.send_pattern();
                    self.bridge.send(Command::Play);
                } else {
                    self.bridge.send(Command::Pause);
                }
            }
            Action::MoveUp => match (&self.editor, &self.mode) {
                (Editor::Pattern, _) => {
                    if self.project_mode() {
                        self.move_overview_vertical(-1);
                    } else {
                        self.cursor_col = self.cursor_col.saturating_sub(1);
                    }
                }
                (Editor::Graph, Mode::Normal) => self.graph_move_up(),
                (Editor::Graph, Mode::Edit) => {
                    self.param_cursor = self.param_cursor.saturating_sub(1);
                }
            },
            Action::MoveDown => match (&self.editor, &self.mode) {
                (Editor::Pattern, _) => {
                    if self.project_mode() {
                        self.move_overview_vertical(1);
                    } else if self.cursor_col < self.grid.columns.saturating_sub(1) {
                        self.cursor_col += 1;
                    }
                }
                (Editor::Graph, Mode::Normal) => self.graph_move_down(),
                (Editor::Graph, Mode::Edit) => {
                    let max = self.current_node_param_count().saturating_sub(1);
                    if self.param_cursor < max {
                        self.param_cursor += 1;
                    }
                }
            },
            Action::MoveLeft => match (&self.editor, &self.mode) {
                (Editor::Pattern, _) => {
                    if self.project_mode() {
                        self.move_overview_horizontal(-1);
                    } else {
                        self.cursor_row = self.cursor_row.saturating_sub(1);
                    }
                }
                (Editor::Graph, Mode::Normal) => self.graph_move_left(),
                (Editor::Graph, Mode::Edit) => self.adjust_param_coarse(-1),
            },
            Action::MoveRight => match (&self.editor, &self.mode) {
                (Editor::Pattern, _) => {
                    if self.project_mode() {
                        self.move_overview_horizontal(1);
                    } else if self.cursor_row < self.grid.rows.saturating_sub(1) {
                        self.cursor_row += 1;
                    }
                }
                (Editor::Graph, Mode::Normal) => self.graph_move_right(),
                (Editor::Graph, Mode::Edit) => self.adjust_param_coarse(1),
            },
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::SaveProject => {
                if let Some(workspace) = self.project_workspace.as_ref() {
                    if let Err(e) = workspace.save() {
                        eprintln!("save error: {e}");
                    }
                } else {
                    let path = crate::project::default_project_path();
                    eprintln!("save not available for legacy shell ({})", path.display());
                }
            }
            Action::LoadProject => {
                if let Some(workspace) = self.project_workspace.as_ref() {
                    match workspace.reload() {
                        Ok(workspace) => self.load_workspace(workspace),
                        Err(e) => eprintln!("load error: {e}"),
                    }
                } else {
                    let path = crate::project::default_project_path();
                    eprintln!("load not available for legacy shell ({})", path.display());
                }
            }
            Action::SwingUp => {
                if self.project_mode() {
                    return;
                }
                self.swing = (self.swing + 0.05).min(0.9);
                if self.should_sync_pattern() {
                    self.send_pattern();
                }
            }
            Action::SwingDown => {
                if self.project_mode() {
                    return;
                }
                self.swing = (self.swing - 0.05).max(0.0);
                if self.should_sync_pattern() {
                    self.send_pattern();
                }
            }
            Action::GateCycle => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    if let Some(note) = self.grid.get(self.cursor_row, self.cursor_col).cloned() {
                        self.push_undo();
                        let new_gate = cycle_gate(note.gate);
                        let mut updated = note;
                        updated.gate = new_gate;
                        self.grid
                            .set(self.cursor_row, self.cursor_col, Some(updated));
                        if self.should_sync_pattern() {
                            self.send_pattern();
                        }
                    }
                }
            }
            Action::NoteInput(degree) => {
                if self.project_mode() {
                    return;
                }
                if self.editor != Editor::Pattern {
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

                if self.should_sync_pattern() {
                    self.send_pattern();
                }

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
                if self.project_mode() {
                    return;
                }
                if self.editor != Editor::Pattern {
                    return;
                }
                self.push_undo();
                self.grid.set(self.cursor_row, self.cursor_col, None);
                if self.should_sync_pattern() {
                    self.send_pattern();
                }
            }
            Action::OctaveUp => {
                if self.project_mode() {
                    return;
                }
                self.octave = (self.octave + 1).min(9);
                if self.should_sync_pattern() {
                    self.send_pattern();
                }
            }
            Action::OctaveDown => {
                if self.project_mode() {
                    return;
                }
                self.octave = (self.octave - 1).max(-4);
                if self.should_sync_pattern() {
                    self.send_pattern();
                }
            }
            Action::BpmUp => {
                if self.project_mode() {
                    if let Some(workspace) = self.project_workspace.as_mut() {
                        let tempo = (self.bpm + 1.0).min(300.0).round() as u16;
                        workspace.set_tempo_bpm(tempo);
                    }
                    self.sync_project_scene();
                    return;
                }
                if self.editor == Editor::Graph && self.mode == Mode::Edit {
                    self.adjust_param_fine(1);
                } else {
                    self.bpm = (self.bpm + 1.0).min(300.0);
                    self.bridge.send(Command::SetBpm(self.bpm));
                }
            }
            Action::BpmDown => {
                if self.project_mode() {
                    if let Some(workspace) = self.project_workspace.as_mut() {
                        let tempo = (self.bpm - 1.0).max(20.0).round() as u16;
                        workspace.set_tempo_bpm(tempo);
                    }
                    self.sync_project_scene();
                    return;
                }
                if self.editor == Editor::Graph && self.mode == Mode::Edit {
                    self.adjust_param_fine(-1);
                } else {
                    self.bpm = (self.bpm - 1.0).max(20.0);
                    self.bridge.send(Command::SetBpm(self.bpm));
                }
            }
            Action::ParamFineUp => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Graph && self.mode == Mode::Edit {
                    self.adjust_param_fine(1);
                }
            }
            Action::ParamFineDown => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Graph && self.mode == Mode::Edit {
                    self.adjust_param_fine(-1);
                }
            }
            Action::EuclideanFill => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    self.push_undo();
                    self.euclidean_k = (self.euclidean_k + 1) % (self.grid.rows + 1);
                    let pattern = trem::euclidean::euclidean(self.euclidean_k, self.grid.rows);
                    let template = NoteEvent::new(0, self.octave, Rational::new(3, 4));
                    self.grid
                        .fill_euclidean(self.cursor_col, &pattern, template);
                    if self.should_sync_pattern() {
                        self.send_pattern();
                    }
                }
            }
            Action::RandomizeVoice => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    self.push_undo();
                    self.randomize_current_voice();
                    if self.should_sync_pattern() {
                        self.send_pattern();
                    }
                }
            }
            Action::ReverseVoice => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    self.push_undo();
                    self.grid.reverse_voice(self.cursor_col);
                    if self.should_sync_pattern() {
                        self.send_pattern();
                    }
                }
            }
            Action::ShiftVoiceLeft => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    self.push_undo();
                    self.grid.shift_voice(self.cursor_col, -1);
                    if self.should_sync_pattern() {
                        self.send_pattern();
                    }
                }
            }
            Action::ShiftVoiceRight => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    self.push_undo();
                    self.grid.shift_voice(self.cursor_col, 1);
                    if self.should_sync_pattern() {
                        self.send_pattern();
                    }
                }
            }
            Action::VelocityUp => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    self.push_undo();
                    self.adjust_note_velocity(Rational::new(1, 8));
                    if self.should_sync_pattern() {
                        self.send_pattern();
                    }
                }
            }
            Action::VelocityDown => {
                if self.project_mode() {
                    return;
                }
                if self.editor == Editor::Pattern {
                    self.push_undo();
                    self.adjust_note_velocity(Rational::new(-1, 8));
                    if self.should_sync_pattern() {
                        self.send_pattern();
                    }
                }
            }
            Action::CycleBottomPane => {
                if self.project_mode() {
                    return;
                }
                self.bottom_pane = self.bottom_pane.next();
            }
            Action::EnterGraph => {
                if self.project_mode() {
                    return;
                }
                if self.editor != Editor::Graph || self.mode != Mode::Normal {
                    return;
                }
                if self.graph_cursor >= self.graph_has_children.len() {
                    return;
                }
                if !self.graph_has_children[self.graph_cursor] {
                    return;
                }
                self.enter_nested_graph();
            }
            Action::ExitGraph => {
                if self.project_mode() {
                    return;
                }
                if self.editor != Editor::Graph {
                    return;
                }
                self.exit_nested_graph();
            }
        }
        if sync_scope {
            self.sync_scope_focus();
        }
    }

    fn current_node_param_count(&self) -> usize {
        self.graph_params
            .get(self.graph_cursor)
            .map_or(0, |p| p.len())
    }

    fn adjust_param_coarse(&mut self, direction: i32) {
        self.adjust_param_by(direction, false);
    }

    fn adjust_param_fine(&mut self, direction: i32) {
        self.adjust_param_by(direction, true);
    }

    fn adjust_param_by(&mut self, direction: i32, fine: bool) {
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

        let base_step = if desc.step > 0.0 {
            desc.step
        } else {
            (desc.max - desc.min) * 0.01
        };
        let step = if fine { base_step * 0.1 } else { base_step };

        let old = values[self.param_cursor];
        let new_val = (old + step * direction as f64).clamp(desc.min, desc.max);
        values[self.param_cursor] = new_val;

        let node_id = self.graph_nodes[self.graph_cursor].0;
        let mut path = self.graph_path.clone();
        path.push(node_id);
        self.bridge.send(Command::SetParam {
            path,
            param_id: desc.id,
            value: new_val,
        });

        if !self.playing {
            self.fire_param_preview();
        }
    }

    /// Sends a short preview note so the user hears parameter changes even
    /// when the transport is stopped. The note flows through the full graph
    /// chain (synths → bus → FX → master), making all node tweaks audible.
    fn fire_param_preview(&mut self) {
        let voice = self.voice_ids.first().copied().unwrap_or(0);
        if let Some((old, _)) = self.preview_note_off.take() {
            self.bridge.send(Command::NoteOff { voice: old });
        }
        self.bridge.send(Command::NoteOn {
            frequency: 440.0,
            velocity: 0.6,
            voice,
        });
        self.preview_note_off = Some((voice, Instant::now()));
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
        for e in &self.graph_edges {
            if e.src_node == current_id && seen.insert(e.dst_node) {
                if let Some(idx) = self
                    .graph_nodes
                    .iter()
                    .position(|(id, _)| *id == e.dst_node)
                {
                    self.graph_cursor = idx;
                    return;
                }
            }
        }
    }

    fn graph_move_left(&mut self) {
        let current_id = self.graph_nodes[self.graph_cursor].0;
        let mut seen = HashSet::new();
        for e in &self.graph_edges {
            if e.dst_node == current_id && seen.insert(e.src_node) {
                if let Some(idx) = self
                    .graph_nodes
                    .iter()
                    .position(|(id, _)| *id == e.src_node)
                {
                    self.graph_cursor = idx;
                    return;
                }
            }
        }
    }

    fn enter_nested_graph(&mut self) {
        let node_id = self.graph_nodes[self.graph_cursor].0;

        self.graph_stack.push(GraphFrame {
            nodes: self.graph_nodes.clone(),
            edges: self.graph_edges.clone(),
            cursor: self.graph_cursor,
            params: self.graph_params.clone(),
            param_values: self.graph_param_values.clone(),
            param_groups: self.graph_param_groups.clone(),
            depths: self.graph_depths.clone(),
            layers: self.graph_layers.clone(),
            has_children: self.graph_has_children.clone(),
            node_descriptions: self.graph_node_descriptions.clone(),
        });

        let entered_name = self.graph_nodes[self.graph_cursor].1.clone();
        self.graph_path.push(node_id);
        self.graph_breadcrumb.push(entered_name);

        if let Some(snap) = self.nested_graph_snapshots.get(&self.graph_path).cloned() {
            self.load_graph_from_snapshot(&snap);
        } else {
            // No host snapshot for this path — keep empty placeholder until a bridge protocol exists.
            self.graph_nodes = vec![];
            self.graph_edges = vec![];
            self.graph_depths = vec![];
            self.graph_layers = vec![];
            self.graph_params = vec![];
            self.graph_param_values = vec![];
            self.graph_param_groups = vec![];
            self.graph_has_children = vec![];
            self.graph_cursor = 0;
        }
    }

    fn exit_nested_graph(&mut self) {
        if let Some(frame) = self.graph_stack.pop() {
            self.graph_nodes = frame.nodes;
            self.graph_edges = frame.edges;
            self.graph_cursor = frame.cursor;
            self.graph_params = frame.params;
            self.graph_param_values = frame.param_values;
            self.graph_param_groups = frame.param_groups;
            self.graph_depths = frame.depths;
            self.graph_layers = frame.layers;
            self.graph_has_children = frame.has_children;
            self.graph_node_descriptions = frame.node_descriptions;
            self.graph_path.pop();
            self.graph_breadcrumb.pop();
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
                Notification::ScopeData(snap) => {
                    self.scope_master = snap.master;
                    self.scope_graph_in = snap.graph_in;
                }
                Notification::Meter { peak_l, peak_r } => {
                    self.peak_l = peak_l;
                    self.peak_r = peak_r;
                }
                Notification::Stopped => {
                    self.playing = false;
                    self.engine_pattern_active = false;
                    self.current_play_row = None;
                }
            }
        }
    }

    #[inline]
    fn should_sync_pattern(&self) -> bool {
        self.playing || self.engine_pattern_active
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
        let loop_len = ((self.grid.rows as f64 * 60.0 / self.bpm.max(1e-6)) * 44_100.0)
            .round()
            .max(0.0) as usize;
        self.bridge.send(Command::LoadEvents { events, loop_len });
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
            if self.should_sync_pattern() {
                self.send_pattern();
            }
        }
    }

    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(self.grid.cells.clone());
            self.grid.cells = snapshot;
            if self.should_sync_pattern() {
                self.send_pattern();
            }
        }
    }

    fn render_main_canvas(
        &mut self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        roll_loop_beats: Option<f64>,
    ) {
        if let Some(roll) = &mut self.pattern_roll {
            roll.draw(
                frame,
                area,
                self.playing,
                self.beat_position,
                roll_loop_beats.unwrap_or(self.grid.rows as f64),
            );
            return;
        }

        match self.editor {
            Editor::Pattern => {
                if let Some(workspace) = self.project_workspace.as_ref() {
                    frame.render_widget(
                        OverviewView {
                            scene: &workspace.scene,
                            clips: &workspace.clips,
                            selected_lane: self.overview_lane,
                            selected_block: self.overview_block,
                            beat_position: self.beat_position,
                            playing: self.playing,
                        },
                        area,
                    );
                } else {
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
                        area,
                    );
                }
            }
            Editor::Graph => {
                let params = self.graph_params.get(self.graph_cursor);
                let values = self.graph_param_values.get(self.graph_cursor);
                let groups = self.graph_param_groups.get(self.graph_cursor);
                frame.render_widget(
                    GraphViewWidget {
                        nodes: &self.graph_nodes,
                        edges: &self.graph_edges,
                        selected: self.graph_cursor,
                        params: params.map(|p| p.as_slice()),
                        param_values: values.map(|v| v.as_slice()),
                        param_groups: groups.map(|g| g.as_slice()),
                        param_cursor: if self.mode == Mode::Edit {
                            Some(self.param_cursor)
                        } else {
                            None
                        },
                        breadcrumb: &self.graph_breadcrumb,
                        has_children: &self.graph_has_children,
                    },
                    area,
                );
            }
        }
    }

    fn render_sidebar(&self, frame: &mut ratatui::Frame<'_>, area: Rect, shell: &ShellContext) {
        if self.help_open {
            frame.render_widget(
                HelpOverlay {
                    project_mode: shell.project_mode,
                    zone: &shell.zone,
                    mode: &shell.mode,
                    tool: &shell.tool,
                },
                area,
            );
        } else {
            frame.render_widget(
                ContextPanel {
                    title: "INFO",
                    zone: &shell.zone,
                    mode: &shell.mode,
                    tool: &shell.tool,
                    frames: &shell.frames,
                    details: &shell.info_lines,
                    selection: &shell.selection_summary,
                    actions: &shell.actions_summary,
                    esc_hint: shell.esc_hint.as_deref(),
                    playing: self.playing,
                    bpm: self.bpm,
                    beat_position: self.beat_position,
                },
                area,
            );
        }
    }

    fn render_bottom_panel(&mut self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let now = Instant::now();
        self.spectrum_analyzer_in.fall_ms = self.spectrum_fall_ms;
        self.spectrum_analyzer_out.fall_ms = self.spectrum_fall_ms;
        let (spec_in, nr_in) = self.spectrum_analyzer_in.analyze(&self.scope_graph_in, now);
        let (spec_out, nr_out) = self.spectrum_analyzer_out.analyze(&self.scope_master, now);

        match (self.editor, self.bottom_pane) {
            (_, BottomPane::Hidden) => {}
            (Editor::Graph, BottomPane::Waveform) => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(area);
                frame.render_widget(
                    ScopeView {
                        samples: &self.scope_graph_in,
                    },
                    chunks[0],
                );
                frame.render_widget(
                    ScopeView {
                        samples: &self.scope_master,
                    },
                    chunks[1],
                );
            }
            (Editor::Graph, BottomPane::Spectrum) => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(area);
                let fall = self.spectrum_fall_ms;
                frame.render_widget(
                    SpectrumView {
                        magnitudes: spec_in,
                        norm_ref: nr_in,
                        title: "IN",
                        decay_ms_label: fall,
                    },
                    chunks[0],
                );
                frame.render_widget(
                    SpectrumView {
                        magnitudes: spec_out,
                        norm_ref: nr_out,
                        title: "OUT",
                        decay_ms_label: fall,
                    },
                    chunks[1],
                );
            }
            (Editor::Pattern, BottomPane::Waveform) => {
                frame.render_widget(
                    ScopeView {
                        samples: &self.scope_master,
                    },
                    area,
                );
            }
            (Editor::Pattern, BottomPane::Spectrum) => {
                frame.render_widget(
                    SpectrumView {
                        magnitudes: spec_out,
                        norm_ref: nr_out,
                        title: "OUT",
                        decay_ms_label: self.spectrum_fall_ms,
                    },
                    area,
                );
            }
        }
    }

    fn render_fullscreen_overlays(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        shell: &ShellContext,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let hud_area = Rect::new(area.x, area.y, area.width, 1);
        frame.render_widget(
            FullscreenHud {
                zone: &shell.zone,
                mode: &shell.mode,
                tool: &shell.tool,
                focus_path: &shell.focus_path,
                esc_hint: shell.esc_hint.as_deref(),
            },
            hud_area,
        );

        if self.help_open {
            let overlay = fullscreen_overlay_rect(area);
            frame.render_widget(Clear, overlay);
            self.render_sidebar(frame, overlay, shell);
        }
    }

    /// Lays out the focus shell: transport, main view, optional feedback panel, and status strip.
    pub fn draw(&mut self, frame: &mut ratatui::Frame) {
        self.refresh_host_stats();

        let focus_stack = self.focus_stack();
        let shell = self.build_shell_context(&focus_stack);
        let roll_loop_beats = self.pattern_roll_loop_beats();

        if self.fullscreen {
            self.render_main_canvas(frame, frame.area(), roll_loop_beats);
            self.render_fullscreen_overlays(frame, frame.area(), &shell);
            return;
        }

        let bottom_h = self.bottom_panel_height();
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(4),
                Constraint::Length(bottom_h),
                Constraint::Length(1),
            ])
            .split(frame.area());
        let content = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(30)])
            .split(outer[1]);

        frame.render_widget(
            TransportView {
                bpm: self.bpm,
                beat_position: self.beat_position,
                playing: self.playing,
                mode: &self.mode,
                editor: &self.editor,
                zone: &shell.zone,
                mode_label: &shell.mode,
                tool_label: &shell.tool,
                focus_path: &shell.focus_path,
                project_mode: shell.project_mode,
                project_name: shell.project_name.as_deref(),
                scale_name: &self.scale_name,
                octave: self.octave,
                swing: self.swing,
                bottom_pane: self.bottom_pane,
            },
            outer[0],
        );

        self.render_main_canvas(frame, content[0], roll_loop_beats);
        self.render_sidebar(frame, content[1], &shell);
        self.render_bottom_panel(frame, outer[2]);

        frame.render_widget(
            StatusBar {
                selection: &shell.selection_summary,
                actions: &shell.actions_summary,
                esc_hint: shell.esc_hint.as_deref(),
            },
            outer[3],
        );
    }

    /// Terminal main loop until quit: render, handle keys, poll notifications.
    pub fn run<B>(mut self, terminal: &mut ratatui::Terminal<B>) -> anyhow::Result<()>
    where
        B: ratatui::backend::Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        self.sync_scope_focus();
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Release {
                        if self.pattern_roll.is_some() {
                            if matches!(key.code, KeyCode::Enter)
                                && key.modifiers.contains(KeyModifiers::SHIFT)
                            {
                                self.handle_action(Action::ToggleFullscreen);
                            } else if matches!(key.code, KeyCode::Char('?')) {
                                self.handle_action(Action::ToggleHelp);
                            } else if matches!(key.code, KeyCode::Char('i')) {
                                self.handle_action(Action::ShowInfoPane);
                            } else if self.help_open && matches!(key.code, KeyCode::Esc) {
                                self.handle_action(Action::ToggleHelp);
                            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                                match key.code {
                                    KeyCode::Char('c') | KeyCode::Char('q') => {
                                        self.should_quit = true;
                                    }
                                    _ => {
                                        let bpm = self.bpm;
                                        let out = {
                                            let bridge = &mut self.bridge;
                                            let playing = &mut self.playing;
                                            let engine = &mut self.engine_pattern_active;
                                            self.pattern_roll.as_mut().map(|roll| {
                                                roll.handle_key(
                                                    key, bridge, bpm, 44100.0, playing, engine,
                                                )
                                            })
                                        };
                                        if out
                                            == Some(
                                                crate::pattern_roll::PatternRollOutcome::CloseApply,
                                            )
                                        {
                                            self.close_pattern_roll_apply();
                                        }
                                    }
                                }
                            } else {
                                let bpm = self.bpm;
                                let out = {
                                    let bridge = &mut self.bridge;
                                    let playing = &mut self.playing;
                                    let engine = &mut self.engine_pattern_active;
                                    self.pattern_roll.as_mut().map(|roll| {
                                        roll.handle_key(key, bridge, bpm, 44100.0, playing, engine)
                                    })
                                };
                                if out == Some(crate::pattern_roll::PatternRollOutcome::CloseApply)
                                {
                                    self.close_pattern_roll_apply();
                                }
                            }
                        } else {
                            let ctx = InputContext {
                                editor: self.editor,
                                mode: &self.mode,
                                graph_is_nested: !self.graph_path.is_empty(),
                                help_open: self.help_open,
                            };
                            if let Some(action) = input::handle_key(key, &ctx) {
                                self.handle_action(action);
                            }
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

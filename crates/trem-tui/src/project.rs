//! Project workspace model backed by `trem-project` package files.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use num_rational::Rational64;
use trem::event::{cmp_timed_event_delivery, GraphEvent, TimedEvent};
use trem::graph::Edge;
use trem::rung::{BeatTime, Clip, ClipNote, NoteMeta};
use trem_project::{
    BlockContent, ClipDocument, GraphDocument, GraphNodeKind, LanePerformer, ProjectPackage,
    SceneDocument,
};

/// Default package path loaded by the rebuilt TUI.
pub const DEFAULT_PROJECT_DIR: &str = "demos/easybeat";

#[inline]
pub fn default_project_path() -> PathBuf {
    PathBuf::from(DEFAULT_PROJECT_DIR)
}

/// Loaded project package plus authored sub-documents used by the TUI shell.
#[derive(Clone)]
pub struct ProjectWorkspace {
    pub root: PathBuf,
    pub package: ProjectPackage,
    pub scene: SceneDocument,
    pub clips: BTreeMap<String, ClipDocument>,
    pub graphs: BTreeMap<String, GraphDocument>,
}

/// Graph data adapted for the existing graph widget.
pub struct GraphViewData {
    pub name: String,
    pub nodes: Vec<(u32, String)>,
    pub edges: Vec<Edge>,
}

/// Simple runtime playback scene built from authored project documents.
pub struct Scene {
    pub bpm: f64,
    pub loop_beats: Rational64,
    pub loop_len: usize,
    pub events: Vec<TimedEvent>,
}

/// Background scene timing used while editing one project clip in the piano roll.
pub struct ClipPreviewContext {
    pub block_start: Rational64,
    pub loop_beats: Rational64,
    pub background_events: Vec<TimedEvent>,
}

impl ProjectWorkspace {
    /// Loads the root manifest, root scene, and referenced clip/graph docs.
    pub fn load(root: &Path) -> Result<Self> {
        let package = ProjectPackage::load(root)
            .with_context(|| format!("load project package {}", root.display()))?;
        let scene = package.load_root_scene().context("load root scene")?;

        let mut clips = BTreeMap::new();
        for clip_id in package.manifest.refs.clips.keys() {
            let clip = package
                .load_clip(clip_id)
                .with_context(|| format!("load clip {clip_id}"))?;
            clips.insert(clip_id.clone(), clip);
        }

        let mut graphs = BTreeMap::new();
        for graph_id in package.manifest.refs.graphs.keys() {
            let graph = package
                .load_graph(graph_id)
                .with_context(|| format!("load graph {graph_id}"))?;
            graphs.insert(graph_id.clone(), graph);
        }

        Ok(Self {
            root: root.to_path_buf(),
            package,
            scene,
            clips,
            graphs,
        })
    }

    /// Reloads the same package from disk.
    pub fn reload(&self) -> Result<Self> {
        Self::load(&self.root)
    }

    /// Writes the current scene plus loaded clip/graph docs back to disk.
    pub fn save(&self) -> Result<()> {
        self.package.save_manifest().context("save manifest")?;

        let scene_path = self
            .package
            .root_scene_path()
            .context("resolve root scene")?;
        write_toml(&scene_path, &self.scene)
            .with_context(|| format!("write {}", scene_path.display()))?;

        for (clip_id, rel) in &self.package.manifest.refs.clips {
            if let Some(clip) = self.clips.get(clip_id) {
                let path = self.root.join(rel);
                write_json(&path, clip).with_context(|| format!("write {}", path.display()))?;
            }
        }

        for (graph_id, rel) in &self.package.manifest.refs.graphs {
            if let Some(graph) = self.graphs.get(graph_id) {
                let path = self.root.join(rel);
                write_json(&path, graph).with_context(|| format!("write {}", path.display()))?;
            }
        }

        Ok(())
    }

    /// Project display name from the root manifest.
    pub fn project_name(&self) -> &str {
        &self.package.manifest.project.name
    }

    /// Project tempo in BPM.
    pub fn tempo_bpm(&self) -> f64 {
        self.package.manifest.project.tempo_bpm as f64
    }

    /// Integer timeline size used for viewport defaults.
    pub fn timeline_beats_u32(&self) -> u32 {
        parse_beat_expr(&self.scene.scene.timeline_beats)
            .filter(|beats| *beats.denom() == 1 && *beats.numer() > 0)
            .map(|beats| *beats.numer() as u32)
            .unwrap_or(16)
    }

    /// Total lane count in the root scene.
    pub fn lane_count(&self) -> usize {
        self.scene.lanes.len()
    }

    /// Returns a lane by index.
    pub fn lane(&self, lane_idx: usize) -> Option<&trem_project::LaneSpec> {
        self.scene.lanes.get(lane_idx)
    }

    /// Returns the selected block on the given lane.
    pub fn block(&self, lane_idx: usize, block_idx: usize) -> Option<&trem_project::BlockSpec> {
        self.lane(lane_idx)?.blocks.get(block_idx)
    }

    /// Resolves the selected block to a clip document.
    pub fn clip_for_selection(&self, lane_idx: usize, block_idx: usize) -> Option<&ClipDocument> {
        let block = self.block(lane_idx, block_idx)?;
        match &block.content {
            BlockContent::Clip { clip } => self.clips.get(clip),
            _ => None,
        }
    }

    /// Replaces one authored clip in memory.
    pub fn replace_clip(&mut self, clip: ClipDocument) {
        self.clips.insert(clip.clip.id.clone(), clip);
    }

    /// Returns the graph view implied by the current overview selection.
    pub fn graph_view_for_selection(
        &self,
        lane_idx: usize,
        block_idx: usize,
    ) -> Option<GraphViewData> {
        let lane = self.lane(lane_idx)?;

        if let Some(block) = lane.blocks.get(block_idx) {
            if let BlockContent::Graph { graph } = &block.content {
                return self.graph_view(graph);
            }
        }

        match lane.performer.as_ref()? {
            LanePerformer::Graph { graph } => self.graph_view(graph),
            LanePerformer::Standard { tag } => Some(standard_graph_view(&lane.label, tag)),
        }
    }

    /// Updates the manifest tempo in memory.
    pub fn set_tempo_bpm(&mut self, tempo_bpm: u16) {
        self.package.manifest.project.tempo_bpm = tempo_bpm.max(1);
    }

    fn graph_view(&self, graph_id: &str) -> Option<GraphViewData> {
        self.graphs.get(graph_id).map(graph_document_to_view)
    }
}

impl Scene {
    /// Compiles the current project workspace into a simple looping event list.
    pub fn from_workspace(workspace: &ProjectWorkspace) -> Self {
        let bpm = workspace.tempo_bpm();
        let loop_beats = parse_beat_expr(&workspace.scene.scene.timeline_beats)
            .filter(|beats| *beats > Rational64::from_integer(0))
            .unwrap_or_else(|| Rational64::from_integer(16));
        let mut events = Vec::new();

        for (lane_idx, lane) in workspace.scene.lanes.iter().enumerate() {
            let voice = voice_for_lane(lane_idx, &lane.id);
            for block in &lane.blocks {
                let BlockContent::Clip { clip } = &block.content else {
                    continue;
                };
                let Some(clip_doc) = workspace.clips.get(clip) else {
                    continue;
                };
                let block_start =
                    parse_beat_expr(&block.start).unwrap_or_else(|| Rational64::from_integer(0));
                for note in &clip_doc.notes {
                    let note_start = block_start
                        + parse_beat_expr(&note.start)
                            .unwrap_or_else(|| Rational64::from_integer(0));
                    let note_length = parse_beat_expr(&note.length)
                        .unwrap_or_else(|| Rational64::new(1, 4))
                        .max(Rational64::new(1, 64));
                    let on = beat_to_samples(note_start, bpm);
                    let off =
                        beat_to_samples(note_start + note_length, bpm).max(on.saturating_add(1));
                    events.push(TimedEvent {
                        sample_offset: on,
                        event: GraphEvent::NoteOn {
                            frequency: midi_to_hz(i32::from(note.pitch)),
                            velocity: (f64::from(note.velocity) / 127.0).clamp(0.0, 1.0),
                            voice,
                        },
                    });
                    events.push(TimedEvent {
                        sample_offset: off,
                        event: GraphEvent::NoteOff { voice },
                    });
                }
            }
        }

        events.sort_by(cmp_timed_event_delivery);
        Self {
            bpm,
            loop_len: beat_to_samples(loop_beats, bpm),
            loop_beats,
            events,
        }
    }
}

/// Returns exact preview timing for the selected project clip plus the rest of the scene.
pub fn clip_preview_context(
    workspace: &ProjectWorkspace,
    lane_idx: usize,
    block_idx: usize,
) -> Option<ClipPreviewContext> {
    let bpm = workspace.tempo_bpm();
    let loop_beats = parse_beat_expr(&workspace.scene.scene.timeline_beats)
        .filter(|beats| *beats > Rational64::from_integer(0))
        .unwrap_or_else(|| Rational64::from_integer(16));
    let block = workspace.block(lane_idx, block_idx)?;
    let BlockContent::Clip { .. } = &block.content else {
        return None;
    };
    let block_start = parse_beat_expr(&block.start).unwrap_or_else(|| Rational64::from_integer(0));
    let mut background_events = Vec::new();

    for (other_lane_idx, lane) in workspace.scene.lanes.iter().enumerate() {
        let voice = voice_for_lane(other_lane_idx, &lane.id);
        for (other_block_idx, other_block) in lane.blocks.iter().enumerate() {
            let BlockContent::Clip { clip } = &other_block.content else {
                continue;
            };
            if other_lane_idx == lane_idx && other_block_idx == block_idx {
                continue;
            }
            let Some(clip_doc) = workspace.clips.get(clip) else {
                continue;
            };
            let start =
                parse_beat_expr(&other_block.start).unwrap_or_else(|| Rational64::from_integer(0));
            append_clip_document_events(&mut background_events, clip_doc, voice, start, bpm);
        }
    }

    background_events.sort_by(cmp_timed_event_delivery);
    Some(ClipPreviewContext {
        block_start,
        loop_beats,
        background_events,
    })
}

/// Voice ids used by the simple playback scene and piano-roll previews.
pub fn lane_voice_ids(scene: &SceneDocument) -> Vec<u32> {
    scene
        .lanes
        .iter()
        .enumerate()
        .map(|(lane_idx, lane)| voice_for_lane(lane_idx, &lane.id))
        .collect()
}

/// Parses `"16"` or `"3/2"` style beat expressions.
pub fn parse_beat_expr(expr: &str) -> Option<Rational64> {
    let expr = expr.trim();
    if expr.is_empty() {
        return None;
    }
    if let Some((n, d)) = expr.split_once('/') {
        let numer: i64 = n.trim().parse().ok()?;
        let denom: i64 = d.trim().parse().ok()?;
        if denom == 0 {
            return None;
        }
        Some(Rational64::new(numer, denom))
    } else {
        Some(Rational64::from_integer(expr.parse().ok()?))
    }
}

/// Formats a beat value back to a compact authored string.
pub fn format_beat_expr(beats: Rational64) -> String {
    if *beats.denom() == 1 {
        beats.numer().to_string()
    } else {
        format!("{}/{}", beats.numer(), beats.denom())
    }
}

/// Converts a project clip into the existing piano-roll clip type.
pub fn clip_document_to_roll_clip(doc: &ClipDocument, voice: u32) -> Clip {
    let mut notes: Vec<ClipNote> = doc
        .notes
        .iter()
        .map(|note| {
            let start = parse_beat_expr(&note.start).unwrap_or_else(|| Rational64::from_integer(0));
            let length = parse_beat_expr(&note.length).unwrap_or_else(|| Rational64::new(1, 4));
            ClipNote {
                id: None,
                class: i32::from(note.pitch).clamp(0, 127),
                t_on: BeatTime(start),
                t_off: BeatTime(start + length.max(Rational64::new(1, 64))),
                voice,
                velocity: (f64::from(note.velocity) / 127.0).clamp(0.0, 1.0),
                meta: NoteMeta::default(),
            }
        })
        .collect();
    notes.sort_by(|a, b| {
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.class.cmp(&b.class))
    });
    Clip {
        notes,
        length_beats: parse_beat_expr(&doc.clip.length_beats).map(BeatTime),
    }
}

/// Converts the edited piano-roll clip back into the authored project clip format.
pub fn roll_clip_to_document(template: &ClipDocument, clip: &Clip) -> ClipDocument {
    let mut out = template.clone();
    out.clip.length_beats = clip
        .length_beats
        .map(|beats| format_beat_expr(beats.rational()))
        .unwrap_or_else(|| template.clip.length_beats.clone());
    out.notes = clip
        .notes
        .iter()
        .map(|note| {
            let start = note.t_on.rational();
            let length = (note.t_off.rational() - note.t_on.rational()).max(Rational64::new(1, 64));
            trem_project::ClipNote {
                pitch: note.class.clamp(0, 127) as i16,
                start: format_beat_expr(start),
                length: format_beat_expr(length),
                velocity: (note.velocity.clamp(0.0, 1.0) * 127.0)
                    .round()
                    .clamp(1.0, 127.0) as u8,
            }
        })
        .collect();
    out
}

fn graph_document_to_view(graph: &GraphDocument) -> GraphViewData {
    let ids: BTreeMap<&str, u32> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| (node.id.as_str(), idx as u32 + 1))
        .collect();

    let nodes = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| (idx as u32 + 1, graph_node_label(node)))
        .collect();

    let mut edges = Vec::new();
    for node in &graph.nodes {
        let Some(&dst) = ids.get(node.id.as_str()) else {
            continue;
        };
        for (dst_port, input) in node.inputs.iter().enumerate() {
            let Some(&src) = ids.get(input.as_str()) else {
                continue;
            };
            edges.push(Edge {
                src_node: src,
                src_port: 0,
                dst_node: dst,
                dst_port: dst_port as u16,
            });
        }
    }

    GraphViewData {
        name: graph.graph.name.clone(),
        nodes,
        edges,
    }
}

fn standard_graph_view(label: &str, tag: &str) -> GraphViewData {
    GraphViewData {
        name: label.to_string(),
        nodes: vec![(1, format!("{label} · {tag}")), (2, "Output".into())],
        edges: vec![Edge {
            src_node: 1,
            src_port: 0,
            dst_node: 2,
            dst_port: 0,
        }],
    }
}

fn graph_node_label(node: &trem_project::GraphNodeSpec) -> String {
    match &node.kind {
        GraphNodeKind::Input { port } => format!("{} < {}", node.label, port),
        GraphNodeKind::Output { port } => format!("{} > {}", node.label, port),
        GraphNodeKind::Standard { tag } => format!("{} · {}", node.label, tag),
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text)?;
    Ok(())
}

fn write_toml<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(value)?;
    fs::write(path, text)?;
    Ok(())
}

fn voice_for_lane(lane_idx: usize, lane_id: &str) -> u32 {
    match lane_id {
        "lead" => 0,
        "bass" => 1,
        "kick" => 2,
        "snare" => 3,
        "hat" => 4,
        _ => lane_idx as u32,
    }
}

fn append_clip_document_events(
    events: &mut Vec<TimedEvent>,
    clip_doc: &ClipDocument,
    voice: u32,
    block_start: Rational64,
    bpm: f64,
) {
    for note in &clip_doc.notes {
        let note_start = block_start
            + parse_beat_expr(&note.start).unwrap_or_else(|| Rational64::from_integer(0));
        let note_length = parse_beat_expr(&note.length)
            .unwrap_or_else(|| Rational64::new(1, 4))
            .max(Rational64::new(1, 64));
        let on = beat_to_samples(note_start, bpm);
        let off = beat_to_samples(note_start + note_length, bpm).max(on.saturating_add(1));
        events.push(TimedEvent {
            sample_offset: on,
            event: GraphEvent::NoteOn {
                frequency: midi_to_hz(i32::from(note.pitch)),
                velocity: (f64::from(note.velocity) / 127.0).clamp(0.0, 1.0),
                voice,
            },
        });
        events.push(TimedEvent {
            sample_offset: off,
            event: GraphEvent::NoteOff { voice },
        });
    }
}

fn beat_to_samples(beats: Rational64, bpm: f64) -> usize {
    let beat_f = *beats.numer() as f64 / *beats.denom() as f64;
    let seconds = beat_f * 60.0 / bpm.max(1e-6);
    (seconds * 44_100.0).round().max(0.0) as usize
}

fn midi_to_hz(midi: i32) -> f64 {
    let note = midi.clamp(0, 127) as f64;
    440.0 * 2.0_f64.powf((note - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easybeat_runtime_scene_has_tempo_and_events() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demos/easybeat");
        let workspace = ProjectWorkspace::load(&root).expect("load demo workspace");
        let scene = Scene::from_workspace(&workspace);
        assert_eq!(scene.bpm, 146.0);
        assert_eq!(scene.loop_beats, Rational64::from_integer(16));
        assert!(scene.loop_len > 0);
        assert!(!scene.events.is_empty(), "easybeat should emit note events");
        assert!(scene
            .events
            .iter()
            .any(|event| matches!(event.event, GraphEvent::NoteOn { voice: 0, .. })));
        assert!(scene
            .events
            .iter()
            .any(|event| matches!(event.event, GraphEvent::NoteOn { voice: 2, .. })));
    }
}

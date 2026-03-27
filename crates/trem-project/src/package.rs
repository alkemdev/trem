//! Load/save helpers for hybrid `trem.toml` project packages.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::clip::{ClipDocument, ClipKind, ClipMeta, ClipNote};
use crate::graph::{GraphDocument, GraphMeta, GraphNodeKind, GraphNodeSpec};
use crate::layout::MANIFEST_FILE;
use crate::manifest::ProjectManifest;
use crate::scene::SceneDocument;

/// Loaded project package rooted at a directory containing `trem.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectPackage {
    pub root: PathBuf,
    pub manifest: ProjectManifest,
}

/// Errors from reading, writing, or validating a trem project package.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest TOML parse error: {0}")]
    ManifestToml(#[from] toml::de::Error),
    #[error("manifest validation error: {0}")]
    InvalidManifest(String),
    #[error("manifest TOML encode error: {0}")]
    ManifestEncode(#[from] toml::ser::Error),
    #[error("scene TOML parse error: {0}")]
    SceneToml(toml::de::Error),
    #[error("scene validation error: {0}")]
    InvalidScene(String),
    #[error("clip JSON error: {0}")]
    ClipJson(serde_json::Error),
    #[error("clip validation error: {0}")]
    InvalidClip(String),
    #[error("graph JSON error: {0}")]
    GraphJson(serde_json::Error),
    #[error("graph validation error: {0}")]
    InvalidGraph(String),
}

impl ProjectPackage {
    /// Path to `trem.toml` under `root`.
    pub fn manifest_path(root: &Path) -> PathBuf {
        root.join(MANIFEST_FILE)
    }

    /// Loads and validates the root manifest.
    pub fn load(root: &Path) -> Result<Self, PackageError> {
        let manifest_path = Self::manifest_path(root);
        let text = fs::read_to_string(&manifest_path)?;
        let manifest: ProjectManifest = toml::from_str(&text)?;
        manifest
            .validate_basic()
            .map_err(PackageError::InvalidManifest)?;
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// Writes the root manifest back to disk.
    pub fn save_manifest(&self) -> Result<(), PackageError> {
        fs::create_dir_all(&self.root)?;
        let path = Self::manifest_path(&self.root);
        let text = toml::to_string_pretty(&self.manifest)?;
        fs::write(path, text)?;
        Ok(())
    }

    /// Resolves the scene file for the manifest's `root_scene`.
    pub fn root_scene_path(&self) -> Result<PathBuf, PackageError> {
        let rel = self.resolve_ref(
            &self.manifest.refs.scenes,
            &self.manifest.project.root_scene,
            "scene",
        )?;
        Ok(self.root.join(rel))
    }

    /// Loads and validates the root scene document.
    pub fn load_root_scene(&self) -> Result<SceneDocument, PackageError> {
        let path = self.root_scene_path()?;
        let text = fs::read_to_string(path)?;
        let scene: SceneDocument = toml::from_str(&text).map_err(PackageError::SceneToml)?;
        scene.validate_basic().map_err(PackageError::InvalidScene)?;
        Ok(scene)
    }

    /// Loads and validates a named clip document.
    pub fn load_clip(&self, clip: &str) -> Result<ClipDocument, PackageError> {
        let path = self.resolve_ref(&self.manifest.refs.clips, clip, "clip")?;
        let text = fs::read_to_string(self.root.join(path))?;
        let clip_doc: ClipDocument = serde_json::from_str(&text).map_err(PackageError::ClipJson)?;
        clip_doc
            .validate_basic()
            .map_err(PackageError::InvalidClip)?;
        Ok(clip_doc)
    }

    /// Loads and validates a named graph document.
    pub fn load_graph(&self, graph: &str) -> Result<GraphDocument, PackageError> {
        let path = self.resolve_ref(&self.manifest.refs.graphs, graph, "graph")?;
        let text = fs::read_to_string(self.root.join(path))?;
        let graph_doc: GraphDocument =
            serde_json::from_str(&text).map_err(PackageError::GraphJson)?;
        graph_doc
            .validate_basic()
            .map_err(PackageError::InvalidGraph)?;
        Ok(graph_doc)
    }

    /// Writes a root-manifest + root-scene `easybeat` package scaffold to `root`.
    pub fn scaffold_easybeat(root: &Path) -> Result<Self, PackageError> {
        let package = Self {
            root: root.to_path_buf(),
            manifest: ProjectManifest::easybeat(),
        };
        package.save_manifest()?;

        let scene_path = package.root_scene_path()?;
        if let Some(parent) = scene_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let scene_text = toml::to_string_pretty(&SceneDocument::easybeat())?;
        fs::write(scene_path, scene_text)?;

        for clip in easybeat_clips() {
            let rel = package.resolve_ref(&package.manifest.refs.clips, &clip.clip.id, "clip")?;
            write_json(&package.root.join(rel), &clip).map_err(PackageError::ClipJson)?;
        }

        for graph in easybeat_graphs() {
            let rel =
                package.resolve_ref(&package.manifest.refs.graphs, &graph.graph.id, "graph")?;
            write_json(&package.root.join(rel), &graph).map_err(PackageError::GraphJson)?;
        }

        Ok(package)
    }

    fn resolve_ref<'a>(
        &'a self,
        refs: &'a std::collections::BTreeMap<String, String>,
        label: &str,
        kind: &str,
    ) -> Result<&'a str, PackageError> {
        refs.get(label).map(String::as_str).ok_or_else(|| {
            PackageError::InvalidManifest(format!("missing {kind} reference for {label:?}"))
        })
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), serde_json::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(serde_json::Error::io)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text).map_err(serde_json::Error::io)?;
    Ok(())
}

fn easybeat_clips() -> Vec<ClipDocument> {
    vec![
        clip(
            "kick",
            "Kick",
            ClipKind::DrumPattern,
            vec![
                note(36, "0", "1/8", 120),
                note(36, "1", "1/8", 108),
                note(36, "2", "1/8", 116),
                note(36, "3", "1/8", 108),
                note(36, "4", "1/8", 120),
                note(36, "11/2", "1/8", 94),
                note(36, "6", "1/8", 114),
                note(36, "7", "1/8", 108),
                note(36, "8", "1/8", 122),
                note(36, "9", "1/8", 108),
                note(36, "10", "1/8", 116),
                note(36, "11", "1/8", 108),
                note(36, "12", "1/8", 120),
                note(36, "27/2", "1/8", 96),
                note(36, "14", "1/8", 118),
                note(36, "15", "1/8", 110),
            ],
        ),
        clip(
            "snare",
            "Snare",
            ClipKind::DrumPattern,
            vec![
                note(38, "1", "1/8", 116),
                note(38, "3", "1/8", 124),
                note(38, "5", "1/8", 118),
                note(38, "7", "1/8", 124),
                note(38, "9", "1/8", 116),
                note(38, "11", "1/8", 124),
                note(38, "13", "1/8", 118),
                note(38, "15", "1/8", 126),
            ],
        ),
        clip(
            "hat",
            "Hat",
            ClipKind::DrumPattern,
            (0..32)
                .map(|step| {
                    let start = if step % 2 == 0 {
                        format!("{}", step / 2)
                    } else {
                        format!("{}/2", step)
                    };
                    let velocity = if step % 4 == 1 {
                        88
                    } else if step % 4 == 3 {
                        76
                    } else {
                        68
                    };
                    note(42, &start, "1/4", velocity)
                })
                .collect(),
        ),
        clip(
            "bass",
            "Bass",
            ClipKind::PianoRoll,
            vec![
                note(33, "0", "3/4", 102),
                note(40, "1", "1/2", 86),
                note(33, "2", "3/4", 98),
                note(43, "3", "1/2", 88),
                note(29, "4", "3/4", 102),
                note(36, "5", "1/2", 84),
                note(29, "6", "3/4", 96),
                note(36, "7", "1/2", 86),
                note(36, "8", "3/4", 104),
                note(43, "9", "1/2", 84),
                note(36, "10", "3/4", 98),
                note(43, "11", "1/2", 88),
                note(31, "12", "3/4", 102),
                note(38, "13", "1/2", 84),
                note(31, "14", "3/4", 100),
                note(43, "15", "1/2", 92),
            ],
        ),
        clip(
            "lead",
            "Lead",
            ClipKind::PianoRoll,
            vec![
                note(69, "0", "1/2", 96),
                note(72, "1/2", "1/2", 84),
                note(76, "1", "1/2", 88),
                note(79, "3/2", "1/2", 82),
                note(81, "2", "1/2", 92),
                note(79, "5/2", "1/2", 84),
                note(76, "3", "1/2", 86),
                note(72, "7/2", "1/2", 90),
                note(65, "4", "1/2", 96),
                note(69, "9/2", "1/2", 84),
                note(72, "5", "1/2", 88),
                note(76, "11/2", "1/2", 84),
                note(79, "6", "1/2", 92),
                note(76, "13/2", "1/2", 84),
                note(72, "7", "1/2", 88),
                note(69, "15/2", "1/2", 96),
                note(72, "8", "1/2", 96),
                note(76, "17/2", "1/2", 84),
                note(79, "9", "1/2", 88),
                note(84, "19/2", "1/2", 84),
                note(79, "10", "1/2", 94),
                note(76, "21/2", "1/2", 86),
                note(72, "11", "1/2", 88),
                note(79, "23/2", "1/2", 96),
                note(67, "12", "1/2", 96),
                note(71, "25/2", "1/2", 84),
                note(74, "13", "1/2", 88),
                note(79, "27/2", "1/2", 84),
                note(83, "14", "1/2", 92),
                note(79, "29/2", "1/2", 86),
                note(74, "15", "1/2", 88),
                note(71, "31/2", "1/2", 100),
            ],
        ),
    ]
}

fn easybeat_graphs() -> Vec<GraphDocument> {
    vec![
        graph(
            "bass",
            "Bass Voice",
            vec![
                node(
                    "voice",
                    "Analog Voice",
                    GraphNodeKind::Standard { tag: "syn".into() },
                    vec![],
                    BTreeMap::from([
                        ("drive".into(), 0.25),
                        ("cutoff_hz".into(), 620.0),
                        ("env_amount".into(), 0.50),
                    ]),
                ),
                node(
                    "grit",
                    "Drive",
                    GraphNodeKind::Standard { tag: "dst".into() },
                    vec!["voice".into()],
                    BTreeMap::from([
                        ("shape".into(), 0.28),
                        ("drive".into(), 0.22),
                        ("mix".into(), 0.30),
                    ]),
                ),
                node(
                    "filter",
                    "Low Pass",
                    GraphNodeKind::Standard { tag: "lpf".into() },
                    vec!["grit".into()],
                    BTreeMap::from([("cutoff_hz".into(), 780.0), ("q".into(), 0.90)]),
                ),
                node(
                    "out",
                    "Output",
                    GraphNodeKind::Output {
                        port: "mono".into(),
                    },
                    vec!["filter".into()],
                    BTreeMap::new(),
                ),
            ],
        ),
        graph(
            "lead",
            "Lead Voice",
            vec![
                node(
                    "voice",
                    "Lead Voice",
                    GraphNodeKind::Standard { tag: "ldv".into() },
                    vec![],
                    BTreeMap::from([
                        ("attack_s".into(), 0.05),
                        ("cutoff_hz".into(), 1900.0),
                        ("glide_s".into(), 0.028),
                    ]),
                ),
                node(
                    "flutter",
                    "Flutter Delay",
                    GraphNodeKind::Standard { tag: "dly".into() },
                    vec!["voice".into()],
                    BTreeMap::from([
                        ("delay_ms".into(), 38.0),
                        ("feedback".into(), 0.40),
                        ("mix".into(), 0.22),
                    ]),
                ),
                node(
                    "air",
                    "Plate",
                    GraphNodeKind::Standard { tag: "vrb".into() },
                    vec!["flutter".into()],
                    BTreeMap::from([
                        ("size".into(), 0.30),
                        ("damp".into(), 0.62),
                        ("mix".into(), 0.10),
                    ]),
                ),
                node(
                    "out",
                    "Output",
                    GraphNodeKind::Output {
                        port: "stereo".into(),
                    },
                    vec!["air".into()],
                    BTreeMap::new(),
                ),
            ],
        ),
        graph(
            "main_mix",
            "Main Mix",
            vec![
                node(
                    "inst",
                    "Inst Bus",
                    GraphNodeKind::Input {
                        port: "inst_bus".into(),
                    },
                    vec![],
                    BTreeMap::new(),
                ),
                node(
                    "drums",
                    "Drum Bus",
                    GraphNodeKind::Input {
                        port: "drum_bus".into(),
                    },
                    vec![],
                    BTreeMap::new(),
                ),
                node(
                    "sum",
                    "Mixer",
                    GraphNodeKind::Standard { tag: "mix".into() },
                    vec!["inst".into(), "drums".into()],
                    BTreeMap::from([("channels".into(), 2.0)]),
                ),
                node(
                    "eq",
                    "EQ",
                    GraphNodeKind::Standard { tag: "peq".into() },
                    vec!["sum".into()],
                    BTreeMap::from([
                        ("low_db".into(), -1.5),
                        ("mid_db".into(), 3.0),
                        ("high_db".into(), 1.5),
                    ]),
                ),
                node(
                    "glue",
                    "Glue Compressor",
                    GraphNodeKind::Standard { tag: "com".into() },
                    vec!["eq".into()],
                    BTreeMap::from([
                        ("threshold_db".into(), -18.0),
                        ("ratio".into(), 3.0),
                        ("attack_ms".into(), 8.0),
                        ("release_ms".into(), 120.0),
                    ]),
                ),
                node(
                    "space",
                    "Stereo Delay",
                    GraphNodeKind::Standard { tag: "dly".into() },
                    vec!["glue".into()],
                    BTreeMap::from([
                        ("delay_ms".into(), 260.0),
                        ("feedback".into(), 0.16),
                        ("mix".into(), 0.035),
                    ]),
                ),
                node(
                    "air",
                    "Plate Reverb",
                    GraphNodeKind::Standard { tag: "vrb".into() },
                    vec!["space".into()],
                    BTreeMap::from([
                        ("size".into(), 0.32),
                        ("damp".into(), 0.62),
                        ("mix".into(), 0.045),
                    ]),
                ),
                node(
                    "limit",
                    "Limiter",
                    GraphNodeKind::Standard { tag: "lim".into() },
                    vec!["air".into()],
                    BTreeMap::from([("ceiling_db".into(), -0.3), ("release_ms".into(), 100.0)]),
                ),
                node(
                    "out",
                    "Main Out",
                    GraphNodeKind::Output {
                        port: "main_out".into(),
                    },
                    vec!["limit".into()],
                    BTreeMap::new(),
                ),
            ],
        ),
    ]
}

fn clip(id: &str, name: &str, kind: ClipKind, notes: Vec<ClipNote>) -> ClipDocument {
    ClipDocument {
        clip: ClipMeta {
            id: id.into(),
            name: name.into(),
            length_beats: "16".into(),
            kind,
        },
        notes,
    }
}

fn note(pitch: i16, start: &str, length: &str, velocity: u8) -> ClipNote {
    ClipNote {
        pitch,
        start: start.into(),
        length: length.into(),
        velocity,
    }
}

fn graph(id: &str, name: &str, nodes: Vec<GraphNodeSpec>) -> GraphDocument {
    GraphDocument {
        graph: GraphMeta {
            id: id.into(),
            name: name.into(),
        },
        nodes,
    }
}

fn node(
    id: &str,
    label: &str,
    kind: GraphNodeKind,
    inputs: Vec<String>,
    params: BTreeMap<String, f64>,
) -> GraphNodeSpec {
    GraphNodeSpec {
        id: id.into(),
        label: label.into(),
        kind,
        inputs,
        params,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easybeat_scaffold_roundtrips() {
        let root = std::env::temp_dir().join(format!(
            "trem_project_scaffold_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let package = ProjectPackage::scaffold_easybeat(&root).expect("scaffold");
        let loaded = ProjectPackage::load(&root).expect("load");
        assert_eq!(loaded.manifest.project.name, "easybeat");
        let scene = loaded.load_root_scene().expect("scene");
        assert_eq!(scene.scene.id, "easybeat");
        assert_eq!(scene.lanes.len(), 6);
        let lead = loaded.load_clip("lead").expect("lead clip");
        assert_eq!(lead.clip.kind, ClipKind::PianoRoll);
        let mix = loaded.load_graph("main_mix").expect("mix graph");
        assert!(mix.nodes.iter().any(|node| node.id == "limit"));
        let _ = fs::remove_dir_all(package.root);
    }

    #[test]
    fn checked_in_easybeat_demo_loads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demos/easybeat");
        let package = ProjectPackage::load(&root).expect("load demo");
        let scene = package.load_root_scene().expect("scene");
        assert_eq!(scene.scene.output, "main_out");
        let bass = package.load_clip("bass").expect("bass clip");
        assert_eq!(bass.clip.kind, ClipKind::PianoRoll);
        let lead = package.load_graph("lead").expect("lead graph");
        assert!(lead.nodes.iter().any(|node| node.id == "flutter"));
    }
}

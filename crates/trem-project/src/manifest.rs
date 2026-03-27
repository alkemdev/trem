//! Root `trem.toml` manifest types.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Root manifest for a trem project package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManifest {
    /// Project identity and root entrypoints.
    pub project: ProjectMeta,
    /// Machine-agnostic semantic ports used by the project.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ports: BTreeMap<String, PortSpec>,
    /// Top-level file references inside the project package.
    pub refs: ManifestRefs,
}

/// Core metadata from `trem.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// Human-visible project name.
    pub name: String,
    /// Default playback tempo in BPM.
    pub tempo_bpm: u16,
    /// Scene id that opens as the project root surface.
    pub root_scene: String,
}

/// File references from the root manifest into sub-documents.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRefs {
    /// Scene id -> relative path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scenes: BTreeMap<String, String>,
    /// Named clip asset -> relative path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub clips: BTreeMap<String, String>,
    /// Named graph asset -> relative path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub graphs: BTreeMap<String, String>,
    /// Named sample asset -> relative path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub samples: BTreeMap<String, String>,
    /// Named MIDI asset -> relative path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub midis: BTreeMap<String, String>,
}

/// Semantic I/O port used inside the project package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    /// Port kind (e.g. audio output or MIDI input).
    pub kind: PortKind,
    /// Optional channel count for audio ports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u16>,
}

/// Kind of semantic project port.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortKind {
    AudioOut,
    AudioIn,
    MidiIn,
    MidiOut,
    ClockIn,
    ClockOut,
    Custom,
}

impl ProjectManifest {
    /// Minimal `easybeat` root manifest scaffold.
    pub fn easybeat() -> Self {
        let mut ports = BTreeMap::new();
        ports.insert(
            "main_out".into(),
            PortSpec {
                kind: PortKind::AudioOut,
                channels: Some(2),
            },
        );

        let mut scenes = BTreeMap::new();
        scenes.insert("easybeat".into(), "scenes/easybeat.toml".into());

        let mut clips = BTreeMap::new();
        for clip in ["kick", "snare", "hat", "bass", "lead"] {
            clips.insert(clip.into(), format!("clips/{clip}.json"));
        }
        let mut graphs = BTreeMap::new();
        for graph in ["bass", "lead", "main_mix"] {
            graphs.insert(graph.into(), format!("graphs/{graph}.json"));
        }

        Self {
            project: ProjectMeta {
                name: "easybeat".into(),
                tempo_bpm: 146,
                root_scene: "easybeat".into(),
            },
            ports,
            refs: ManifestRefs {
                scenes,
                clips,
                graphs,
                ..ManifestRefs::default()
            },
        }
    }

    /// Structural validation for the root manifest.
    pub fn validate_basic(&self) -> Result<(), String> {
        if self.project.name.trim().is_empty() {
            return Err("project.name must not be empty".into());
        }
        if self.project.tempo_bpm == 0 {
            return Err("project.tempo_bpm must be > 0".into());
        }
        if self.project.root_scene.trim().is_empty() {
            return Err("project.root_scene must not be empty".into());
        }
        if !self.refs.scenes.contains_key(&self.project.root_scene) {
            return Err(format!(
                "root_scene {:?} is missing from refs.scenes",
                self.project.root_scene
            ));
        }
        for (label, path) in all_ref_paths(&self.refs) {
            validate_relative_path(label, path)?;
        }
        Ok(())
    }
}

fn all_ref_paths(refs: &ManifestRefs) -> impl Iterator<Item = (&str, &str)> {
    refs.scenes
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .chain(refs.clips.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .chain(refs.graphs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .chain(refs.samples.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .chain(refs.midis.iter().map(|(k, v)| (k.as_str(), v.as_str())))
}

fn validate_relative_path(label: &str, path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err(format!("reference {label:?} has an empty path"));
    }
    let p = Path::new(path);
    if p.is_absolute() {
        return Err(format!("reference {label:?} must be relative: {path:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easybeat_manifest_validates() {
        ProjectManifest::easybeat()
            .validate_basic()
            .expect("easybeat manifest should validate");
    }

    #[test]
    fn root_scene_must_exist() {
        let mut manifest = ProjectManifest::easybeat();
        manifest.project.root_scene = "missing".into();
        let err = manifest.validate_basic().expect_err("missing root scene");
        assert!(err.contains("root_scene"));
    }
}

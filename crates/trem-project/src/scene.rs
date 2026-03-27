//! Scene documents referenced from `trem.toml`.

use serde::{Deserialize, Serialize};

/// One scene document inside `scenes/*.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneDocument {
    /// Scene identity and timeline metadata.
    pub scene: SceneMeta,
    /// Fluid/custom lanes visible at the scene root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<LaneSpec>,
}

/// Metadata for a root scene surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneMeta {
    /// Scene id used by the root manifest.
    pub id: String,
    /// Display label.
    pub name: String,
    /// Semantic output port for this scene mix.
    pub output: String,
    /// Scene horizon in beats, stored as a string expression for now (`"16"`, `"8+8"` later).
    pub timeline_beats: String,
}

/// One custom row/lane on the scene timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneSpec {
    /// Stable lane id within the scene.
    pub id: String,
    /// Human-visible label.
    pub label: String,
    /// Lane kind hint.
    pub kind: LaneKind,
    /// Optional default performer for blocks on this lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performer: Option<LanePerformer>,
    /// Timed blocks shown on this lane.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<BlockSpec>,
}

/// Lane kind at the scene root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    Instrument,
    Bus,
    Marker,
    Control,
    Audio,
    Custom,
}

/// Default sound source or processor entered from a lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LanePerformer {
    Standard { tag: String },
    Graph { graph: String },
}

/// One timed root block that can be entered from the scene surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSpec {
    /// Stable block id within the scene.
    pub id: String,
    /// Human-visible block label.
    pub name: String,
    /// Start beat expression.
    pub start: String,
    /// Length beat expression.
    pub length: String,
    /// Typed payload / target of the block.
    #[serde(flatten)]
    pub content: BlockContent,
}

/// Typed content of a scene block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockContent {
    Clip { clip: String },
    Graph { graph: String },
    Sample { sample: String },
    Midi { midi: String },
    Marker { text: String },
}

impl SceneDocument {
    /// Minimal first-pass `easybeat` scene scaffold.
    pub fn easybeat() -> Self {
        Self {
            scene: SceneMeta {
                id: "easybeat".into(),
                name: "easybeat".into(),
                output: "main_out".into(),
                timeline_beats: "16".into(),
            },
            lanes: vec![
                lane_with_clip(
                    "kick",
                    "Kick",
                    Some(LanePerformer::Standard { tag: "kick".into() }),
                ),
                lane_with_clip(
                    "snare",
                    "Snare",
                    Some(LanePerformer::Standard { tag: "snr".into() }),
                ),
                lane_with_clip(
                    "hat",
                    "Hat",
                    Some(LanePerformer::Standard { tag: "hat".into() }),
                ),
                lane_with_clip(
                    "bass",
                    "Bass",
                    Some(LanePerformer::Graph {
                        graph: "bass".into(),
                    }),
                ),
                lane_with_clip(
                    "lead",
                    "Lead",
                    Some(LanePerformer::Graph {
                        graph: "lead".into(),
                    }),
                ),
                LaneSpec {
                    id: "main".into(),
                    label: "Main".into(),
                    kind: LaneKind::Bus,
                    performer: None,
                    blocks: vec![BlockSpec {
                        id: "main_mix".into(),
                        name: "Main Mix".into(),
                        start: "0".into(),
                        length: "16".into(),
                        content: BlockContent::Graph {
                            graph: "main_mix".into(),
                        },
                    }],
                },
            ],
        }
    }

    /// Structural validation for ids and beat strings.
    pub fn validate_basic(&self) -> Result<(), String> {
        if self.scene.id.trim().is_empty() {
            return Err("scene.id must not be empty".into());
        }
        if self.scene.name.trim().is_empty() {
            return Err("scene.name must not be empty".into());
        }
        if self.scene.output.trim().is_empty() {
            return Err("scene.output must not be empty".into());
        }
        if self.scene.timeline_beats.trim().is_empty() {
            return Err("scene.timeline_beats must not be empty".into());
        }
        let mut lane_ids = std::collections::BTreeSet::new();
        for lane in &self.lanes {
            if !lane_ids.insert(lane.id.as_str()) {
                return Err(format!("duplicate lane id {:?}", lane.id));
            }
            if lane.label.trim().is_empty() {
                return Err(format!("lane {:?} has an empty label", lane.id));
            }
            if let Some(performer) = &lane.performer {
                match performer {
                    LanePerformer::Standard { tag } if tag.trim().is_empty() => {
                        return Err(format!("lane {:?} has an empty standard tag", lane.id));
                    }
                    LanePerformer::Graph { graph } if graph.trim().is_empty() => {
                        return Err(format!("lane {:?} has an empty graph ref", lane.id));
                    }
                    _ => {}
                }
            }
            let mut block_ids = std::collections::BTreeSet::new();
            for block in &lane.blocks {
                if !block_ids.insert(block.id.as_str()) {
                    return Err(format!(
                        "duplicate block id {:?} inside lane {:?}",
                        block.id, lane.id
                    ));
                }
                if block.start.trim().is_empty() || block.length.trim().is_empty() {
                    return Err(format!("block {:?} has an empty beat expression", block.id));
                }
            }
        }
        Ok(())
    }
}

fn lane_with_clip(id: &str, label: &str, performer: Option<LanePerformer>) -> LaneSpec {
    LaneSpec {
        id: id.into(),
        label: label.into(),
        kind: LaneKind::Instrument,
        performer,
        blocks: vec![BlockSpec {
            id: format!("{id}_clip"),
            name: format!("{label} Clip"),
            start: "0".into(),
            length: "16".into(),
            content: BlockContent::Clip { clip: id.into() },
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easybeat_scene_validates() {
        SceneDocument::easybeat()
            .validate_basic()
            .expect("easybeat scene should validate");
    }
}

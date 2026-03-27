//! Canonical authored note clips stored under `clips/*.json`.

use serde::{Deserialize, Serialize};

/// One authored clip document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipDocument {
    /// Clip identity and editor hints.
    pub clip: ClipMeta,
    /// Authored note events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<ClipNote>,
}

/// Clip metadata shared across editors and automation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipMeta {
    /// Stable clip id within the package.
    pub id: String,
    /// Human-visible label.
    pub name: String,
    /// Loop length in beats.
    pub length_beats: String,
    /// Preferred editor / semantic hint.
    pub kind: ClipKind,
}

/// Editor hint for a clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    PianoRoll,
    DrumPattern,
    Custom,
}

/// One MIDI-like note event on the clip timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipNote {
    /// MIDI note number (`36` kick, `48` C3, etc.).
    pub pitch: i16,
    /// Start position in beats.
    pub start: String,
    /// Note length in beats.
    pub length: String,
    /// MIDI-style velocity.
    pub velocity: u8,
}

impl ClipDocument {
    /// Structural validation for clip ids and note timing fields.
    pub fn validate_basic(&self) -> Result<(), String> {
        if self.clip.id.trim().is_empty() {
            return Err("clip.id must not be empty".into());
        }
        if self.clip.name.trim().is_empty() {
            return Err("clip.name must not be empty".into());
        }
        if self.clip.length_beats.trim().is_empty() {
            return Err("clip.length_beats must not be empty".into());
        }
        for (idx, note) in self.notes.iter().enumerate() {
            if note.start.trim().is_empty() || note.length.trim().is_empty() {
                return Err(format!("note #{idx} has an empty beat expression"));
            }
            if note.velocity == 0 {
                return Err(format!("note #{idx} must have velocity > 0"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_validation_rejects_zero_velocity() {
        let clip = ClipDocument {
            clip: ClipMeta {
                id: "bass".into(),
                name: "Bass".into(),
                length_beats: "16".into(),
                kind: ClipKind::PianoRoll,
            },
            notes: vec![ClipNote {
                pitch: 36,
                start: "0".into(),
                length: "1".into(),
                velocity: 0,
            }],
        };
        let err = clip.validate_basic().expect_err("zero velocity");
        assert!(err.contains("velocity"));
    }
}

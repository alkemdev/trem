//! # Rung clip interchange
//!
//! JSON-serializable [`Clip`] for **time × class** note data: rational beats, integer class
//! rows, voice lanes, and float metadata pairs. Enable the crate **`rung`** feature for this
//! module. Optional **[`midi`]** import (feature **`midi`**) maps Standard MIDI Files into this
//! representation (see [`midi::MidiImportOptions`]).
//!
//! Specification narrative: `flow/prop/piano-roll-editor-model.md` in the trem repo.

mod beat_time;
#[cfg(feature = "midi")]
pub mod midi;

pub use beat_time::BeatTime;

use serde::{Deserialize, Serialize};

/// Wrapper for on-disk / wire interchange (versioned).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RungFile {
    /// Must be `"rung"`.
    pub format: String,
    pub schema_version: u32,
    pub clip: Clip,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

/// Optional audit trail (import source, mapping id, etc.).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<String>,
}

/// A sequence of notes in beat time.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    /// Notes (order not significant for playback; sort by `t_on` when rendering).
    pub notes: Vec<ClipNote>,
    /// Optional loop / export horizon in beats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_beats: Option<BeatTime>,
}

/// One sounded note: an interval in **beats** on one **class** row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipNote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// Vertical grid row; meaning is defined by the host ladder (see spec).
    pub class: i32,
    pub t_on: BeatTime,
    pub t_off: BeatTime,
    pub voice: u32,
    /// 0.0 ..= 1.0 in this interchange layer.
    pub velocity: f64,
    #[serde(default, skip_serializing_if = "NoteMeta::is_empty")]
    pub meta: NoteMeta,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteMeta {
    /// `(param_id, value)` pairs; duplicates last-wins on normalize (see [`NoteMeta::normalize`]).
    pub pairs: Vec<(u32, f64)>,
}

impl NoteMeta {
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Last occurrence of each `u32` key wins.
    pub fn normalize(&mut self) {
        let mut map = std::collections::HashMap::new();
        for (k, v) in self.pairs.drain(..) {
            map.insert(k, v);
        }
        self.pairs = map.into_iter().collect();
        self.pairs.sort_by_key(|(k, _)| *k);
    }
}

/// Errors from Rung JSON validation, serialization, or MIDI import.
#[derive(Debug, thiserror::Error)]
pub enum RungError {
    #[error("invalid rung file: {0}")]
    InvalidFile(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(feature = "midi")]
    #[error("MIDI parse error: {0}")]
    Midi(String),
}

impl RungFile {
    pub const FORMAT: &'static str = "rung";
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn new(clip: Clip) -> Self {
        Self {
            format: Self::FORMAT.to_string(),
            schema_version: Self::SCHEMA_VERSION,
            clip,
            provenance: None,
        }
    }

    pub fn from_json(s: &str) -> Result<Self, RungError> {
        let f: RungFile = serde_json::from_str(s)?;
        f.validate()?;
        Ok(f)
    }

    pub fn to_json_pretty(&self) -> Result<String, RungError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn validate(&self) -> Result<(), RungError> {
        if self.format != Self::FORMAT {
            return Err(RungError::InvalidFile(format!(
                "expected format {:?}, got {:?}",
                Self::FORMAT,
                self.format
            )));
        }
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(RungError::InvalidFile(format!(
                "unsupported schema_version {} (supported: {})",
                self.schema_version,
                Self::SCHEMA_VERSION
            )));
        }
        for n in &self.clip.notes {
            if n.t_off <= n.t_on {
                return Err(RungError::InvalidFile(format!(
                    "note id {:?}: t_off must be > t_on",
                    n.id
                )));
            }
            if !(n.velocity.is_finite() && (0.0..=1.0).contains(&n.velocity)) {
                return Err(RungError::InvalidFile(format!(
                    "note id {:?}: velocity must be in [0,1]",
                    n.id
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_rational::Rational64;

    #[test]
    fn roundtrip_json() {
        let clip = Clip {
            notes: vec![ClipNote {
                id: Some(1),
                class: 60,
                t_on: BeatTime(Rational64::new(0, 1)),
                t_off: BeatTime(Rational64::new(1, 4)),
                voice: 0,
                velocity: 0.8,
                meta: NoteMeta {
                    pairs: vec![(7, 0.5)],
                },
            }],
            length_beats: Some(BeatTime(Rational64::new(4, 1))),
        };
        let file = RungFile::new(clip);
        let s = file.to_json_pretty().unwrap();
        let back = RungFile::from_json(&s).unwrap();
        assert_eq!(back, file);
    }
}

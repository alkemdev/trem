//! Composition-layer note data and runtime graph events with sample timing.
//!
//! [`NoteEvent`] uses scale degrees; [`GraphEvent`] is what processors see after pitch resolution.

use crate::math::Rational;

/// A note event in the composition layer.
///
/// Uses integer degree (into the active scale) rather than Hz or MIDI numbers.
/// The scale resolves degree → Pitch → Hz at render time.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoteEvent {
    /// Scale degree (0-indexed). Can be negative for degrees below the reference.
    pub degree: i32,
    /// Octave offset applied after scale resolution.
    pub octave: i32,
    /// Velocity as a rational in [0, 1]. Exact.
    pub velocity: Rational,
    /// Gate length as a fraction of the step duration (0, 1].
    /// `7/8` is default (legato-ish), `1/4` is staccato, `1/1` is full-length tie.
    pub gate: Rational,
    /// Arbitrary parameter overrides (keyed by param id).
    pub params: Vec<(u32, f64)>,
}

impl NoteEvent {
    /// Full note with degree, octave shift, and velocity; gate defaults to `7/8`.
    pub fn new(degree: i32, octave: i32, velocity: Rational) -> Self {
        Self {
            degree,
            octave,
            velocity,
            gate: Rational::new(7, 8),
            params: Vec::new(),
        }
    }

    /// Default octave `0`, velocity `3/4`, gate `7/8`.
    pub fn simple(degree: i32) -> Self {
        Self::new(degree, 0, Rational::new(3, 4))
    }

    /// Sets a custom gate length; chainable.
    pub fn with_gate(mut self, gate: Rational) -> Self {
        self.gate = gate;
        self
    }

    /// Appends a `(param_id, value)` override; chainable for multiple params.
    pub fn with_param(mut self, id: u32, value: f64) -> Self {
        self.params.push((id, value));
        self
    }
}

/// Events that flow through the audio graph at render time.
/// These are the result of resolving NoteEvents against a Scale.
#[derive(Clone, Debug)]
pub enum GraphEvent {
    /// Start a note: resolved Hz, linear velocity, and a voice slot for matching `NoteOff`.
    NoteOn {
        frequency: f64,
        velocity: f64,
        voice: u32,
    },
    /// Release the given `voice`; must pair with an earlier `NoteOn` for that voice.
    NoteOff { voice: u32 },
    /// Automation to `node`/`param` at this time (e.g. from `NoteEvent` param overrides).
    Param { node: u32, param: u32, value: f64 },
}

/// A [`GraphEvent`] scheduled at `sample_offset` from the start of the current process block (or render segment).
#[derive(Clone, Debug)]
pub struct TimedEvent {
    /// Sample index within the block where `event` should fire.
    pub sample_offset: usize,
    /// The payload delivered to the graph at that offset.
    pub event: GraphEvent,
}

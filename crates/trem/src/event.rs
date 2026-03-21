use crate::math::Rational;

/// A note event in the composition layer.
///
/// Uses integer degree (into the active scale) rather than Hz or MIDI numbers.
/// The scale resolves degree → Pitch → Hz at render time.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteEvent {
    /// Scale degree (0-indexed). Can be negative for degrees below the reference.
    pub degree: i32,
    /// Octave offset applied after scale resolution.
    pub octave: i32,
    /// Velocity as a rational in [0, 1]. Exact.
    pub velocity: Rational,
    /// Arbitrary parameter overrides (keyed by param id).
    pub params: Vec<(u32, f64)>,
}

impl NoteEvent {
    pub fn new(degree: i32, octave: i32, velocity: Rational) -> Self {
        Self {
            degree,
            octave,
            velocity,
            params: Vec::new(),
        }
    }

    pub fn simple(degree: i32) -> Self {
        Self::new(degree, 0, Rational::new(3, 4))
    }

    pub fn with_param(mut self, id: u32, value: f64) -> Self {
        self.params.push((id, value));
        self
    }
}

/// Events that flow through the audio graph at render time.
/// These are the result of resolving NoteEvents against a Scale.
#[derive(Clone, Debug)]
pub enum GraphEvent {
    NoteOn {
        frequency: f64,
        velocity: f64,
        voice: u32,
    },
    NoteOff {
        voice: u32,
    },
    Param {
        node: u32,
        param: u32,
        value: f64,
    },
}

/// A GraphEvent with a sample-accurate timestamp.
#[derive(Clone, Debug)]
pub struct TimedEvent {
    pub sample_offset: usize,
    pub event: GraphEvent,
}

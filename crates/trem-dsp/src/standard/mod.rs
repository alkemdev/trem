//! Standard library of [`Node`](trem::graph::Node) implementations for the trem graph:
//! oscillators, envelopes, dynamics, mixing, filters, noise, drum voices, effects, EQ, and
//! composable nested voices. See also [`crate::interfaces`] for the types used to implement nodes.

pub mod delay;
pub mod distort;
pub mod drums;
pub mod duck;
pub mod dynamics;
pub mod env;
pub mod eq;
pub mod filter;
pub mod gain;
pub mod graphic_eq;
pub mod lfo;
pub mod mix;
pub mod noise;
pub mod osc;
pub mod reverb;
pub mod synth;
pub mod wavetable;

pub use delay::StereoDelay;
pub use distort::Distortion;
pub use drums::{HatSynth, KickSynth, SnareSynth};
pub use duck::SidechainDucker;
pub use dynamics::{Compressor, Limiter};
pub use env::Adsr;
pub use eq::ParametricEq;
pub use filter::{BiquadFilter, FilterType, ModulatedLowPass};
pub use gain::{Gain, MonoGain, StereoGain, StereoPan};
pub use graphic_eq::GraphicEq;
pub use lfo::Lfo;
pub use mix::{MonoCrossfade, StereoMixer};
pub use noise::Noise;
pub use osc::{Oscillator, Waveform};
pub use reverb::PlateReverb;
pub use synth::{analog_voice, lead_voice};
pub use wavetable::Wavetable;

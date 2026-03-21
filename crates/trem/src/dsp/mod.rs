//! Audio DSP processors for the Trem graph: oscillators, envelopes, dynamics, mixing,
//! filters, noise, drum voices, time-based effects, EQ, and composable sub-graph voices.

pub mod delay;
pub mod drums;
pub mod env;
pub mod eq;
pub mod filter;
pub mod gain;
pub mod mix;
pub mod noise;
pub mod osc;
pub mod reverb;
pub mod subgraph;
pub mod synth;

pub use delay::StereoDelay;
pub use drums::{HatSynth, KickSynth, SnareSynth};
pub use env::Adsr;
pub use eq::ParametricEq;
pub use filter::{BiquadFilter, FilterType};
pub use gain::{Gain, MonoGain, StereoGain};
pub use mix::{MonoCrossfade, StereoMixer};
pub use noise::Noise;
pub use osc::{Oscillator, Waveform};
pub use reverb::PlateReverb;
pub use subgraph::SubGraph;
pub use synth::analog_voice;

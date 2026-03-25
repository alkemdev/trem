//! Built-in [`Node`](trem::graph::Node) implementations for [**trem**](https://docs.rs/trem).
//!
//! This crate splits the stock node library from the core engine:
//!
//! - [`interfaces`] — re-exports from `trem` for implementing [`Node`](trem::graph::Node) nodes.
//! - [`standard`] — oscillators, envelopes, effects, drums, nested composite graphs, etc.
//!
//! ## Registry
//!
//! Fill a [`trem::registry::Registry`] with all built-in tags:
//!
//! ```
//! use trem::registry::Registry;
//! use trem_dsp::register_standard;
//!
//! let mut reg = Registry::new();
//! register_standard(&mut reg);
//! assert!(reg.get("osc").is_some());
//! ```
//!
//! Or use [`standard_registry`] for an empty registry plus one call.

#[cfg(feature = "export")]
pub mod export;

pub mod interfaces;
pub mod standard;

pub use standard::*;

use trem::registry::{Category, Registry};

/// Registers every built-in node tag on `reg` (see [`Registry::register`](trem::registry::Registry::register)).
pub fn register_standard(reg: &mut Registry) {
    use crate::standard::{
        analog_voice, lead_voice, Adsr, BiquadFilter, Compressor, Distortion, FilterType,
        GraphicEq, HatSynth, KickSynth, Lfo, Limiter, MonoCrossfade, MonoGain, Noise, Oscillator,
        ParametricEq, PlateReverb, SidechainDucker, SnareSynth, StereoDelay, StereoGain,
        StereoMixer, StereoPan, Waveform, Wavetable,
    };

    reg.register("osc", "Oscillator", Category::Source, || {
        Box::new(Oscillator::new(Waveform::Sine))
    });
    reg.register("noi", "Noise", Category::Source, || Box::new(Noise::new()));
    reg.register("kick", "Kick Drum", Category::Source, || {
        Box::new(KickSynth::new(0))
    });
    reg.register("snr", "Snare Drum", Category::Source, || {
        Box::new(SnareSynth::new(0))
    });
    reg.register("hat", "Hi-Hat", Category::Source, || {
        Box::new(HatSynth::new(0))
    });

    reg.register("syn", "Analog Voice", Category::Source, || {
        Box::new(analog_voice(0, 64))
    });
    reg.register("ldv", "Lead Voice", Category::Source, || {
        Box::new(lead_voice(0, 64))
    });

    reg.register("dly", "Stereo Delay", Category::Effect, || {
        Box::new(StereoDelay::new(250.0, 0.4, 0.3))
    });
    reg.register("dst", "Distortion", Category::Effect, || {
        Box::new(Distortion::new())
    });
    reg.register("vrb", "Plate Reverb", Category::Effect, || {
        Box::new(PlateReverb::new(0.5, 0.5, 0.3))
    });
    reg.register("peq", "Parametric EQ", Category::Effect, || {
        Box::new(ParametricEq::new())
    });

    reg.register("lpf", "Low-Pass Filter", Category::Filter, || {
        Box::new(BiquadFilter::new(FilterType::LowPass, 1000.0, 0.707))
    });
    reg.register("hpf", "High-Pass Filter", Category::Filter, || {
        Box::new(BiquadFilter::new(FilterType::HighPass, 200.0, 0.707))
    });
    reg.register("bpf", "Band-Pass Filter", Category::Filter, || {
        Box::new(BiquadFilter::new(FilterType::BandPass, 1000.0, 1.0))
    });

    reg.register("env", "ADSR Envelope", Category::Modulator, || {
        Box::new(Adsr::new(0.01, 0.1, 0.7, 0.3))
    });

    reg.register("geq", "Graphic EQ", Category::Filter, || {
        Box::new(GraphicEq::new())
    });

    reg.register("wav", "Wavetable", Category::Source, || {
        Box::new(Wavetable::new())
    });

    reg.register("lfo", "LFO", Category::Modulator, || {
        Box::new(Lfo::new(1.0))
    });

    reg.register("lim", "Limiter", Category::Dynamics, || {
        Box::new(Limiter::new(-0.3, 100.0))
    });
    reg.register("com", "Compressor", Category::Dynamics, || {
        Box::new(Compressor::new(-18.0, 4.0, 10.0, 150.0))
    });
    reg.register("duk", "Sidechain Duck", Category::Dynamics, || {
        Box::new(SidechainDucker::new(0.85, 1.0, 120.0))
    });

    reg.register("vol", "Volume", Category::Utility, || {
        Box::new(StereoGain::new(1.0))
    });
    reg.register("pan", "Stereo Pan", Category::Utility, || {
        Box::new(StereoPan::new(0.0))
    });
    reg.register("gain", "Mono Gain", Category::Utility, || {
        Box::new(MonoGain::new(1.0))
    });
    reg.register("mix", "Stereo Mixer", Category::Mixer, || {
        Box::new(StereoMixer::new(2))
    });
    reg.register("xfade", "Mono Crossfade", Category::Mixer, || {
        Box::new(MonoCrossfade::new(0.5))
    });
}

/// Shorthand: [`Registry::new`](trem::registry::Registry::new) then [`register_standard`].
pub fn standard_registry() -> Registry {
    let mut r = Registry::new();
    register_standard(&mut r);
    r
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn standard_registry_has_all_tags() {
        let reg = standard_registry();
        let expected = [
            "osc", "noi", "kick", "snr", "hat", "syn", "ldv", "dly", "dst", "vrb", "peq", "geq",
            "lpf", "hpf", "bpf", "env", "wav", "lfo", "lim", "com", "duk", "vol", "pan", "gain",
            "mix", "xfade",
        ];
        for tag in &expected {
            assert!(reg.get(tag).is_some(), "missing tag: {tag}");
        }
        assert_eq!(reg.tags().len(), expected.len());
    }

    #[test]
    fn create_returns_node() {
        let reg = standard_registry();
        let node = reg.create("osc").unwrap();
        assert!(!node.info().name.is_empty());
        assert!(node.info().sig.is_source());
    }

    #[test]
    fn create_unknown_returns_none() {
        let reg = standard_registry();
        assert!(reg.create("xyz").is_none());
    }

    #[test]
    fn categories_are_populated() {
        let reg = standard_registry();
        assert!(!reg.tags_in(Category::Source).is_empty());
        assert!(!reg.tags_in(Category::Effect).is_empty());
        assert!(!reg.tags_in(Category::Filter).is_empty());
        assert!(!reg.tags_in(Category::Utility).is_empty());
        assert!(!reg.tags_in(Category::Mixer).is_empty());
        assert!(!reg.tags_in(Category::Modulator).is_empty());
    }

    #[test]
    fn all_registered_nodes_construct_without_panic() {
        let reg = standard_registry();
        for tag in reg.tags() {
            let node = reg.create(tag).unwrap();
            let info = node.info();
            assert!(!info.name.is_empty(), "empty name for tag: {tag}");
            assert!(
                info.sig.outputs > 0 || info.sig.inputs > 0,
                "zero I/O for {tag}"
            );
        }
    }

    #[test]
    fn created_node_params_are_valid() {
        let reg = standard_registry();
        for tag in reg.tags() {
            let node = reg.create(tag).unwrap();
            for p in node.params() {
                assert!(p.min <= p.default, "{tag}/{}: min > default", p.name);
                assert!(p.default <= p.max, "{tag}/{}: default > max", p.name);
            }
        }
    }
}

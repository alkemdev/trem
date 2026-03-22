//! Processor registry: create processors by short tag at runtime.
//!
//! The [`Registry`] maps 3-letter tags (e.g. `"osc"`, `"dly"`, `"vrb"`) to factory
//! functions that produce [`Processor`] trait objects. A standard library ships via
//! [`Registry::standard()`] covering all built-in DSP processors.

use crate::graph::Processor;
use std::collections::HashMap;

/// Category classification for grouping processors in UIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Sound generators: oscillators, noise, drum voices, composite synths.
    Source,
    /// Time-based effects: delay, reverb, EQ.
    Effect,
    /// Summing and crossfade stages.
    Mixer,
    /// Compressors, limiters, gates.
    Dynamics,
    /// Frequency-domain processors: biquad, graphic EQ.
    Filter,
    /// Control-rate generators: envelopes, LFOs.
    Modulator,
    /// Gain, panning, and other routing utilities.
    Utility,
}

impl Category {
    /// Human-readable name for display in menus and headings.
    pub fn label(self) -> &'static str {
        match self {
            Category::Source => "Source",
            Category::Effect => "Effect",
            Category::Mixer => "Mixer",
            Category::Dynamics => "Dynamics",
            Category::Filter => "Filter",
            Category::Modulator => "Modulator",
            Category::Utility => "Utility",
        }
    }
}

/// Metadata for a registered processor type.
pub struct ProcessorEntry {
    /// Short mnemonic used as the lookup key (e.g. `"osc"`, `"dly"`).
    pub tag: &'static str,
    /// Full human-readable name (e.g. `"Oscillator"`, `"Stereo Delay"`).
    pub name: &'static str,
    /// Grouping category for UI display.
    pub category: Category,
    factory: Box<dyn Fn() -> Box<dyn Processor> + Send + Sync>,
}

impl ProcessorEntry {
    /// Instantiate a new default processor from this entry's factory.
    pub fn create(&self) -> Box<dyn Processor> {
        (self.factory)()
    }
}

/// Runtime registry mapping tags to processor factories.
///
/// Use [`Registry::standard()`] for all built-in processors, or
/// [`Registry::new()`] + [`Registry::register()`] to build a custom set.
pub struct Registry {
    entries: HashMap<&'static str, ProcessorEntry>,
    by_category: HashMap<Category, Vec<&'static str>>,
}

impl Registry {
    /// Empty registry with no entries.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Register a processor factory under a tag.
    pub fn register<F>(&mut self, tag: &'static str, name: &'static str, category: Category, f: F)
    where
        F: Fn() -> Box<dyn Processor> + Send + Sync + 'static,
    {
        self.by_category.entry(category).or_default().push(tag);
        self.entries.insert(
            tag,
            ProcessorEntry {
                tag,
                name,
                category,
                factory: Box::new(f),
            },
        );
    }

    /// Create a processor by tag. Returns `None` if the tag is unknown.
    pub fn create(&self, tag: &str) -> Option<Box<dyn Processor>> {
        self.entries.get(tag).map(|e| e.create())
    }

    /// Look up entry metadata by tag.
    pub fn get(&self, tag: &str) -> Option<&ProcessorEntry> {
        self.entries.get(tag)
    }

    /// All registered tags.
    pub fn tags(&self) -> Vec<&'static str> {
        let mut tags: Vec<_> = self.entries.keys().copied().collect();
        tags.sort();
        tags
    }

    /// Tags belonging to a category.
    pub fn tags_in(&self, category: Category) -> &[&'static str] {
        self.by_category
            .get(&category)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// All distinct categories that have at least one entry.
    pub fn categories(&self) -> Vec<Category> {
        let mut cats: Vec<_> = self.by_category.keys().copied().collect();
        cats.sort_by_key(|c| c.label());
        cats
    }

    /// Build the standard library registry with all built-in processors.
    pub fn standard() -> Self {
        use crate::dsp;
        let mut r = Self::new();

        // Sources
        r.register("osc", "Oscillator", Category::Source, || {
            Box::new(dsp::Oscillator::new(dsp::Waveform::Sine))
        });
        r.register("noi", "Noise", Category::Source, || {
            Box::new(dsp::Noise::new())
        });
        r.register("kick", "Kick Drum", Category::Source, || {
            Box::new(dsp::KickSynth::new(0))
        });
        r.register("snr", "Snare Drum", Category::Source, || {
            Box::new(dsp::SnareSynth::new(0))
        });
        r.register("hat", "Hi-Hat", Category::Source, || {
            Box::new(dsp::HatSynth::new(0))
        });

        // Synth voice (composite graph)
        r.register("syn", "Analog Voice", Category::Source, || {
            Box::new(dsp::analog_voice(0, 64))
        });
        r.register("ldv", "Lead Voice", Category::Source, || {
            Box::new(dsp::lead_voice(0, 64))
        });

        // Effects
        r.register("dly", "Stereo Delay", Category::Effect, || {
            Box::new(dsp::StereoDelay::new(250.0, 0.4, 0.3))
        });
        r.register("dst", "Distortion", Category::Effect, || {
            Box::new(dsp::Distortion::new())
        });
        r.register("vrb", "Plate Reverb", Category::Effect, || {
            Box::new(dsp::PlateReverb::new(0.5, 0.5, 0.3))
        });
        r.register("peq", "Parametric EQ", Category::Effect, || {
            Box::new(dsp::ParametricEq::new())
        });

        // Filters
        r.register("lpf", "Low-Pass Filter", Category::Filter, || {
            Box::new(dsp::BiquadFilter::new(
                dsp::FilterType::LowPass,
                1000.0,
                0.707,
            ))
        });
        r.register("hpf", "High-Pass Filter", Category::Filter, || {
            Box::new(dsp::BiquadFilter::new(
                dsp::FilterType::HighPass,
                200.0,
                0.707,
            ))
        });
        r.register("bpf", "Band-Pass Filter", Category::Filter, || {
            Box::new(dsp::BiquadFilter::new(
                dsp::FilterType::BandPass,
                1000.0,
                1.0,
            ))
        });

        // Envelope
        r.register("env", "ADSR Envelope", Category::Modulator, || {
            Box::new(dsp::Adsr::new(0.01, 0.1, 0.7, 0.3))
        });

        // Graphic EQ
        r.register("geq", "Graphic EQ", Category::Filter, || {
            Box::new(dsp::GraphicEq::new())
        });

        // Wavetable oscillator
        r.register("wav", "Wavetable", Category::Source, || {
            Box::new(dsp::Wavetable::new())
        });

        // LFO
        r.register("lfo", "LFO", Category::Modulator, || {
            Box::new(dsp::Lfo::new(1.0))
        });

        // Dynamics
        r.register("lim", "Limiter", Category::Dynamics, || {
            Box::new(dsp::Limiter::new(-0.3, 100.0))
        });
        r.register("com", "Compressor", Category::Dynamics, || {
            Box::new(dsp::Compressor::new(-18.0, 4.0, 10.0, 150.0))
        });

        // Mixers / Utility
        r.register("vol", "Volume", Category::Utility, || {
            Box::new(dsp::StereoGain::new(1.0))
        });
        r.register("pan", "Stereo Pan", Category::Utility, || {
            Box::new(dsp::StereoPan::new(0.0))
        });
        r.register("gain", "Mono Gain", Category::Utility, || {
            Box::new(dsp::MonoGain::new(1.0))
        });
        r.register("mix", "Stereo Mixer", Category::Mixer, || {
            Box::new(dsp::StereoMixer::new(2))
        });
        r.register("xfade", "Mono Crossfade", Category::Mixer, || {
            Box::new(dsp::MonoCrossfade::new(0.5))
        });

        r
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_registry_has_all_tags() {
        let reg = Registry::standard();
        let expected = [
            "osc", "noi", "kick", "snr", "hat", "syn", "ldv", "dly", "dst", "vrb", "peq", "geq",
            "lpf", "hpf", "bpf", "env", "wav", "lfo", "lim", "com", "vol", "pan", "gain", "mix",
            "xfade",
        ];
        for tag in &expected {
            assert!(reg.get(tag).is_some(), "missing tag: {tag}");
        }
        assert_eq!(reg.tags().len(), expected.len());
    }

    #[test]
    fn create_returns_processor() {
        let reg = Registry::standard();
        let osc = reg.create("osc").unwrap();
        assert!(!osc.info().name.is_empty());
        assert!(osc.info().sig.is_source());
    }

    #[test]
    fn create_unknown_returns_none() {
        let reg = Registry::standard();
        assert!(reg.create("xyz").is_none());
    }

    #[test]
    fn categories_are_populated() {
        let reg = Registry::standard();
        assert!(!reg.tags_in(Category::Source).is_empty());
        assert!(!reg.tags_in(Category::Effect).is_empty());
        assert!(!reg.tags_in(Category::Filter).is_empty());
        assert!(!reg.tags_in(Category::Utility).is_empty());
        assert!(!reg.tags_in(Category::Mixer).is_empty());
        assert!(!reg.tags_in(Category::Modulator).is_empty());
    }

    #[test]
    fn all_processors_construct_without_panic() {
        let reg = Registry::standard();
        for tag in reg.tags() {
            let proc = reg.create(tag).unwrap();
            let info = proc.info();
            assert!(!info.name.is_empty(), "empty name for tag: {tag}");
            assert!(
                info.sig.outputs > 0 || info.sig.inputs > 0,
                "zero I/O for {tag}"
            );
        }
    }

    #[test]
    fn created_processor_params_are_valid() {
        let reg = Registry::standard();
        for tag in reg.tags() {
            let proc = reg.create(tag).unwrap();
            for p in proc.params() {
                assert!(p.min <= p.default, "{tag}/{}: min > default", p.name);
                assert!(p.default <= p.max, "{tag}/{}: default > max", p.name);
            }
        }
    }
}

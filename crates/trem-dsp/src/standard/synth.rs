//! Pre-wired synthesizer voices built as nested [`Graph`] nodes.
//!
//! Each factory function returns a self-contained voice that responds to
//! note events for a specific `voice_id` and exposes its key parameters
//! through the generic self-describing system.

use super::env::Adsr;
use super::filter::{BiquadFilter, FilterType, ModulatedLowPass};
use super::gain::MonoGain;
use super::mix::MonoCrossfade;
use super::osc::{Oscillator, Waveform};
use super::wavetable::Wavetable;
use trem::graph::{Graph, GroupHint, Node, ParamGroup};

/// Dual-oscillator analog-style voice with lowpass filter and ADSR envelope.
///
/// Internal signal chain:
/// ```text
/// Osc1 (saw) ──┐
///              ├── crossfade ── LP filter ── ADSR ── gain
/// Osc2 (sq)  ──┘
/// ```
///
/// Exposed parameters (IDs 0–8):
///
/// | ID | Label      | Range            | Default |
/// |----|------------|------------------|---------|
/// | 0  | Detune     | −24 … +24 st     | 0.1     |
/// | 1  | Osc Mix    | 0 … 1            | 0.5     |
/// | 2  | Cutoff     | 20 … 20 000 Hz   | 2000    |
/// | 3  | Resonance  | 0.1 … 20         | 1.5     |
/// | 4  | Attack     | 0.001 … 5 s      | 0.005   |
/// | 5  | Decay      | 0.001 … 5 s      | 0.2     |
/// | 6  | Sustain    | 0 … 1            | 0.6     |
/// | 7  | Release    | 0.001 … 5 s      | 0.3     |
/// | 8  | Level      | 0 … 2            | 0.5     |
pub fn analog_voice(voice_id: u32, block_size: usize) -> Graph {
    let mut g = Graph::labeled(block_size, "synth");

    let osc1 = g.add_node(Box::new(
        Oscillator::new(Waveform::Saw).with_voice(voice_id),
    ));

    let mut osc2_proc = Oscillator::new(Waveform::Square).with_voice(voice_id);
    osc2_proc.detune = 0.1;
    let osc2 = g.add_node(Box::new(osc2_proc));

    let xfade = g.add_node(Box::new(MonoCrossfade::new(0.5)));
    let filt = g.add_node(Box::new(BiquadFilter::new(
        FilterType::LowPass,
        2000.0,
        1.5,
    )));
    let env = g.add_node(Box::new(
        Adsr::new(0.005, 0.2, 0.6, 0.3).with_voice(voice_id),
    ));
    let gain = g.add_node(Box::new(MonoGain::new(0.5)));

    g.connect(osc1, 0, xfade, 0);
    g.connect(osc2, 0, xfade, 1);
    g.connect(xfade, 0, filt, 0);
    g.connect(filt, 0, env, 0);
    g.connect(env, 0, gain, 0);
    g.set_output(gain, 1);

    let g_osc = g.add_group(ParamGroup {
        id: 0,
        name: "Oscillator",
        hint: GroupHint::Oscillator,
    });
    let g_filt = g.add_group(ParamGroup {
        id: 0,
        name: "Filter",
        hint: GroupHint::Filter,
    });
    let g_env = g.add_group(ParamGroup {
        id: 0,
        name: "Envelope",
        hint: GroupHint::Envelope,
    });
    let g_out = g.add_group(ParamGroup {
        id: 0,
        name: "Output",
        hint: GroupHint::Level,
    });

    g.expose_param_in_group(osc2, 0, "Detune", g_osc);
    g.expose_param_in_group(xfade, 0, "Osc Mix", g_osc);
    g.expose_param_in_group(filt, 0, "Cutoff", g_filt);
    g.expose_param_in_group(filt, 1, "Resonance", g_filt);
    g.expose_param_in_group(env, 0, "Attack", g_env);
    g.expose_param_in_group(env, 1, "Decay", g_env);
    g.expose_param_in_group(env, 2, "Sustain", g_env);
    g.expose_param_in_group(env, 3, "Release", g_env);
    g.expose_param_in_group(gain, 0, "Level", g_out);

    g
}

/// Lead-style voice: dual oscillators, subtle wavetable air, modulated low-pass, ADSR.
///
/// ```text
/// Osc1 (saw) ──┐
///              ├── xfade ──┬── xfade ── mod LP ── ADSR ── gain
/// Osc2 (tri) ──┘           │
/// Wavetable ───────────────┘
/// ```
///
/// External parameter indices (after nesting in `instrument_channel`):
/// 0 Detune, 1 Osc Mix, 2 WT Mix, 3 WT Shape, 4 Cutoff, 5 Resonance,
/// 6 LFO Rate, 7 LFO Depth, 8–11 ADSR, 12 Level.
pub fn lead_voice(voice_id: u32, block_size: usize) -> Graph {
    let mut g = Graph::labeled(block_size, "lead_synth");

    let osc1 = g.add_node(Box::new(
        Oscillator::new(Waveform::Saw).with_voice(voice_id),
    ));

    let mut osc2_proc = Oscillator::new(Waveform::Triangle).with_voice(voice_id);
    osc2_proc.detune = 0.08;
    let osc2 = g.add_node(Box::new(osc2_proc));

    let osc_mix = g.add_node(Box::new(MonoCrossfade::new(0.52)));

    let mut wt = Wavetable::new();
    wt.set_param(0, 1.4);
    wt.set_param(1, -0.04);
    wt.set_param(2, 0.32);
    let wav = g.add_node(Box::new(wt));

    let wt_mix = g.add_node(Box::new(MonoCrossfade::new(0.88)));

    let filt = g.add_node(Box::new(ModulatedLowPass::new(2800.0, 1.65, 0.28, 520.0)));

    let env = g.add_node(Box::new(
        Adsr::new(0.004, 0.18, 0.55, 0.22).with_voice(voice_id),
    ));
    let gain = g.add_node(Box::new(MonoGain::new(0.72)));

    g.connect(osc1, 0, osc_mix, 0);
    g.connect(osc2, 0, osc_mix, 1);
    g.connect(osc_mix, 0, wt_mix, 0);
    g.connect(wav, 0, wt_mix, 1);
    g.connect(wt_mix, 0, filt, 0);
    g.connect(filt, 0, env, 0);
    g.connect(env, 0, gain, 0);
    g.set_output(gain, 1);

    let g_osc = g.add_group(ParamGroup {
        id: 0,
        name: "Oscillator",
        hint: GroupHint::Oscillator,
    });
    let g_filt = g.add_group(ParamGroup {
        id: 0,
        name: "Filter",
        hint: GroupHint::Filter,
    });
    let g_mod = g.add_group(ParamGroup {
        id: 0,
        name: "Modulation",
        hint: GroupHint::Generic,
    });
    let g_env = g.add_group(ParamGroup {
        id: 0,
        name: "Envelope",
        hint: GroupHint::Envelope,
    });
    let g_out = g.add_group(ParamGroup {
        id: 0,
        name: "Output",
        hint: GroupHint::Level,
    });

    g.expose_param_in_group(osc2, 0, "Detune", g_osc);
    g.expose_param_in_group(osc_mix, 0, "Osc Mix", g_osc);
    g.expose_param_in_group(wt_mix, 0, "WT Mix", g_osc);
    g.expose_param_in_group(wav, 0, "WT Shape", g_osc);

    g.expose_param_in_group(filt, 0, "Cutoff", g_filt);
    g.expose_param_in_group(filt, 1, "Resonance", g_filt);
    g.expose_param_in_group(filt, 2, "LFO Rate", g_mod);
    g.expose_param_in_group(filt, 3, "LFO Depth", g_mod);

    g.expose_param_in_group(env, 0, "Attack", g_env);
    g.expose_param_in_group(env, 1, "Decay", g_env);
    g.expose_param_in_group(env, 2, "Sustain", g_env);
    g.expose_param_in_group(env, 3, "Release", g_env);
    g.expose_param_in_group(gain, 0, "Level", g_out);

    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use trem::event::{GraphEvent, TimedEvent};
    use trem::graph::{Node, ProcessContext};

    #[test]
    fn lead_voice_responds_to_notes() {
        let mut synth = lead_voice(0, 256);
        assert_eq!(synth.info().sig.outputs, 1);
        assert_eq!(synth.params().len(), 13);

        let events = vec![TimedEvent {
            sample_offset: 0,
            event: GraphEvent::NoteOn {
                frequency: 440.0,
                velocity: 0.8,
                voice: 0,
            },
        }];

        let mut out = vec![vec![0.0f32; 256]];
        let inputs: Vec<&[f32]> = vec![];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut out,
            frames: 256,
            sample_rate: 44100.0,
            events: &events,
        };
        synth.process(&mut ctx);

        let peak = out[0].iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak > 0.01,
            "lead_voice should produce audible output, peak={peak}"
        );
    }

    #[test]
    fn analog_voice_responds_to_notes() {
        let mut synth = analog_voice(0, 256);
        assert_eq!(synth.info().sig.outputs, 1);
        assert_eq!(synth.params().len(), 9);

        let events = vec![TimedEvent {
            sample_offset: 0,
            event: GraphEvent::NoteOn {
                frequency: 440.0,
                velocity: 0.8,
                voice: 0,
            },
        }];

        let mut out = vec![vec![0.0f32; 256]];
        let inputs: Vec<&[f32]> = vec![];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut out,
            frames: 256,
            sample_rate: 44100.0,
            events: &events,
        };
        synth.process(&mut ctx);

        let peak = out[0].iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak > 0.01,
            "synth should produce audible output, peak={peak}"
        );
    }

    #[test]
    fn param_tweaking_changes_sound() {
        let mut synth = analog_voice(0, 512);

        let events = vec![TimedEvent {
            sample_offset: 0,
            event: GraphEvent::NoteOn {
                frequency: 440.0,
                velocity: 0.8,
                voice: 0,
            },
        }];

        let run = |s: &mut Graph, evts: &[TimedEvent]| -> f32 {
            s.reset();
            let mut out = vec![vec![0.0f32; 512]];
            let inputs: Vec<&[f32]> = vec![];
            let mut ctx = ProcessContext {
                inputs: &inputs,
                outputs: &mut out,
                frames: 512,
                sample_rate: 44100.0,
                events: evts,
            };
            s.process(&mut ctx);
            out[0].iter().map(|s| s * s).sum::<f32>()
        };

        let energy_default = run(&mut synth, &events);

        synth.set_param(2, 200.0);
        let energy_dark = run(&mut synth, &events);

        assert!(
            energy_dark < energy_default,
            "lower cutoff should reduce energy: {energy_dark} vs {energy_default}"
        );
    }
}

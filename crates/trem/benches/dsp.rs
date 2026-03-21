use divan::Bencher;
use trem::dsp::*;
use trem::event::{GraphEvent, TimedEvent};
use trem::graph::{ProcessContext, Processor};

fn main() {
    divan::main();
}

const BLOCK: usize = 512;
const SR: f64 = 44100.0;

fn tone_input(frames: usize) -> Vec<f32> {
    let mut buf = vec![0.0f32; frames];
    for i in 0..frames {
        buf[i] = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / SR).sin() as f32;
    }
    buf
}

fn note_on_event() -> Vec<TimedEvent> {
    vec![TimedEvent {
        sample_offset: 0,
        event: GraphEvent::NoteOn {
            frequency: 440.0,
            velocity: 0.8,
            voice: 0,
        },
    }]
}

fn run_source(proc: &mut dyn Processor, frames: usize) {
    let mut out = vec![vec![0.0f32; frames]];
    let inputs: Vec<&[f32]> = vec![];
    let mut ctx = ProcessContext {
        inputs: &inputs,
        outputs: &mut out,
        frames,
        sample_rate: SR,
        events: &[],
    };
    proc.process(&mut ctx);
}

fn run_mono_effect(proc: &mut dyn Processor, input: &[f32], frames: usize) {
    let input_refs: Vec<&[f32]> = vec![input];
    let mut out = vec![vec![0.0f32; frames]];
    let mut ctx = ProcessContext {
        inputs: &input_refs,
        outputs: &mut out,
        frames,
        sample_rate: SR,
        events: &[],
    };
    proc.process(&mut ctx);
}

fn run_stereo_effect(proc: &mut dyn Processor, input_l: &[f32], input_r: &[f32], frames: usize) {
    let input_refs: Vec<&[f32]> = vec![input_l, input_r];
    let mut out = vec![vec![0.0f32; frames], vec![0.0f32; frames]];
    let mut ctx = ProcessContext {
        inputs: &input_refs,
        outputs: &mut out,
        frames,
        sample_rate: SR,
        events: &[],
    };
    proc.process(&mut ctx);
}

fn prime_source(proc: &mut dyn Processor) {
    let events = note_on_event();
    let mut out = vec![vec![0.0f32; BLOCK]];
    let inputs: Vec<&[f32]> = vec![];
    let mut ctx = ProcessContext {
        inputs: &inputs,
        outputs: &mut out,
        frames: BLOCK,
        sample_rate: SR,
        events: &events,
    };
    proc.process(&mut ctx);
}

// ---------------------------------------------------------------------------
// Oscillators
// ---------------------------------------------------------------------------

mod oscillator {
    use super::*;

    #[divan::bench]
    fn sine(bencher: Bencher) {
        let mut osc = Oscillator::new(Waveform::Sine);
        prime_source(&mut osc);
        bencher.bench_local(|| run_source(&mut osc, BLOCK));
    }

    #[divan::bench]
    fn saw(bencher: Bencher) {
        let mut osc = Oscillator::new(Waveform::Saw);
        prime_source(&mut osc);
        bencher.bench_local(|| run_source(&mut osc, BLOCK));
    }

    #[divan::bench]
    fn square(bencher: Bencher) {
        let mut osc = Oscillator::new(Waveform::Square);
        prime_source(&mut osc);
        bencher.bench_local(|| run_source(&mut osc, BLOCK));
    }

    #[divan::bench]
    fn triangle(bencher: Bencher) {
        let mut osc = Oscillator::new(Waveform::Triangle);
        prime_source(&mut osc);
        bencher.bench_local(|| run_source(&mut osc, BLOCK));
    }
}

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

mod envelope {
    use super::*;

    #[divan::bench]
    fn adsr_active(bencher: Bencher) {
        let mut env = Adsr::new(0.01, 0.1, 0.5, 0.1);
        env.trigger();
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_mono_effect(&mut env, &input, BLOCK));
    }

    #[divan::bench]
    fn adsr_idle(bencher: Bencher) {
        let mut env = Adsr::new(0.01, 0.1, 0.5, 0.1);
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_mono_effect(&mut env, &input, BLOCK));
    }
}

// ---------------------------------------------------------------------------
// Filter
// ---------------------------------------------------------------------------

mod filter {
    use super::*;

    #[divan::bench]
    fn lowpass_1k(bencher: Bencher) {
        let mut f = BiquadFilter::new(FilterType::LowPass, 1000.0, 0.707);
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_mono_effect(&mut f, &input, BLOCK));
    }

    #[divan::bench]
    fn highpass_200(bencher: Bencher) {
        let mut f = BiquadFilter::new(FilterType::HighPass, 200.0, 0.707);
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_mono_effect(&mut f, &input, BLOCK));
    }

    #[divan::bench]
    fn bandpass_800(bencher: Bencher) {
        let mut f = BiquadFilter::new(FilterType::BandPass, 800.0, 2.0);
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_mono_effect(&mut f, &input, BLOCK));
    }
}

// ---------------------------------------------------------------------------
// Gain / Mix
// ---------------------------------------------------------------------------

mod gain_mix {
    use super::*;

    #[divan::bench]
    fn mono_gain(bencher: Bencher) {
        let mut g = MonoGain::new(0.5);
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_mono_effect(&mut g, &input, BLOCK));
    }

    #[divan::bench]
    fn stereo_mixer_4ch(bencher: Bencher) {
        let mut mixer = StereoMixer::new(4);
        let ch0 = tone_input(BLOCK);
        let ch1 = tone_input(BLOCK);
        let ch2 = vec![0.0f32; BLOCK];
        let ch3 = vec![0.0f32; BLOCK];
        let input_refs: Vec<&[f32]> = vec![&ch0, &ch1, &ch2, &ch3];
        let mut out = vec![vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]];
        bencher.bench_local(|| {
            out[0].fill(0.0);
            out[1].fill(0.0);
            let mut ctx = ProcessContext {
                inputs: &input_refs,
                outputs: &mut out,
                frames: BLOCK,
                sample_rate: SR,
                events: &[],
            };
            mixer.process(&mut ctx);
        });
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

mod effects {
    use super::*;

    #[divan::bench]
    fn delay_375ms(bencher: Bencher) {
        let mut d = StereoDelay::new(375.0, 0.4, 0.3);
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_stereo_effect(&mut d, &input, &input, BLOCK));
    }

    #[divan::bench]
    fn reverb_medium(bencher: Bencher) {
        let mut r = PlateReverb::new(0.6, 0.4, 0.35);
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_stereo_effect(&mut r, &input, &input, BLOCK));
    }

    #[divan::bench]
    fn eq_3band(bencher: Bencher) {
        let mut e = ParametricEq::new();
        let input = tone_input(BLOCK);
        bencher.bench_local(|| run_stereo_effect(&mut e, &input, &input, BLOCK));
    }
}

// ---------------------------------------------------------------------------
// Drum synths
// ---------------------------------------------------------------------------

mod drums {
    use super::*;

    #[divan::bench]
    fn kick(bencher: Bencher) {
        let mut k = KickSynth::new(0);
        prime_source(&mut k);
        bencher.bench_local(|| run_source(&mut k, BLOCK));
    }

    #[divan::bench]
    fn snare(bencher: Bencher) {
        let mut s = SnareSynth::new(1);
        prime_source(&mut s);
        bencher.bench_local(|| run_source(&mut s, BLOCK));
    }

    #[divan::bench]
    fn hat(bencher: Bencher) {
        let mut h = HatSynth::new(2);
        prime_source(&mut h);
        bencher.bench_local(|| run_source(&mut h, BLOCK));
    }
}

// ---------------------------------------------------------------------------
// SubGraph / analog_voice
// ---------------------------------------------------------------------------

mod subgraph {
    use super::*;

    #[divan::bench]
    fn analog_voice_block(bencher: Bencher) {
        let mut synth = analog_voice(0, BLOCK);
        prime_source(&mut synth);
        bencher.bench_local(|| run_source(&mut synth, BLOCK));
    }

    #[divan::bench]
    fn analog_voice_construct() {
        divan::black_box(analog_voice(0, BLOCK));
    }
}

// ---------------------------------------------------------------------------
// Block size scaling
// ---------------------------------------------------------------------------

mod block_sizes {
    use super::*;

    #[divan::bench(args = [64, 128, 256, 512, 1024, 2048])]
    fn oscillator_saw(bencher: Bencher, frames: usize) {
        let mut osc = Oscillator::new(Waveform::Saw);
        prime_source(&mut osc);
        bencher.bench_local(|| run_source(&mut osc, frames));
    }

    #[divan::bench(args = [64, 128, 256, 512, 1024, 2048])]
    fn reverb(bencher: Bencher, frames: usize) {
        let mut rev = PlateReverb::new(0.6, 0.4, 0.35);
        let input_l = tone_input(frames);
        let input_r = tone_input(frames);
        bencher.bench_local(|| run_stereo_effect(&mut rev, &input_l, &input_r, frames));
    }
}

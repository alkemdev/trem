use divan::Bencher;
use trem::dsp::*;
use trem::event::{GraphEvent, NoteEvent, TimedEvent};
use trem::graph::Graph;
use trem::math::Rational;
use trem::pitch::Tuning;
use trem::render;
use trem::tree::Tree;

fn main() {
    divan::main();
}

const BLOCK: usize = 512;
const SR: f64 = 44100.0;

fn note_on(offset: usize, freq: f64, voice: u32) -> TimedEvent {
    TimedEvent {
        sample_offset: offset,
        event: GraphEvent::NoteOn {
            frequency: freq,
            velocity: 0.8,
            voice,
        },
    }
}

// ---------------------------------------------------------------------------
// Graph processing
// ---------------------------------------------------------------------------

mod graph_process {
    use super::*;

    fn simple_chain() -> (Graph, u32) {
        let mut g = Graph::new(BLOCK);
        let osc = g.add_node(Box::new(Oscillator::new(Waveform::Saw)));
        let filt = g.add_node(Box::new(BiquadFilter::new(
            FilterType::LowPass,
            2000.0,
            1.0,
        )));
        let env = g.add_node(Box::new(Adsr::new(0.01, 0.1, 0.5, 0.1)));
        let gain = g.add_node(Box::new(Gain::new(0.5)));
        g.connect(osc, 0, filt, 0);
        g.connect(filt, 0, env, 0);
        g.connect(env, 0, gain, 0);
        (g, gain)
    }

    fn full_mix_graph() -> (Graph, u32) {
        let mut g = Graph::new(BLOCK);

        let lead_osc = g.add_node(Box::new(Oscillator::new(Waveform::Triangle).with_voice(0)));
        let lead_env = g.add_node(Box::new(Adsr::new(0.01, 0.1, 0.4, 0.1).with_voice(0)));
        let lead_gain = g.add_node(Box::new(Gain::new(0.5)));
        g.connect(lead_osc, 0, lead_env, 0);
        g.connect(lead_env, 0, lead_gain, 0);

        let bass_osc = g.add_node(Box::new(Oscillator::new(Waveform::Saw).with_voice(1)));
        let bass_filt = g.add_node(Box::new(BiquadFilter::new(FilterType::LowPass, 800.0, 1.5)));
        let bass_env = g.add_node(Box::new(Adsr::new(0.005, 0.2, 0.5, 0.08).with_voice(1)));
        let bass_gain = g.add_node(Box::new(Gain::new(0.3)));
        g.connect(bass_osc, 0, bass_filt, 0);
        g.connect(bass_filt, 0, bass_env, 0);
        g.connect(bass_env, 0, bass_gain, 0);

        let kick = g.add_node(Box::new(KickSynth::new(2)));
        let snare = g.add_node(Box::new(SnareSynth::new(3)));

        let mixer = g.add_node(Box::new(StereoMixer::new(8)));
        g.connect(lead_gain, 0, mixer, 0);
        g.connect(lead_gain, 0, mixer, 1);
        g.connect(bass_gain, 0, mixer, 2);
        g.connect(bass_gain, 0, mixer, 3);
        g.connect(kick, 0, mixer, 4);
        g.connect(kick, 0, mixer, 5);
        g.connect(snare, 0, mixer, 6);
        g.connect(snare, 0, mixer, 7);

        let delay = g.add_node(Box::new(StereoDelay::new(375.0, 0.3, 0.2)));
        g.connect(mixer, 0, delay, 0);
        g.connect(mixer, 1, delay, 1);

        let reverb = g.add_node(Box::new(PlateReverb::new(0.5, 0.4, 0.25)));
        g.connect(delay, 0, reverb, 0);
        g.connect(delay, 1, reverb, 1);

        let master = g.add_node(Box::new(StereoMixer::new(2)));
        g.connect(reverb, 0, master, 0);
        g.connect(reverb, 1, master, 1);

        (g, master)
    }

    #[divan::bench]
    fn simple_chain_no_events(bencher: Bencher) {
        let (mut g, _) = simple_chain();
        g.run(BLOCK, SR, &[note_on(0, 440.0, 0)]);
        bencher.bench_local(|| g.run(BLOCK, SR, &[]));
    }

    #[divan::bench]
    fn simple_chain_with_note(bencher: Bencher) {
        let (mut g, _) = simple_chain();
        let events = vec![note_on(0, 440.0, 0)];
        bencher.bench_local(|| g.run(BLOCK, SR, &events));
    }

    #[divan::bench]
    fn full_mix_no_events(bencher: Bencher) {
        let (mut g, _) = full_mix_graph();
        let startup = vec![note_on(0, 440.0, 0), note_on(0, 220.0, 1)];
        g.run(BLOCK, SR, &startup);
        bencher.bench_local(|| g.run(BLOCK, SR, &[]));
    }

    #[divan::bench]
    fn full_mix_with_notes(bencher: Bencher) {
        let (mut g, _) = full_mix_graph();
        let events = vec![
            note_on(0, 440.0, 0),
            note_on(64, 220.0, 1),
            note_on(128, 55.0, 2),
            note_on(256, 330.0, 3),
        ];
        bencher.bench_local(|| g.run(BLOCK, SR, &events));
    }

    #[divan::bench(args = [4, 8, 16, 32])]
    fn graph_node_scaling(bencher: Bencher, num_voices: usize) {
        let mut g = Graph::new(BLOCK);
        let mut gains = Vec::new();
        for i in 0..num_voices {
            let osc = g.add_node(Box::new(
                Oscillator::new(Waveform::Saw).with_voice(i as u32),
            ));
            let gain = g.add_node(Box::new(Gain::new(1.0 / num_voices as f32)));
            g.connect(osc, 0, gain, 0);
            gains.push(gain);
        }
        let mixer = g.add_node(Box::new(StereoMixer::new(num_voices as u16 * 2)));
        for (i, &gain) in gains.iter().enumerate() {
            g.connect(gain, 0, mixer, (i * 2) as u16);
            g.connect(gain, 0, mixer, (i * 2 + 1) as u16);
        }
        let events: Vec<TimedEvent> = (0..num_voices)
            .map(|i| note_on(0, 220.0 * (i + 1) as f64, i as u32))
            .collect();
        g.run(BLOCK, SR, &events);
        bencher.bench_local(|| g.run(BLOCK, SR, &[]));
    }
}

// ---------------------------------------------------------------------------
// Render pipeline
// ---------------------------------------------------------------------------

mod render_bench {
    use super::*;

    fn simple_graph() -> (Graph, u32) {
        let mut g = Graph::new(BLOCK);
        let osc = g.add_node(Box::new(Oscillator::new(Waveform::Saw)));
        let env = g.add_node(Box::new(Adsr::new(0.005, 0.05, 0.3, 0.1)));
        let gain = g.add_node(Box::new(Gain::new(0.5)));
        g.connect(osc, 0, env, 0);
        g.connect(env, 0, gain, 0);
        (g, gain)
    }

    #[divan::bench]
    fn tree_to_timed_events(bencher: Bencher) {
        let scale = Tuning::edo12().to_scale();
        let tree = Tree::seq(vec![
            Tree::leaf(NoteEvent::simple(0)),
            Tree::rest(),
            Tree::leaf(NoteEvent::simple(4)),
            Tree::rest(),
        ]);
        bencher.bench(|| {
            render::tree_to_timed_events(&tree, Rational::integer(4), 120.0, SR, &scale, 440.0)
        });
    }

    #[divan::bench]
    fn grid_to_timed_events(bencher: Bencher) {
        let scale = Tuning::edo12().to_scale();
        let mut grid = trem::grid::Grid::new(16, 5);
        for r in (0..16).step_by(4) {
            grid.set(r, 0, Some(NoteEvent::simple(0)));
        }
        for r in (2..16).step_by(4) {
            grid.set(r, 1, Some(NoteEvent::simple(4)));
        }
        let voice_ids = vec![0, 1, 2, 3, 4];
        bencher.bench(|| {
            render::grid_to_timed_events(
                &grid,
                Rational::integer(4),
                130.0,
                SR,
                &scale,
                440.0,
                &voice_ids,
                0.0,
            )
        });
    }

    #[divan::bench]
    fn offline_render_1_beat(bencher: Bencher) {
        let scale = Tuning::edo12().to_scale();
        let tree = Tree::seq(vec![
            Tree::leaf(NoteEvent::simple(0)),
            Tree::rest(),
            Tree::leaf(NoteEvent::simple(4)),
            Tree::rest(),
        ]);
        bencher.bench_local(|| {
            let (mut g, out) = simple_graph();
            render::render_pattern(
                &tree,
                Rational::integer(1),
                120.0,
                SR,
                &scale,
                440.0,
                &mut g,
                out,
            )
        });
    }

    #[divan::bench]
    fn offline_render_4_beats(bencher: Bencher) {
        let scale = Tuning::edo12().to_scale();
        let tree = Tree::seq(vec![
            Tree::leaf(NoteEvent::simple(0)),
            Tree::rest(),
            Tree::leaf(NoteEvent::simple(4)),
            Tree::rest(),
        ]);
        bencher.bench_local(|| {
            let (mut g, out) = simple_graph();
            render::render_pattern(
                &tree,
                Rational::integer(4),
                120.0,
                SR,
                &scale,
                440.0,
                &mut g,
                out,
            )
        });
    }
}

// ---------------------------------------------------------------------------
// Topology rebuild
// ---------------------------------------------------------------------------

mod topology {
    use super::*;

    #[divan::bench(args = [4, 16, 64])]
    fn rebuild_graph(bencher: Bencher, nodes: usize) {
        bencher.bench_local(|| {
            let mut g = Graph::new(BLOCK);
            let mut prev = g.add_node(Box::new(Oscillator::new(Waveform::Saw)));
            for _ in 1..nodes {
                let next = g.add_node(Box::new(Gain::new(0.9)));
                g.connect(prev, 0, next, 0);
                prev = next;
            }
            g.run(1, SR, &[]);
        });
    }
}

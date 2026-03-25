//! Offline rendering: build a synth graph and render a pattern to sample buffers.
//!
//! Run with: `cargo run -p trem-dsp --example offline_render`

use trem::event::NoteEvent;
use trem::graph::Graph;
use trem::math::Rational;
use trem::pitch::Tuning;
use trem::tree::Tree;
use trem_dsp::{Adsr, Gain, Oscillator, Waveform};

fn main() {
    let mut graph = Graph::new(512);
    let osc = graph.add_node(Box::new(Oscillator::new(Waveform::Saw)));
    let env = graph.add_node(Box::new(Adsr::new(0.005, 0.1, 0.4, 0.2)));
    let gain = graph.add_node(Box::new(Gain::new(0.5)));
    graph.connect(osc, 0, env, 0);
    graph.connect(env, 0, gain, 0);

    let scale = Tuning::edo12().to_scale();

    let tree = Tree::seq(vec![
        Tree::leaf(NoteEvent::simple(0)),
        Tree::leaf(NoteEvent::simple(4)),
        Tree::leaf(NoteEvent::simple(7)),
        Tree::leaf(NoteEvent::new(0, 1, Rational::new(3, 4))),
    ]);

    let bpm = 120.0;
    let beats = Rational::integer(4);
    let sample_rate = 44100.0;

    let audio = trem::render::render_pattern(
        &tree,
        beats,
        bpm,
        sample_rate,
        &scale,
        440.0,
        &mut graph,
        gain,
    )
    .expect("render");

    let peak_l: f32 = audio[0].iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let peak_r: f32 = audio[1].iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let duration_s = audio[0].len() as f64 / sample_rate;

    println!(
        "Rendered {:.2}s of audio ({} samples per channel)",
        duration_s,
        audio[0].len()
    );
    println!("Peak L: {:.4}  Peak R: {:.4}", peak_l, peak_r);
    println!("Channels: {}", audio.len());
}

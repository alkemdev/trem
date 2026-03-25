//! Integration test: [`trem::render::render_pattern`] with stock DSP nodes (lives here so `trem`
//! does not dev-depend on `trem-dsp`, avoiding duplicate `trem` artifacts in the graph).

use trem::event::NoteEvent;
use trem::graph::Graph;
use trem::math::Rational;
use trem::pitch::Tuning;
use trem::render::render_pattern;
use trem::tree::Tree;
use trem_dsp::{Adsr, Gain, Oscillator, Waveform};

#[test]
fn render_simple_pattern() {
    let scale = Tuning::edo12().to_scale();

    let tree = Tree::seq(vec![
        Tree::leaf(NoteEvent::simple(0)),
        Tree::rest(),
        Tree::leaf(NoteEvent::simple(4)),
        Tree::rest(),
    ]);

    let mut graph = Graph::new(512);
    let osc = graph.add_node(Box::new(Oscillator::new(Waveform::Saw)));
    let env = graph.add_node(Box::new(Adsr::new(0.005, 0.05, 0.3, 0.1)));
    let gain = graph.add_node(Box::new(Gain::new(0.5)));
    graph.connect(osc, 0, env, 0);
    graph.connect(env, 0, gain, 0);

    let output = render_pattern(
        &tree,
        Rational::integer(4),
        120.0,
        44100.0,
        &scale,
        440.0,
        &mut graph,
        gain,
    )
    .expect("render");

    assert_eq!(output.len(), 2);
    assert!(output[0].len() >= 88200);
    let energy: f32 = output[0].iter().map(|s| s * s).sum();
    assert!(energy > 0.0, "output should contain audio");
}

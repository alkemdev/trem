//! Mono sine [`Node`] → gain [`Node`] → read peak after [`Graph::run`].
//!
//! Shows [`trem::graph::PrepareEnv`] / [`trem::graph::Node::prepare`] on the source and
//! [`Graph::run`](trem::graph::Graph::run) returning [`Result`](Result).

use std::f64::consts::TAU;
use trem::graph::{Graph, Node, NodeInfo, PrepareEnv, PrepareError, ProcessContext, Sig};

struct SineSource {
    phase: f64,
    frequency_hz: f64,
}

impl Node for SineSource {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "sine",
            sig: Sig::SOURCE1,
            description: "",
        }
    }

    fn prepare(&mut self, env: &PrepareEnv) -> Result<(), PrepareError> {
        if !env.sample_rate.is_finite() || env.sample_rate <= 0.0 {
            return Err(PrepareError("sample_rate must be positive".into()));
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        let sr = ctx.sample_rate;
        for i in 0..ctx.frames {
            ctx.outputs[0][i] = (TAU * self.phase).sin() as f32 * 0.2;
            self.phase += self.frequency_hz / sr;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }
}

struct GainDb {
    linear: f32,
}

impl Node for GainDb {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "gain",
            sig: Sig::MONO,
            description: "",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for i in 0..ctx.frames {
            ctx.outputs[0][i] = ctx.inputs[0][i] * self.linear;
        }
    }

    fn reset(&mut self) {}
}

fn main() {
    let mut graph = Graph::new(256);
    let src = graph.add_node(Box::new(SineSource {
        phase: 0.0,
        frequency_hz: 440.0,
    }));
    let gain = graph.add_node(Box::new(GainDb {
        linear: 10f32.powf(-6.0 / 20.0),
    }));
    graph.connect(src, 0, gain, 0);

    graph.run(128, 48_000.0, &[]).expect("graph run");

    let out = graph.output_buffer(gain, 0);
    let peak = out[..128].iter().map(|s| s.abs()).fold(0f32, f32::max);
    println!("peak after −6 dB gain ≈ {:.4} (expect ≈ 0.1)", peak);
}

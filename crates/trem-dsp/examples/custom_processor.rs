//! Implementing a custom Node: a simple waveshaper (soft-clip distortion).
//!
//! Run with: `cargo run -p trem-dsp --example custom_processor`

use trem::graph::{
    Graph, Node, NodeInfo, ParamDescriptor, ParamFlags, ParamUnit, ProcessContext, Sig,
};
use trem_dsp::{Oscillator, Waveform};

/// Soft-clip waveshaper: applies `tanh(drive * x)` to each sample.
struct Waveshaper {
    drive: f64,
}

impl Waveshaper {
    fn new(drive: f64) -> Self {
        Self { drive }
    }
}

impl Node for Waveshaper {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "waveshaper",
            sig: Sig {
                inputs: 1,
                outputs: 1,
            },
            description: "Soft-clip distortion via tanh",
        }
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            id: 0,
            name: "Drive",
            min: 0.1,
            max: 20.0,
            default: 1.0,
            unit: ParamUnit::Linear,
            flags: ParamFlags::empty(),
            step: 0.5,
            group: None,
            help: "Amount of distortion applied to the signal",
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.drive,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if id == 0 {
            self.drive = value.clamp(0.1, 20.0);
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let input = ctx.inputs[0];
        let output = &mut ctx.outputs[0];
        for i in 0..ctx.frames {
            output[i] = (self.drive as f32 * input[i]).tanh();
        }
    }

    fn reset(&mut self) {}
}

fn main() {
    let ws = Waveshaper::new(5.0);
    let info = ws.info();
    println!("Custom node: {}", info.name);
    println!("  Inputs:  {}", info.sig.inputs);
    println!("  Outputs: {}", info.sig.outputs);
    println!("  Description: {}", info.description);
    for p in ws.params() {
        println!(
            "  Param '{}': [{}, {}] default={}",
            p.name, p.min, p.max, p.default
        );
    }

    let mut graph = Graph::new(64);
    let osc = graph.add_node(Box::new(Oscillator::new(Waveform::Sine)));
    let shaper = graph.add_node(Box::new(Waveshaper::new(5.0)));
    graph.connect(osc, 0, shaper, 0);

    use trem::event::GraphEvent;
    use trem::event::TimedEvent;

    let events = [TimedEvent {
        sample_offset: 0,
        event: GraphEvent::NoteOn {
            frequency: 440.0,
            velocity: 1.0,
            voice: 0,
        },
    }];

    graph.run(64, 44100.0, &events).expect("graph run");
    let buf = graph.output_buffer(shaper, 0);
    let peak: f32 = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    println!("\nProcessed 64 samples through waveshaper (drive=5.0)");
    println!("  Peak output: {peak:.4} (soft-clipped, always <= 1.0)");
}

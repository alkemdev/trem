//! Fixed delay: line buffer allocated in [`Node::prepare`](trem::graph::Node::prepare), not in
//! [`Node::process`](trem::graph::Node::process).
//!
//! If [`Graph::run`] is called with `frames` larger than the graph’s initial block capacity, the
//! graph rebuilds and **prepare runs again** so the delay line can grow.

use trem::graph::{Graph, Node, NodeInfo, PrepareEnv, PrepareError, ProcessContext, Sig};

struct FixedDelay {
    line: Vec<f32>,
    write: usize,
    delay_samples: usize,
}

impl FixedDelay {
    fn new(delay_samples: usize) -> Self {
        Self {
            line: Vec::new(),
            write: 0,
            delay_samples,
        }
    }
}

impl Node for FixedDelay {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "delay",
            sig: Sig::MONO,
            description: "",
        }
    }

    fn prepare(&mut self, env: &PrepareEnv) -> Result<(), PrepareError> {
        let cap = env
            .max_block_frames
            .checked_add(self.delay_samples)
            .ok_or_else(|| PrepareError("delay too large".into()))?;
        self.line.clear();
        self.line.resize(cap, 0.0);
        self.write = 0;
        Ok(())
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        let n = ctx.frames;
        let len = self.line.len();
        if self.delay_samples == 0 || len == 0 {
            ctx.outputs[0][..n].copy_from_slice(&ctx.inputs[0][..n]);
            return;
        }
        let d = self.delay_samples % len;
        for i in 0..n {
            let r = (self.write + len - d) % len;
            let delayed = self.line[r];
            ctx.outputs[0][i] = delayed;
            self.line[self.write] = ctx.inputs[0][i];
            self.write = (self.write + 1) % len;
        }
    }

    fn reset(&mut self) {
        self.line.fill(0.0);
        self.write = 0;
    }
}

struct ImpulseThenSilence;

impl Node for ImpulseThenSilence {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "impulse",
            sig: Sig::SOURCE1,
            description: "",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        ctx.outputs[0][..ctx.frames].fill(0.0);
        if ctx.frames > 0 {
            ctx.outputs[0][0] = 1.0;
        }
    }

    fn reset(&mut self) {}
}

fn main() {
    let mut graph = Graph::new(32);
    let src = graph.add_node(Box::new(ImpulseThenSilence));
    let del = graph.add_node(Box::new(FixedDelay::new(8)));
    graph.connect(src, 0, del, 0);

    graph.run(16, 48_000.0, &[]).unwrap();
    let sample = graph.output_buffer(del, 0)[8];
    println!("delayed impulse at index 8 ≈ {:.4} (expect 1.0)", sample);

    graph.run(64, 48_000.0, &[]).unwrap();
    println!(
        "graph block capacity after oversized run = {}",
        graph.block_capacity()
    );
}

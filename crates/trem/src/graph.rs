use crate::event::TimedEvent;

pub type NodeId = u32;
pub type PortIdx = u16;

/// Metadata about a processor's ports.
#[derive(Clone, Debug)]
pub struct ProcessorInfo {
    pub name: &'static str,
    pub audio_inputs: u16,
    pub audio_outputs: u16,
}

/// Describes a single automatable parameter.
///
/// This is the self-describing interface that lets any UI (TUI, web, etc.)
/// enumerate and control a processor's parameters without hardcoding.
#[derive(Clone, Debug)]
pub struct ParamDescriptor {
    pub id: u32,
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub unit: ParamUnit,
    pub flags: ParamFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamUnit {
    Linear,
    Decibels,
    Hertz,
    Milliseconds,
    Seconds,
    Percent,
    Semitones,
    Octaves,
}

impl ParamUnit {
    pub fn suffix(self) -> &'static str {
        match self {
            ParamUnit::Linear => "",
            ParamUnit::Decibels => " dB",
            ParamUnit::Hertz => " Hz",
            ParamUnit::Milliseconds => " ms",
            ParamUnit::Seconds => " s",
            ParamUnit::Percent => "%",
            ParamUnit::Semitones => " st",
            ParamUnit::Octaves => " oct",
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ParamFlags: u8 {
        const NONE      = 0;
        /// Logarithmic knob scaling (good for frequency, time).
        const LOG_SCALE = 1 << 0;
        /// Bipolar range (centered at 0, e.g. pan -1..1).
        const BIPOLAR   = 1 << 1;
    }
}

/// Context passed to a processor for each processing block.
pub struct ProcessContext<'a> {
    pub inputs: &'a [&'a [f32]],
    pub outputs: &'a mut [Vec<f32>],
    pub frames: usize,
    pub sample_rate: f64,
    pub events: &'a [TimedEvent],
}

/// Trait for audio processing nodes.
///
/// `params` / `get_param` / `set_param` form the self-describing interface
/// that any UI can enumerate and drive without coupling to the concrete type.
pub trait Processor: Send {
    fn info(&self) -> ProcessorInfo;
    fn process(&mut self, ctx: &mut ProcessContext);
    fn reset(&mut self);

    /// Enumerate all controllable parameters.
    fn params(&self) -> Vec<ParamDescriptor> {
        vec![]
    }

    /// Read a parameter's current value by id.
    fn get_param(&self, _id: u32) -> f64 {
        0.0
    }

    /// Write a parameter value by id.
    fn set_param(&mut self, _id: u32, _value: f64) {}
}

/// A connection between two ports in the graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    pub src_node: NodeId,
    pub src_port: PortIdx,
    pub dst_node: NodeId,
    pub dst_port: PortIdx,
}

/// Pre-allocated buffer pool for graph evaluation.
struct BufferPool {
    buffers: Vec<Vec<f32>>,
    block_size: usize,
}

impl BufferPool {
    fn new(count: usize, block_size: usize) -> Self {
        Self {
            buffers: (0..count).map(|_| vec![0.0; block_size]).collect(),
            block_size,
        }
    }

    fn resize(&mut self, count: usize, block_size: usize) {
        self.block_size = block_size;
        self.buffers.resize_with(count, || vec![0.0; block_size]);
        for buf in &mut self.buffers {
            buf.resize(block_size, 0.0);
        }
    }

    fn zero_all(&mut self) {
        for buf in &mut self.buffers {
            buf.iter_mut().for_each(|s| *s = 0.0);
        }
    }

    fn get(&self, idx: usize) -> &[f32] {
        &self.buffers[idx]
    }

    fn get_mut(&mut self, idx: usize) -> &mut Vec<f32> {
        &mut self.buffers[idx]
    }
}

/// Audio processing directed acyclic graph.
///
/// Nodes are processors, edges route audio between ports.
/// Evaluation follows topological order with pre-allocated buffers.
pub struct Graph {
    nodes: Vec<Option<Box<dyn Processor>>>,
    edges: Vec<Edge>,
    topo_order: Vec<NodeId>,
    /// For each node, index into buffer_pool for its first output buffer.
    /// Outputs for node i occupy buffer_offsets[i]..buffer_offsets[i]+num_outputs.
    buffer_offsets: Vec<usize>,
    buffer_pool: BufferPool,
    dirty: bool,
    block_size: usize,
}

impl Graph {
    pub fn new(block_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            topo_order: Vec::new(),
            buffer_offsets: Vec::new(),
            buffer_pool: BufferPool::new(0, block_size),
            dirty: true,
            block_size,
        }
    }

    /// Add a processor node, returns its NodeId.
    pub fn add_node(&mut self, processor: Box<dyn Processor>) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Some(processor));
        self.dirty = true;
        id
    }

    /// Connect src_node:src_port → dst_node:dst_port.
    pub fn connect(
        &mut self,
        src_node: NodeId,
        src_port: PortIdx,
        dst_node: NodeId,
        dst_port: PortIdx,
    ) {
        self.edges.push(Edge {
            src_node,
            src_port,
            dst_node,
            dst_port,
        });
        self.dirty = true;
    }

    pub fn node_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    /// Recompute topological order and allocate buffers.
    fn rebuild(&mut self) {
        let n = self.nodes.len();

        // Topological sort (Kahn's algorithm)
        let mut in_degree = vec![0u32; n];
        let mut adj: Vec<Vec<NodeId>> = vec![vec![]; n];
        for edge in &self.edges {
            in_degree[edge.dst_node as usize] += 1;
            adj[edge.src_node as usize].push(edge.dst_node);
        }

        let mut queue: Vec<NodeId> = (0..n as NodeId)
            .filter(|&i| self.nodes[i as usize].is_some() && in_degree[i as usize] == 0)
            .collect();
        let mut order = Vec::with_capacity(n);

        while let Some(node) = queue.pop() {
            order.push(node);
            for &next in &adj[node as usize] {
                in_degree[next as usize] -= 1;
                if in_degree[next as usize] == 0 {
                    queue.push(next);
                }
            }
        }

        self.topo_order = order;

        // Compute buffer offsets: each node gets slots for its outputs
        self.buffer_offsets = vec![0; n];
        let mut total_buffers = 0usize;
        for i in 0..n {
            self.buffer_offsets[i] = total_buffers;
            if let Some(ref proc) = self.nodes[i] {
                total_buffers += proc.info().audio_outputs as usize;
            }
        }

        self.buffer_pool
            .resize(total_buffers.max(1), self.block_size);
        self.dirty = false;
    }

    /// Process one block of audio.
    pub fn process(&mut self, frames: usize, sample_rate: f64, events: &[TimedEvent]) {
        if self.dirty {
            self.rebuild();
        }
        if frames > self.block_size {
            self.block_size = frames;
            self.dirty = true;
            self.rebuild();
        }

        self.buffer_pool.zero_all();

        let order = self.topo_order.clone();

        for &node_id in &order {
            let ni = node_id as usize;

            // Gather input buffers by summing connected source outputs
            let num_inputs = match &self.nodes[ni] {
                Some(p) => p.info().audio_inputs as usize,
                None => continue,
            };
            let num_outputs = self.nodes[ni].as_ref().unwrap().info().audio_outputs as usize;

            // Build temporary input buffers by mixing connected sources
            let mut input_bufs: Vec<Vec<f32>> = vec![vec![0.0; frames]; num_inputs];
            for edge in &self.edges {
                if edge.dst_node == node_id && (edge.dst_port as usize) < num_inputs {
                    let src_buf_idx =
                        self.buffer_offsets[edge.src_node as usize] + edge.src_port as usize;
                    let src = self.buffer_pool.get(src_buf_idx);
                    let dst = &mut input_bufs[edge.dst_port as usize];
                    for i in 0..frames {
                        dst[i] += src[i];
                    }
                }
            }

            let input_refs: Vec<&[f32]> = input_bufs.iter().map(|b| b.as_slice()).collect();

            // Prepare output buffers — take from pool, ensure capacity, zero the working region
            let out_offset = self.buffer_offsets[ni];
            let mut output_bufs: Vec<Vec<f32>> = (0..num_outputs)
                .map(|p| {
                    let mut buf = std::mem::take(self.buffer_pool.get_mut(out_offset + p));
                    if buf.len() < frames {
                        buf.resize(frames, 0.0);
                    }
                    buf[..frames].fill(0.0);
                    buf
                })
                .collect();

            {
                let mut ctx = ProcessContext {
                    inputs: &input_refs,
                    outputs: &mut output_bufs,
                    frames,
                    sample_rate,
                    events,
                };

                if let Some(ref mut proc) = self.nodes[ni] {
                    proc.process(&mut ctx);
                }
            }

            // Write outputs back to pool
            for (p, buf) in output_bufs.into_iter().enumerate() {
                *self.buffer_pool.get_mut(out_offset + p) = buf;
            }
        }
    }

    /// Read the output buffer of a node/port after processing.
    pub fn output_buffer(&self, node: NodeId, port: PortIdx) -> &[f32] {
        let idx = self.buffer_offsets[node as usize] + port as usize;
        self.buffer_pool.get(idx)
    }

    pub fn reset(&mut self) {
        for node in &mut self.nodes {
            if let Some(ref mut p) = node {
                p.reset();
            }
        }
    }

    /// Snapshot of graph topology for display purposes.
    pub fn topology(&self) -> (Vec<(NodeId, &'static str)>, Vec<Edge>) {
        let nodes = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_ref().map(|p| (i as NodeId, p.info().name)))
            .collect();
        (nodes, self.edges.clone())
    }

    /// Get parameter descriptors for a specific node.
    pub fn node_params(&self, node: NodeId) -> Vec<ParamDescriptor> {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.params(),
            _ => vec![],
        }
    }

    /// Read a parameter value from a specific node.
    pub fn node_param_value(&self, node: NodeId, param_id: u32) -> f64 {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.get_param(param_id),
            _ => 0.0,
        }
    }

    /// Set a parameter value on a specific node.
    pub fn set_node_param(&mut self, node: NodeId, param_id: u32, value: f64) {
        if let Some(Some(ref mut p)) = self.nodes.get_mut(node as usize) {
            p.set_param(param_id, value);
        }
    }

    /// Snapshot all parameter descriptors and current values for every node.
    /// Returns one entry per node-index in `graph_nodes` order.
    pub fn snapshot_all_params(
        &self,
        node_ids: &[NodeId],
    ) -> Vec<(Vec<ParamDescriptor>, Vec<f64>)> {
        node_ids
            .iter()
            .map(|&id| {
                let descs = self.node_params(id);
                let vals: Vec<f64> = descs
                    .iter()
                    .map(|d| self.node_param_value(id, d.id))
                    .collect();
                (descs, vals)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ConstGen {
        value: f32,
    }

    impl Processor for ConstGen {
        fn info(&self) -> ProcessorInfo {
            ProcessorInfo {
                name: "const",
                audio_inputs: 0,
                audio_outputs: 1,
            }
        }
        fn process(&mut self, ctx: &mut ProcessContext) {
            for i in 0..ctx.frames {
                ctx.outputs[0][i] = self.value;
            }
        }
        fn reset(&mut self) {}
    }

    struct Doubler;

    impl Processor for Doubler {
        fn info(&self) -> ProcessorInfo {
            ProcessorInfo {
                name: "doubler",
                audio_inputs: 1,
                audio_outputs: 1,
            }
        }
        fn process(&mut self, ctx: &mut ProcessContext) {
            for i in 0..ctx.frames {
                ctx.outputs[0][i] = ctx.inputs[0][i] * 2.0;
            }
        }
        fn reset(&mut self) {}
    }

    #[test]
    fn simple_graph() {
        let mut graph = Graph::new(64);
        let gen = graph.add_node(Box::new(ConstGen { value: 0.5 }));
        let dbl = graph.add_node(Box::new(Doubler));
        graph.connect(gen, 0, dbl, 0);
        graph.process(64, 44100.0, &[]);

        let out = graph.output_buffer(dbl, 0);
        assert_eq!(out.len(), 64);
        for &s in out {
            assert!((s - 1.0).abs() < 1e-6);
        }
    }
}

//! Audio graph: [`Processor`] nodes, routing, block processing, and parameter introspection.
//!
//! [`Graph`] sorts nodes topologically, sums incoming edges per input port, and reuses a buffer pool per block.

use crate::event::TimedEvent;

/// Stable index for a node in a [`Graph`].
pub type NodeId = u32;
/// Audio input or output port index on a node.
pub type PortIdx = u16;

/// Metadata about a processor's ports.
#[derive(Clone, Debug)]
pub struct ProcessorInfo {
    /// Short name for UIs and debugging.
    pub name: &'static str,
    /// How many input buffers are mixed from upstream connections.
    pub audio_inputs: u16,
    /// How many output buffers this node writes each block.
    pub audio_outputs: u16,
}

/// Describes a single automatable parameter.
///
/// This is the self-describing interface that lets any UI (TUI, web, etc.)
/// enumerate and control a processor's parameters without hardcoding.
#[derive(Clone, Debug)]
pub struct ParamDescriptor {
    /// Opaque id used with [`Processor::get_param`] / [`Processor::set_param`].
    pub id: u32,
    /// Human-readable label for automation UIs.
    pub name: &'static str,
    /// Minimum allowed value (inclusive unless a processor documents otherwise).
    pub min: f64,
    /// Maximum allowed value (inclusive unless documented otherwise).
    pub max: f64,
    /// Value after [`Processor::reset`] or construction.
    pub default: f64,
    /// How to display and interpret the number (e.g. dB vs linear gain).
    pub unit: ParamUnit,
    /// Knob curve and range shape hints.
    pub flags: ParamFlags,
    /// Coarse nudge increment for arrow-key editing. UIs use `step / 10` for
    /// fine adjustments. Zero means "let the UI pick a sensible default".
    pub step: f64,
    /// Which [`ParamGroup`] this parameter belongs to, if any.
    pub group: Option<u32>,
}

/// A named group of related parameters with a visualization hint.
///
/// Processors declare groups via [`Processor::param_groups`]; the group `id`
/// matches the `group` field on individual [`ParamDescriptor`]s. UIs use the
/// [`GroupHint`] to decide what kind of preview widget to render.
#[derive(Clone, Debug)]
pub struct ParamGroup {
    pub id: u32,
    pub name: &'static str,
    pub hint: GroupHint,
}

/// Tells the UI what kind of mini-visualization to show for a parameter group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupHint {
    /// No special visualization — just a labeled section.
    Generic,
    /// ADSR envelope shape (expects Attack, Decay, Sustain, Release params).
    Envelope,
    /// Frequency response curve (expects Cutoff, Resonance params).
    Filter,
    /// Waveform shape (expects waveform type / detune params).
    Oscillator,
    /// Time-domain effect (delay lines, reverb tails).
    TimeBased,
    /// Single level / gain control.
    Level,
}

/// Display/scaling hint for a parameter value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamUnit {
    /// Unlabeled linear quantity.
    Linear,
    /// Decibels (see [`ParamUnit::suffix`] for display).
    Decibels,
    /// Frequency in Hz.
    Hertz,
    /// Time in milliseconds.
    Milliseconds,
    /// Time in seconds.
    Seconds,
    /// 0–100 style percentage when paired with `min`/`max`.
    Percent,
    /// Pitch interval in semitones.
    Semitones,
    /// Octave shift or span in octaves.
    Octaves,
}

impl ParamUnit {
    /// Short suffix for readouts (empty for [`ParamUnit::Linear`]).
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
    /// UI hints for mapping normalized controls to parameter values.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ParamFlags: u8 {
        /// No extra interpretation beyond `min`/`max`.
        const NONE      = 0;
        /// Logarithmic knob scaling (good for frequency, time).
        const LOG_SCALE = 1 << 0;
        /// Bipolar range (centered at 0, e.g. pan -1..1).
        const BIPOLAR   = 1 << 1;
    }
}

/// Context passed to a processor for each processing block.
pub struct ProcessContext<'a> {
    /// One slice per input port, each `frames` long (summed upstream by the graph).
    pub inputs: &'a [&'a [f32]],
    /// One buffer per output port; processors write `frames` samples from the start.
    pub outputs: &'a mut [Vec<f32>],
    /// Samples in this callback (≤ graph block size).
    pub frames: usize,
    /// Host sample rate in Hz.
    pub sample_rate: f64,
    /// Block-relative [`TimedEvent`]s (offsets in `[0, frames)`).
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

    /// Declare parameter groups for structured UI rendering.
    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![]
    }

    /// Read a parameter's current value by id.
    fn get_param(&self, _id: u32) -> f64 {
        0.0
    }

    /// Write a parameter value by id.
    fn set_param(&mut self, _id: u32, _value: f64) {}
}

/// Routes one output port to one input port; multiple edges into the same input are summed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    /// Source node index.
    pub src_node: NodeId,
    /// Which output on `src_node`.
    pub src_port: PortIdx,
    /// Destination node index.
    pub dst_node: NodeId,
    /// Which input on `dst_node`.
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
    /// Empty graph with initial block allocation size (may grow if `process` uses larger `frames`).
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

    /// Number of live (non-removed) processor slots.
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

    /// Runs the graph in topological order for `frames` samples, mixing edges into inputs and passing `events` to each node.
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

    /// Slice of the last [`Graph::process`] output for `node`/`port` (length ≥ last `frames`; only first `frames` are valid).
    pub fn output_buffer(&self, node: NodeId, port: PortIdx) -> &[f32] {
        let idx = self.buffer_offsets[node as usize] + port as usize;
        self.buffer_pool.get(idx)
    }

    /// Calls [`Processor::reset`] on every node (e.g. clear oscillator phase, envelopes).
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

    /// Delegates to [`Processor::params`]; unknown or empty slots return an empty vec.
    pub fn node_params(&self, node: NodeId) -> Vec<ParamDescriptor> {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.params(),
            _ => vec![],
        }
    }

    /// Delegates to [`Processor::param_groups`]; unknown or empty slots return an empty vec.
    pub fn node_param_groups(&self, node: NodeId) -> Vec<ParamGroup> {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.param_groups(),
            _ => vec![],
        }
    }

    /// Reads [`Processor::get_param`]; missing nodes return `0.0`.
    pub fn node_param_value(&self, node: NodeId, param_id: u32) -> f64 {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.get_param(param_id),
            _ => 0.0,
        }
    }

    /// Writes [`Processor::set_param`] if the node exists; otherwise no-op.
    pub fn set_node_param(&mut self, node: NodeId, param_id: u32, value: f64) {
        if let Some(Some(ref mut p)) = self.nodes.get_mut(node as usize) {
            p.set_param(param_id, value);
        }
    }

    /// Snapshot all parameter descriptors, groups, and current values for every node.
    /// Returns one entry per node-index in `graph_nodes` order.
    pub fn snapshot_all_params(
        &self,
        node_ids: &[NodeId],
    ) -> Vec<(Vec<ParamDescriptor>, Vec<f64>, Vec<ParamGroup>)> {
        node_ids
            .iter()
            .map(|&id| {
                let descs = self.node_params(id);
                let vals: Vec<f64> = descs
                    .iter()
                    .map(|d| self.node_param_value(id, d.id))
                    .collect();
                let groups = self.node_param_groups(id);
                (descs, vals, groups)
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

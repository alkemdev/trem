//! Audio graph: [`Processor`] nodes, routing, block processing, and parameter introspection.
//!
//! [`Graph`] sorts nodes topologically, sums incoming edges per input port, and reuses a buffer pool per block.

use crate::event::TimedEvent;

/// Stable index for a node in a [`Graph`].
pub type NodeId = u32;
/// Audio input or output port index on a node.
pub type PortIdx = u16;

/// Port signature: the I/O shape of a processor as a typed pair.
///
/// Composition laws (symmetric monoidal category over signal bundles):
///   - **chain**:    `Sig(a,b) ; Sig(b,c) = Sig(a,c)`  (requires matching width)
///   - **parallel**: `Sig(a,b) | Sig(c,d) = Sig(a+c, b+d)`
///   - **identity**: `Sig(n,n)`  (pass-through)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sig {
    pub inputs: u16,
    pub outputs: u16,
}

/// Returned when two signatures cannot be sequentially composed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigMismatch {
    pub expected: u16,
    pub got: u16,
}

impl std::fmt::Display for SigMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "port mismatch: expected {} outputs, got {} inputs",
            self.expected, self.got
        )
    }
}

impl std::error::Error for SigMismatch {}

impl Sig {
    pub const MONO: Sig = Sig {
        inputs: 1,
        outputs: 1,
    };
    pub const STEREO: Sig = Sig {
        inputs: 2,
        outputs: 2,
    };
    pub const SOURCE1: Sig = Sig {
        inputs: 0,
        outputs: 1,
    };
    pub const SOURCE2: Sig = Sig {
        inputs: 0,
        outputs: 2,
    };

    /// Sequential composition: `self` feeds `other`.
    /// Requires `self.outputs == other.inputs`.
    pub fn chain(self, other: Sig) -> Result<Sig, SigMismatch> {
        if self.outputs == other.inputs {
            Ok(Sig {
                inputs: self.inputs,
                outputs: other.outputs,
            })
        } else {
            Err(SigMismatch {
                expected: self.outputs,
                got: other.inputs,
            })
        }
    }

    /// Parallel composition: side by side, I/O counts sum.
    pub fn parallel(self, other: Sig) -> Sig {
        Sig {
            inputs: self.inputs + other.inputs,
            outputs: self.outputs + other.outputs,
        }
    }

    /// True when the processor has both inputs and outputs (effect/filter).
    pub fn is_effect(&self) -> bool {
        self.inputs > 0 && self.outputs > 0
    }

    /// True when the processor generates signal with no audio input (oscillator, noise, etc.).
    pub fn is_source(&self) -> bool {
        self.inputs == 0 && self.outputs > 0
    }

    /// True when the processor consumes signal with no audio output (analyzer, meter, etc.).
    pub fn is_sink(&self) -> bool {
        self.inputs > 0 && self.outputs == 0
    }
}

/// Metadata about a processor's ports.
#[derive(Clone, Debug)]
pub struct ProcessorInfo {
    /// Short name for UIs and debugging.
    pub name: &'static str,
    /// Port signature (input/output counts).
    pub sig: Sig,
    /// One-line description shown in help/info panels. Empty string if not set.
    pub description: &'static str,
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
    /// Short help text shown in info panels when hovering this parameter.
    pub help: &'static str,
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

    /// If this processor contains an inner [`Graph`], return a reference for introspection.
    fn inner_graph(&self) -> Option<&Graph> {
        None
    }

    /// Mutable access to an inner [`Graph`] for recursive parameter setting.
    fn inner_graph_mut(&mut self) -> Option<&mut Graph> {
        None
    }
}

/// Maps an exposed external parameter to an internal node's parameter.
#[derive(Clone, Debug)]
pub struct MappedParam {
    pub external_id: u32,
    pub node: NodeId,
    pub param_id: u32,
    pub desc: ParamDescriptor,
}

/// No-op source whose output buffers are written directly by a parent [`Graph`].
pub struct GraphInput {
    num_ports: u16,
}

impl GraphInput {
    pub fn new(num_ports: u16) -> Self {
        Self { num_ports }
    }
}

impl Processor for GraphInput {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "input",
            sig: Sig {
                inputs: 0,
                outputs: self.num_ports,
            },
            description: "Graph input ports",
        }
    }
    fn process(&mut self, _ctx: &mut ProcessContext) {}
    fn reset(&mut self) {}
}

/// N-in N-out identity: copies each input port to the corresponding output port.
/// Used internally by combinators to gather signals from multiple nodes.
struct PassThrough {
    channels: u16,
}

impl Processor for PassThrough {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "bus",
            sig: Sig {
                inputs: self.channels,
                outputs: self.channels,
            },
            description: "Internal pass-through bus",
        }
    }
    fn process(&mut self, ctx: &mut ProcessContext) {
        for ch in 0..self.channels as usize {
            if ch < ctx.inputs.len() && ch < ctx.outputs.len() {
                let n = ctx
                    .frames
                    .min(ctx.inputs[ch].len())
                    .min(ctx.outputs[ch].len());
                ctx.outputs[ch][..n].copy_from_slice(&ctx.inputs[ch][..n]);
            }
        }
    }
    fn reset(&mut self) {}
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
///
/// `Graph` itself implements [`Processor`], enabling recursive nesting:
/// a Graph can be inserted as a node in a parent Graph.
pub struct Graph {
    nodes: Vec<Option<Box<dyn Processor>>>,
    edges: Vec<Edge>,
    topo_order: Vec<NodeId>,
    buffer_offsets: Vec<usize>,
    buffer_pool: BufferPool,
    dirty: bool,
    block_size: usize,

    label: &'static str,
    input_node: Option<NodeId>,
    num_inputs: u16,
    output_node: Option<NodeId>,
    num_outputs: u16,
    param_map: Vec<MappedParam>,
    groups: Vec<ParamGroup>,
    next_param_id: u32,
    next_group_id: u32,
    /// Temporary storage for parent inputs when used as a Processor.
    /// `run()` consumes this after zeroing its buffer pool.
    pending_inputs: Vec<Vec<f32>>,
}

impl Graph {
    /// Empty graph with initial block allocation size.
    ///
    /// # Examples
    ///
    /// ```
    /// use trem::graph::Graph;
    /// use trem::dsp::{Oscillator, Adsr, Gain, Waveform};
    ///
    /// let mut g = Graph::new(512);
    /// let osc = g.add_node(Box::new(Oscillator::new(Waveform::Saw)));
    /// let env = g.add_node(Box::new(Adsr::new(0.01, 0.1, 0.5, 0.2)));
    /// let gain = g.add_node(Box::new(Gain::new(0.5)));
    /// g.connect(osc, 0, env, 0);
    /// g.connect(env, 0, gain, 0);
    /// assert_eq!(g.node_count(), 3);
    /// ```
    pub fn new(block_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            topo_order: Vec::new(),
            buffer_offsets: Vec::new(),
            buffer_pool: BufferPool::new(0, block_size),
            dirty: true,
            block_size,
            label: "graph",
            input_node: None,
            num_inputs: 0,
            output_node: None,
            num_outputs: 0,
            param_map: Vec::new(),
            groups: Vec::new(),
            next_param_id: 0,
            next_group_id: 0,
            pending_inputs: Vec::new(),
        }
    }

    /// Construct with a label (used as `ProcessorInfo::name` when this Graph acts as a Processor).
    pub fn labeled(block_size: usize, label: &'static str) -> Self {
        let mut g = Self::new(block_size);
        g.label = label;
        g
    }

    /// The port signature of this graph when used as a processor.
    pub fn sig(&self) -> Sig {
        Sig {
            inputs: self.num_inputs,
            outputs: self.num_outputs,
        }
    }

    /// Designate a [`GraphInput`] node as the entry point for parent audio.
    /// When this Graph is processed as a Processor, parent inputs are copied
    /// into this node's output buffers before `run()`.
    pub fn set_input(&mut self, node: NodeId, num_inputs: u16) {
        self.input_node = Some(node);
        self.num_inputs = num_inputs;
    }

    /// Designate a node whose outputs become this Graph's outputs.
    pub fn set_output(&mut self, node: NodeId, num_outputs: u16) {
        self.output_node = Some(node);
        self.num_outputs = num_outputs;
    }

    /// Declare a parameter group. Returns the assigned group ID.
    pub fn add_group(&mut self, group: ParamGroup) -> u32 {
        let id = self.next_group_id;
        self.next_group_id += 1;
        self.groups.push(ParamGroup { id, ..group });
        id
    }

    /// Expose an internal node's parameter under a new label.
    /// Returns the external parameter ID.
    pub fn expose_param(&mut self, node: NodeId, param_id: u32, label: &'static str) -> u32 {
        self.expose_param_inner(node, param_id, label, None)
    }

    /// Expose an internal node's parameter under a new label, assigned to a group.
    pub fn expose_param_in_group(
        &mut self,
        node: NodeId,
        param_id: u32,
        label: &'static str,
        group: u32,
    ) -> u32 {
        self.expose_param_inner(node, param_id, label, Some(group))
    }

    fn expose_param_inner(
        &mut self,
        node: NodeId,
        param_id: u32,
        label: &'static str,
        group: Option<u32>,
    ) -> u32 {
        let descs = self.node_params(node);
        let mut desc = descs
            .into_iter()
            .find(|d| d.id == param_id)
            .unwrap_or_else(|| panic!("param {param_id} not found on node {node}"));

        let ext_id = self.next_param_id;
        self.next_param_id += 1;
        desc.id = ext_id;
        desc.name = label;
        desc.group = group;

        self.param_map.push(MappedParam {
            external_id: ext_id,
            node,
            param_id,
            desc,
        });
        ext_id
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
                total_buffers += proc.info().sig.outputs as usize;
            }
        }

        self.buffer_pool
            .resize(total_buffers.max(1), self.block_size);
        self.dirty = false;
    }

    /// Runs the graph in topological order for `frames` samples, mixing edges
    /// into inputs and passing `events` to each node. This is the standalone
    /// entry point used by the audio driver. When this Graph is used as a
    /// [`Processor`] inside another Graph, `Processor::process` delegates here.
    pub fn run(&mut self, frames: usize, sample_rate: f64, events: &[TimedEvent]) {
        if self.dirty {
            self.rebuild();
        }
        if frames > self.block_size {
            self.block_size = frames;
            self.dirty = true;
            self.rebuild();
        }

        self.buffer_pool.zero_all();

        if let Some(input_node) = self.input_node {
            if !self.pending_inputs.is_empty() {
                let ni = input_node as usize;
                let out_offset = self.buffer_offsets[ni];
                for (port, data) in self.pending_inputs.drain(..).enumerate() {
                    let buf = self.buffer_pool.get_mut(out_offset + port);
                    let n = frames.min(data.len()).min(buf.len());
                    buf[..n].copy_from_slice(&data[..n]);
                }
            }
        }

        let order = self.topo_order.clone();

        for &node_id in &order {
            // Input node's buffers were already populated from pending_inputs; skip it.
            if self.input_node == Some(node_id) && self.num_inputs > 0 {
                continue;
            }

            let ni = node_id as usize;

            let sig = match &self.nodes[ni] {
                Some(p) => p.info().sig,
                None => continue,
            };
            let num_inputs = sig.inputs as usize;
            let num_outputs = sig.outputs as usize;

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

    /// Returns the description string from a node's `ProcessorInfo`.
    pub fn node_description(&self, node: NodeId) -> &str {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.info().description,
            _ => "",
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

    /// Check whether a node wraps an inner graph (i.e. it is a nested Graph-as-Processor).
    pub fn node_has_children(&self, node: NodeId) -> bool {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.inner_graph().is_some(),
            _ => false,
        }
    }

    // === Recursive introspection ===

    /// Take a full snapshot of this graph level for UI rendering.
    pub fn snapshot(&self) -> GraphSnapshot {
        let nodes = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                n.as_ref().map(|p| {
                    let id = i as u32;
                    let info = p.info();
                    let params = p.params();
                    let param_values = params.iter().map(|d| p.get_param(d.id)).collect();
                    NodeSnapshot {
                        id,
                        name: info.name.to_string(),
                        sig: info.sig,
                        params,
                        param_groups: p.param_groups(),
                        param_values,
                        has_children: p.inner_graph().is_some(),
                    }
                })
            })
            .collect();

        let edges = self
            .edges
            .iter()
            .map(|e| (e.src_node, e.src_port, e.dst_node, e.dst_port))
            .collect();

        GraphSnapshot {
            label: self.label.to_string(),
            sig: self.sig(),
            nodes,
            edges,
        }
    }

    /// Follow a path of node IDs into nested graphs and snapshot the target level.
    pub fn snapshot_at_path(&self, path: &[u32]) -> Option<GraphSnapshot> {
        if path.is_empty() {
            return Some(self.snapshot());
        }

        let node_id = path[0] as usize;
        let node = self.nodes.get(node_id)?.as_ref()?;
        let inner = node.inner_graph()?;
        inner.snapshot_at_path(&path[1..])
    }

    /// Set a parameter on a node reached via a path through nested graphs.
    /// `path` identifies the target graph level, `param_id` identifies the parameter.
    pub fn set_param_at_path(&mut self, path: &[u32], param_id: u32, value: f64) {
        if path.is_empty() {
            return;
        }
        if path.len() == 1 {
            self.set_node_param(path[0], param_id, value);
            return;
        }

        let node_id = path[0] as usize;
        if let Some(Some(ref mut node)) = self.nodes.get_mut(node_id) {
            if let Some(inner) = node.inner_graph_mut() {
                inner.set_param_at_path(&path[1..], param_id, value);
            }
        }
    }

    // === Composition combinators ===

    /// Sequential composition: wire processors `a -> b -> c -> ...`.
    ///
    /// Each processor's outputs must match the next's inputs (validated via [`Sig::chain`]).
    /// The resulting Graph has the first processor's inputs and the last's outputs.
    pub fn chain(
        label: &'static str,
        block_size: usize,
        procs: Vec<Box<dyn Processor>>,
    ) -> Result<Self, SigMismatch> {
        assert!(!procs.is_empty(), "chain requires at least one processor");

        let sigs: Vec<Sig> = procs.iter().map(|p| p.info().sig).collect();
        for i in 1..sigs.len() {
            sigs[i - 1].chain(sigs[i])?;
        }

        let mut g = Graph::labeled(block_size, label);
        let first_inputs = sigs[0].inputs;
        let last_outputs = sigs[sigs.len() - 1].outputs;

        if first_inputs > 0 {
            let inp = g.add_node(Box::new(GraphInput::new(first_inputs)));
            g.set_input(inp, first_inputs);
        }

        let mut node_ids: Vec<NodeId> = Vec::with_capacity(procs.len());
        for p in procs {
            node_ids.push(g.add_node(p));
        }

        if first_inputs > 0 {
            for port in 0..first_inputs {
                g.connect(0, port, node_ids[0], port);
            }
        }

        for i in 1..node_ids.len() {
            let width = sigs[i - 1].outputs;
            for port in 0..width {
                g.connect(node_ids[i - 1], port, node_ids[i], port);
            }
        }

        let last_node = *node_ids.last().unwrap();
        g.set_output(last_node, last_outputs);

        Ok(g)
    }

    /// Parallel composition: processors run independently, I/O counts sum.
    ///
    /// When used as a [`Processor`], parent inputs are split across children in order
    /// and outputs are concatenated.
    pub fn parallel(
        label: &'static str,
        block_size: usize,
        procs: Vec<Box<dyn Processor>>,
    ) -> Self {
        assert!(
            !procs.is_empty(),
            "parallel requires at least one processor"
        );

        let sigs: Vec<Sig> = procs.iter().map(|p| p.info().sig).collect();
        let total_in: u16 = sigs.iter().map(|s| s.inputs).sum();
        let total_out: u16 = sigs.iter().map(|s| s.outputs).sum();

        let mut g = Graph::labeled(block_size, label);

        let inp_id = if total_in > 0 {
            let id = g.add_node(Box::new(GraphInput::new(total_in)));
            g.set_input(id, total_in);
            Some(id)
        } else {
            None
        };

        let collector = g.add_node(Box::new(PassThrough {
            channels: total_out,
        }));

        let mut in_offset: u16 = 0;
        let mut out_offset: u16 = 0;

        for (idx, p) in procs.into_iter().enumerate() {
            let sig = sigs[idx];
            let nid = g.add_node(p);

            if let Some(inp) = inp_id {
                for port in 0..sig.inputs {
                    g.connect(inp, in_offset + port, nid, port);
                }
            }

            for port in 0..sig.outputs {
                g.connect(nid, port, collector, out_offset + port);
            }

            in_offset += sig.inputs;
            out_offset += sig.outputs;
        }

        g.set_output(collector, total_out);
        g
    }

    /// Fluent pipeline builder for ergonomic chain construction.
    pub fn pipeline(label: &'static str, block_size: usize) -> PipelineBuilder {
        PipelineBuilder {
            label,
            block_size,
            input_channels: 0,
            processors: Vec::new(),
        }
    }
}

// === Recursive introspection types ===

/// Path to a node through nested graphs: `[root_node, child_node, ...]`.
pub type NodePath = Vec<u32>;

/// Snapshot of a single node for UI display.
#[derive(Clone, Debug)]
pub struct NodeSnapshot {
    pub id: u32,
    pub name: String,
    pub sig: Sig,
    pub params: Vec<ParamDescriptor>,
    pub param_values: Vec<f64>,
    pub param_groups: Vec<ParamGroup>,
    pub has_children: bool,
}

/// Snapshot of an entire graph level for UI display.
#[derive(Clone, Debug)]
pub struct GraphSnapshot {
    pub label: String,
    pub sig: Sig,
    pub nodes: Vec<NodeSnapshot>,
    pub edges: Vec<(u32, u16, u32, u16)>,
}

/// Fluent builder for sequential processor chains.
pub struct PipelineBuilder {
    label: &'static str,
    block_size: usize,
    input_channels: u16,
    processors: Vec<Box<dyn Processor>>,
}

impl PipelineBuilder {
    /// Declare input channels (call before `then()`).
    pub fn input(mut self, channels: u16) -> Self {
        self.input_channels = channels;
        self
    }

    /// Append a processor to the chain.
    pub fn then(mut self, proc: Box<dyn Processor>) -> Self {
        self.processors.push(proc);
        self
    }

    /// Build the pipeline into a Graph. Panics if empty or sigs are incompatible.
    pub fn build(self) -> Graph {
        assert!(
            !self.processors.is_empty(),
            "pipeline requires at least one processor"
        );

        let sigs: Vec<Sig> = self.processors.iter().map(|p| p.info().sig).collect();

        let mut g = Graph::labeled(self.block_size, self.label);

        if self.input_channels > 0 {
            let inp = g.add_node(Box::new(GraphInput::new(self.input_channels)));
            g.set_input(inp, self.input_channels);
        }

        let mut node_ids: Vec<NodeId> = Vec::with_capacity(self.processors.len());
        for p in self.processors {
            node_ids.push(g.add_node(p));
        }

        if self.input_channels > 0 {
            let width = self.input_channels.min(sigs[0].inputs);
            for port in 0..width {
                g.connect(0, port, node_ids[0], port);
            }
        }

        for i in 1..node_ids.len() {
            let width = sigs[i - 1].outputs.min(sigs[i].inputs);
            for port in 0..width {
                g.connect(node_ids[i - 1], port, node_ids[i], port);
            }
        }

        let last = *node_ids.last().unwrap();
        g.set_output(last, sigs.last().unwrap().outputs);
        g
    }
}

// SAFETY: Graph is Send because all its Processor nodes are Send.
unsafe impl Send for Graph {}

impl Processor for Graph {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: self.label,
            sig: Sig {
                inputs: self.num_inputs,
                outputs: self.num_outputs,
            },
            description: "Composite processor graph",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.pending_inputs.clear();
        if self.input_node.is_some() {
            for port in 0..self.num_inputs as usize {
                if port < ctx.inputs.len() {
                    self.pending_inputs.push(ctx.inputs[port].to_vec());
                } else {
                    self.pending_inputs.push(vec![0.0; ctx.frames]);
                }
            }
        }

        self.run(ctx.frames, ctx.sample_rate, ctx.events);

        if let Some(output_node) = self.output_node {
            for port in 0..self.num_outputs as usize {
                if port < ctx.outputs.len() {
                    let src = self.output_buffer(output_node, port as PortIdx);
                    let frames = ctx.frames.min(src.len());
                    ctx.outputs[port][..frames].copy_from_slice(&src[..frames]);
                }
            }
        }
    }

    fn reset(&mut self) {
        for node in &mut self.nodes {
            if let Some(ref mut p) = node {
                p.reset();
            }
        }
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        self.param_map.iter().map(|m| m.desc.clone()).collect()
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        self.groups.clone()
    }

    fn get_param(&self, id: u32) -> f64 {
        self.param_map
            .iter()
            .find(|m| m.external_id == id)
            .map(|m| self.node_param_value(m.node, m.param_id))
            .unwrap_or(0.0)
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if let Some(m) = self.param_map.iter().find(|m| m.external_id == id) {
            let node = m.node;
            let param_id = m.param_id;
            self.set_node_param(node, param_id, value);
        }
    }

    fn inner_graph(&self) -> Option<&Graph> {
        Some(self)
    }

    fn inner_graph_mut(&mut self) -> Option<&mut Graph> {
        Some(self)
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
                sig: Sig::SOURCE1,
                description: "",
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
                sig: Sig::MONO,
                description: "",
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
        graph.run(64, 44100.0, &[]);

        let out = graph.output_buffer(dbl, 0);
        assert_eq!(out.len(), 64);
        for &s in out {
            assert!((s - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn graph_as_processor() {
        let mut inner = Graph::labeled(64, "inner");
        let gen = inner.add_node(Box::new(ConstGen { value: 0.25 }));
        let dbl = inner.add_node(Box::new(Doubler));
        inner.connect(gen, 0, dbl, 0);
        inner.set_output(dbl, 1);

        assert_eq!(inner.sig(), Sig::SOURCE1);

        let mut outer = Graph::new(64);
        let inner_id = outer.add_node(Box::new(inner));
        outer.run(64, 44100.0, &[]);

        let out = outer.output_buffer(inner_id, 0);
        for &s in &out[..64] {
            assert!((s - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn graph_with_input_node() {
        let mut inner = Graph::labeled(64, "fx");
        let inp = inner.add_node(Box::new(GraphInput::new(1)));
        let dbl = inner.add_node(Box::new(Doubler));
        inner.connect(inp, 0, dbl, 0);
        inner.set_input(inp, 1);
        inner.set_output(dbl, 1);

        assert_eq!(inner.sig(), Sig::MONO);

        let input_data = vec![0.3f32; 64];
        let inputs: Vec<&[f32]> = vec![&input_data];
        let mut outputs = vec![vec![0.0f32; 64]];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames: 64,
            sample_rate: 44100.0,
            events: &[],
        };
        inner.process(&mut ctx);

        for &s in &outputs[0][..64] {
            assert!((s - 0.6).abs() < 1e-5, "expected 0.6, got {s}");
        }
    }

    #[test]
    fn graph_param_exposure() {
        let mut g = Graph::labeled(64, "synth");
        let gen = g.add_node(Box::new(ConstGen { value: 1.0 }));
        let _dbl = g.add_node(Box::new(Doubler));
        // ConstGen has no params, but we can still test the machinery with a real processor
        // Just verify empty param_map works
        assert!(g.params().is_empty());
        assert!(g.param_groups().is_empty());
        assert_eq!(g.info().name, "synth");

        // Test add_group
        let gid = g.add_group(ParamGroup {
            id: 0,
            name: "Test",
            hint: GroupHint::Generic,
        });
        assert_eq!(gid, 0);
        assert_eq!(g.param_groups().len(), 1);

        // node_has_children should return false for leaf processors
        assert!(!g.node_has_children(gen));
    }

    // --- Sig algebra tests ---

    #[test]
    fn sig_chain_valid() {
        let a = Sig {
            inputs: 2,
            outputs: 3,
        };
        let b = Sig {
            inputs: 3,
            outputs: 1,
        };
        assert_eq!(
            a.chain(b),
            Ok(Sig {
                inputs: 2,
                outputs: 1
            })
        );
    }

    #[test]
    fn sig_chain_mismatch() {
        let a = Sig {
            inputs: 2,
            outputs: 3,
        };
        let b = Sig {
            inputs: 2,
            outputs: 1,
        };
        assert_eq!(
            a.chain(b),
            Err(SigMismatch {
                expected: 3,
                got: 2
            })
        );
    }

    #[test]
    fn sig_chain_associativity() {
        let a = Sig {
            inputs: 1,
            outputs: 2,
        };
        let b = Sig {
            inputs: 2,
            outputs: 3,
        };
        let c = Sig {
            inputs: 3,
            outputs: 1,
        };
        let left = a.chain(b).unwrap().chain(c).unwrap();
        let right = a.chain(b.chain(c).unwrap()).unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn sig_chain_identity() {
        let id = Sig {
            inputs: 2,
            outputs: 2,
        };
        let f = Sig {
            inputs: 2,
            outputs: 3,
        };
        assert_eq!(id.chain(f), Ok(f));
        let g = Sig {
            inputs: 1,
            outputs: 2,
        };
        assert_eq!(g.chain(id), Ok(g));
    }

    #[test]
    fn sig_parallel() {
        let a = Sig {
            inputs: 1,
            outputs: 2,
        };
        let b = Sig {
            inputs: 3,
            outputs: 1,
        };
        assert_eq!(
            a.parallel(b),
            Sig {
                inputs: 4,
                outputs: 3
            }
        );
    }

    #[test]
    fn sig_predicates() {
        assert!(Sig::MONO.is_effect());
        assert!(!Sig::MONO.is_source());
        assert!(Sig::SOURCE1.is_source());
        assert!(!Sig::SOURCE1.is_effect());
        assert!(Sig {
            inputs: 2,
            outputs: 0
        }
        .is_sink());
    }

    #[test]
    fn sig_constants() {
        assert_eq!(
            Sig::MONO,
            Sig {
                inputs: 1,
                outputs: 1
            }
        );
        assert_eq!(
            Sig::STEREO,
            Sig {
                inputs: 2,
                outputs: 2
            }
        );
        assert_eq!(
            Sig::SOURCE1,
            Sig {
                inputs: 0,
                outputs: 1
            }
        );
        assert_eq!(
            Sig::SOURCE2,
            Sig {
                inputs: 0,
                outputs: 2
            }
        );
    }

    // --- Combinator tests ---

    #[test]
    fn chain_source_to_effect() {
        let g = Graph::chain(
            "test",
            64,
            vec![Box::new(ConstGen { value: 0.5 }), Box::new(Doubler)],
        )
        .unwrap();
        assert_eq!(g.sig(), Sig::SOURCE1);

        let mut outer = Graph::new(64);
        let inner_id = outer.add_node(Box::new(g));
        outer.run(64, 44100.0, &[]);
        let out = outer.output_buffer(inner_id, 0);
        for &s in &out[..64] {
            assert!((s - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn chain_rejects_mismatch() {
        let result = Graph::chain(
            "bad",
            64,
            vec![
                Box::new(ConstGen { value: 1.0 }), // SOURCE1: 0 in, 1 out
                Box::new(ConstGen { value: 1.0 }), // SOURCE1: 0 in, 1 out -- mismatch: 1 != 0
            ],
        );
        assert!(result.is_err());
    }

    #[test]
    fn chain_with_inputs() {
        let g = Graph::chain("fx", 64, vec![Box::new(Doubler), Box::new(Doubler)]).unwrap();
        assert_eq!(g.sig(), Sig::MONO);

        let input = vec![0.25f32; 64];
        let inputs: Vec<&[f32]> = vec![&input];
        let mut outputs = vec![vec![0.0f32; 64]];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames: 64,
            sample_rate: 44100.0,
            events: &[],
        };
        let mut g = g;
        g.process(&mut ctx);

        for &s in &outputs[0][..64] {
            assert!((s - 1.0).abs() < 1e-5, "0.25 * 2 * 2 = 1.0, got {s}");
        }
    }

    #[test]
    fn chain_sig_matches_algebra() {
        let procs: Vec<Box<dyn Processor>> = vec![Box::new(Doubler), Box::new(Doubler)];
        let s0 = procs[0].info().sig;
        let s1 = procs[1].info().sig;
        let g = Graph::chain("test", 64, procs).unwrap();
        assert_eq!(g.sig(), s0.chain(s1).unwrap());
    }

    #[test]
    fn pipeline_matches_chain() {
        let g_chain = Graph::chain(
            "c",
            64,
            vec![Box::new(ConstGen { value: 0.5 }), Box::new(Doubler)],
        )
        .unwrap();

        let g_pipe = Graph::pipeline("p", 64)
            .then(Box::new(ConstGen { value: 0.5 }))
            .then(Box::new(Doubler))
            .build();

        assert_eq!(g_chain.sig(), g_pipe.sig());

        let mut outer_a = Graph::new(64);
        let id_a = outer_a.add_node(Box::new(g_chain));
        outer_a.run(64, 44100.0, &[]);

        let mut outer_b = Graph::new(64);
        let id_b = outer_b.add_node(Box::new(g_pipe));
        outer_b.run(64, 44100.0, &[]);

        let a = outer_a.output_buffer(id_a, 0);
        let b = outer_b.output_buffer(id_b, 0);
        for i in 0..64 {
            assert!((a[i] - b[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn pipeline_with_input() {
        let mut g = Graph::pipeline("fx", 64)
            .input(1)
            .then(Box::new(Doubler))
            .then(Box::new(Doubler))
            .build();

        assert_eq!(g.sig(), Sig::MONO);

        let data = vec![0.1f32; 64];
        let inputs: Vec<&[f32]> = vec![&data];
        let mut outputs = vec![vec![0.0f32; 64]];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames: 64,
            sample_rate: 44100.0,
            events: &[],
        };
        g.process(&mut ctx);

        for &s in &outputs[0][..64] {
            assert!((s - 0.4).abs() < 1e-5, "0.1 * 2 * 2 = 0.4, got {s}");
        }
    }

    #[test]
    fn parallel_two_sources() {
        let g = Graph::parallel(
            "par",
            64,
            vec![
                Box::new(ConstGen { value: 0.3 }),
                Box::new(ConstGen { value: 0.7 }),
            ],
        );
        assert_eq!(g.sig(), Sig::SOURCE2);

        let mut outer = Graph::new(64);
        let nid = outer.add_node(Box::new(g));
        outer.run(64, 44100.0, &[]);

        let out0 = outer.output_buffer(nid, 0);
        let out1 = outer.output_buffer(nid, 1);
        for i in 0..64 {
            assert!((out0[i] - 0.3).abs() < 1e-6, "port 0 should be 0.3");
            assert!((out1[i] - 0.7).abs() < 1e-6, "port 1 should be 0.7");
        }
    }

    #[test]
    fn node_has_children_for_nested_graph() {
        let inner = Graph::chain(
            "inner",
            64,
            vec![Box::new(ConstGen { value: 1.0 }), Box::new(Doubler)],
        )
        .unwrap();

        let mut outer = Graph::new(64);
        let leaf = outer.add_node(Box::new(ConstGen { value: 1.0 }));
        let nested = outer.add_node(Box::new(inner));

        assert!(!outer.node_has_children(leaf));
        assert!(outer.node_has_children(nested));
    }

    // --- Introspection tests ---

    #[test]
    fn snapshot_flat_graph() {
        let mut g = Graph::labeled(64, "test");
        let _a = g.add_node(Box::new(ConstGen { value: 1.0 }));
        let _b = g.add_node(Box::new(Doubler));
        g.connect(0, 0, 1, 0);

        let snap = g.snapshot();
        assert_eq!(snap.label, "test");
        assert_eq!(snap.nodes.len(), 2);
        assert_eq!(snap.edges.len(), 1);
        assert_eq!(snap.nodes[0].name, "const");
        assert_eq!(snap.nodes[1].name, "doubler");
        assert!(!snap.nodes[0].has_children);
    }

    #[test]
    fn snapshot_at_path_depth_1() {
        let inner = Graph::chain(
            "inner",
            64,
            vec![Box::new(ConstGen { value: 1.0 }), Box::new(Doubler)],
        )
        .unwrap();

        let mut outer = Graph::labeled(64, "root");
        let nested_id = outer.add_node(Box::new(inner));

        let snap = outer.snapshot_at_path(&[nested_id]).unwrap();
        assert_eq!(snap.label, "inner");
        assert!(snap.nodes.len() >= 2);
    }

    #[test]
    fn snapshot_at_path_empty_returns_self() {
        let g = Graph::labeled(64, "me");
        let snap = g.snapshot_at_path(&[]).unwrap();
        assert_eq!(snap.label, "me");
    }

    #[test]
    fn set_param_at_path_depth_1() {
        use crate::dsp::gain::MonoGain;

        let mut inner = Graph::labeled(64, "inner");
        let gain_node = inner.add_node(Box::new(MonoGain::new(1.0)));
        inner.set_output(gain_node, 1);

        let mut outer = Graph::new(64);
        let nested = outer.add_node(Box::new(inner));

        outer.set_param_at_path(&[nested, gain_node], 0, 0.5);

        let val = outer
            .nodes
            .get(nested as usize)
            .and_then(|n| n.as_ref())
            .and_then(|p| p.inner_graph())
            .map(|g| g.node_param_value(gain_node, 0))
            .unwrap();
        assert!((val - 0.5).abs() < 1e-6);
    }
}

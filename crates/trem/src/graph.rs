//! Audio graph: [`Graph`], [`Node`] implementations, routing, and parameter introspection.
//!
//! # Terminology
//!
//! - **[`Node`]** — Trait for a vertex: [`Node::prepare`] (allocation / validation outside the hot
//!   path), then `process` with [`ProcessContext`]; optional parameters. A [`Graph`] is also a
//!   [`Node`] (composite).
//! - **Graph block size** — Upper bound on `ProcessContext::frames` (buffer capacity per callback).
//!   This is “how many samples per `process` call,” not to be confused with [`NodeId`] or the
//!   [`Node`] trait.
//! - **[`Sig`]** — Port counts (inputs × outputs) for a [`Node`]; used to validate wiring.
//!
//! # Building graphs
//!
//! - **[`Graph::new`]** / [`Graph::labeled`] — Empty graph; add [`Node`]s with [`Graph::add_node`]
//!   and wire with [`Graph::connect`].
//! - **[`Graph::from_chain`]** / [`Graph::chain`] — Sequential chain `a → b → c` with automatic
//!   port hookup (signatures must align).
//! - **[`Graph::from_parallel`]** / [`Graph::parallel`] — Side‑by‑side [`Node`]s; inputs and
//!   outputs concatenate in order.
//! - **[`Graph::chain_from_iter`]** — Like [`Graph::chain`], but collects any iterator of
//!   `Box<dyn Node>` (e.g. `vec![a, b, c]`).
//! - **[`Graph::pipeline`]** — Fluent builder: [`PipelineBuilder::input`] then repeated
//!   [`PipelineBuilder::then`].
//!
//! [`Graph`] sorts nodes topologically, sums incoming edges per input port, and reuses a buffer
//! pool each processing pass.

use crate::event::TimedEvent;
use std::collections::HashMap;

/// Stable index for a node in a [`Graph`].
pub type NodeId = u32;
/// Audio input or output port index on a node.
pub type PortIdx = u16;

/// Upper bounds and sample rate for [`Node::prepare`] (delay lines, tables, etc.).
///
/// Call **outside** the per-block hot path when topology or [`Graph`] block capacity changes.
#[derive(Clone, Debug, PartialEq)]
pub struct PrepareEnv {
    /// Maximum `ProcessContext::frames` the host will pass (graph buffer capacity).
    pub max_block_frames: usize,
    pub sample_rate: f64,
}

impl PrepareEnv {
    /// Builds the environment used by [`Graph::run`] after (re)allocation.
    pub fn new(max_block_frames: usize, sample_rate: f64) -> Self {
        Self {
            max_block_frames,
            sample_rate,
        }
    }
}

/// [`Node::prepare`] failed (invalid rate, size overflow, unsupported configuration).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrepareError(pub String);

impl std::fmt::Display for PrepareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for PrepareError {}

/// Port signature: the I/O shape of a [`Node`] as a typed pair.
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

    /// True when the node has both inputs and outputs (effect/filter).
    pub fn is_effect(&self) -> bool {
        self.inputs > 0 && self.outputs > 0
    }

    /// True when the node generates signal with no audio input (oscillator, noise, etc.).
    pub fn is_source(&self) -> bool {
        self.inputs == 0 && self.outputs > 0
    }

    /// True when the node consumes signal with no audio output (analyzer, meter, etc.).
    pub fn is_sink(&self) -> bool {
        self.inputs > 0 && self.outputs == 0
    }
}

/// Metadata about a [`Node`]'s ports and display name.
#[derive(Clone, Debug)]
pub struct NodeInfo {
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
/// enumerate and control a node's parameters without hardcoding.
#[derive(Clone, Debug)]
pub struct ParamDescriptor {
    /// Opaque id used with [`Node::get_param`] / [`Node::set_param`].
    pub id: u32,
    /// Human-readable label for automation UIs.
    pub name: &'static str,
    /// Minimum allowed value (inclusive unless a node documents otherwise).
    pub min: f64,
    /// Maximum allowed value (inclusive unless documented otherwise).
    pub max: f64,
    /// Value after [`Node::reset`] or construction.
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
/// Nodes declare groups via [`Node::param_groups`]; the group `id`
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

/// Context for one audio callback: input/output buffers, frame count, sample rate, and events.
pub struct ProcessContext<'a> {
    /// One slice per input port, each `frames` long (summed upstream by the graph).
    pub inputs: &'a [&'a [f32]],
    /// One buffer per output port; nodes write `frames` samples from the start.
    pub outputs: &'a mut [Vec<f32>],
    /// Samples in this callback (≤ graph block size).
    pub frames: usize,
    /// Host sample rate in Hz.
    pub sample_rate: f64,
    /// Callback-relative [`TimedEvent`]s (offsets in `[0, frames)`).
    pub events: &'a [TimedEvent],
}

/// A node in a [`Graph`]: realtime audio chunk processing plus optional parameter introspection.
///
/// Implement this for oscillators, effects, utilities, or wrap a nested [`Graph`] (which already
/// implements `Node`). [`NodeInfo`] describes port counts; [`ParamDescriptor`] / [`ParamGroup`]
/// power generic UIs and automation.
///
/// `params` / `get_param` / `set_param` form the self-describing interface that any UI can
/// enumerate without coupling to the concrete type.
pub trait Node: Send {
    fn info(&self) -> NodeInfo;
    fn process(&mut self, ctx: &mut ProcessContext);
    fn reset(&mut self);

    /// Allocate or resize internal state when the graph block capacity or sample rate changes.
    ///
    /// The host calls this via [`Graph::run`] after topology rebuilds; override for delay lines,
    /// wavetables, or validation. Default: success.
    fn prepare(&mut self, _env: &PrepareEnv) -> Result<(), PrepareError> {
        Ok(())
    }

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

    /// If this node contains an inner [`Graph`], return a reference for introspection.
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

impl Node for GraphInput {
    fn info(&self) -> NodeInfo {
        NodeInfo {
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

impl Node for PassThrough {
    fn info(&self) -> NodeInfo {
        NodeInfo {
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
/// Nodes are [`Node`] trait objects; edges route audio between ports. Evaluation follows
/// topological order with a reusable buffer pool.
///
/// `Graph` itself implements [`Node`], so you can nest graphs as single nodes in a parent graph.
pub struct Graph {
    nodes: Vec<Option<Box<dyn Node>>>,
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
    /// Temporary storage for parent inputs when used as a Node.
    /// `run()` consumes this after zeroing its buffer pool.
    pending_inputs: Vec<Vec<f32>>,
    /// Per-node per-input-port mix buffers (length `block_size`); reused every [`Graph::run`].
    audio_input_mix: Vec<Vec<Vec<f32>>>,
    needs_prepare: bool,
    last_prepared_sr: f64,
    last_prepared_block: usize,
}

impl Graph {
    /// Empty graph with initial block allocation size.
    ///
    /// # Examples
    ///
    /// ```
    /// use trem::graph::{Graph, ProcessContext, Node, NodeInfo, Sig};
    ///
    /// struct Src;
    /// impl Node for Src {
    ///     fn info(&self) -> NodeInfo {
    ///         NodeInfo {
    ///             name: "src",
    ///             sig: Sig::SOURCE1,
    ///             description: "",
    ///         }
    ///     }
    ///     fn process(&mut self, ctx: &mut ProcessContext) {
    ///         for i in 0..ctx.frames {
    ///             ctx.outputs[0][i] = 0.0;
    ///         }
    ///     }
    ///     fn reset(&mut self) {}
    /// }
    ///
    /// struct Fwd;
    /// impl Node for Fwd {
    ///     fn info(&self) -> NodeInfo {
    ///         NodeInfo {
    ///             name: "fwd",
    ///             sig: Sig::MONO,
    ///             description: "",
    ///         }
    ///     }
    ///     fn process(&mut self, ctx: &mut ProcessContext) {
    ///         for i in 0..ctx.frames {
    ///             ctx.outputs[0][i] = ctx.inputs[0][i];
    ///         }
    ///     }
    ///     fn reset(&mut self) {}
    /// }
    ///
    /// let mut g = Graph::new(512);
    /// let a = g.add_node(Box::new(Src));
    /// let b = g.add_node(Box::new(Fwd));
    /// g.connect(a, 0, b, 0);
    /// assert_eq!(g.node_count(), 2);
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
            audio_input_mix: Vec::new(),
            needs_prepare: true,
            last_prepared_sr: f64::NAN,
            last_prepared_block: 0,
        }
    }

    /// Construct with a label (used as `NodeInfo::name` when this Graph acts as a Node).
    pub fn labeled(block_size: usize, label: &'static str) -> Self {
        let mut g = Self::new(block_size);
        g.label = label;
        g
    }

    /// The port signature of this graph when used as a [`Node`].
    pub fn sig(&self) -> Sig {
        Sig {
            inputs: self.num_inputs,
            outputs: self.num_outputs,
        }
    }

    /// Upper bound on `frames` passed to [`Graph::run`] (buffer capacity after the last rebuild).
    pub fn block_capacity(&self) -> usize {
        self.block_size
    }

    /// Designate a [`GraphInput`] node as the entry point for parent audio.
    /// When this Graph is processed as a Node, parent inputs are copied
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

    /// Add a [`Node`]; returns its [`NodeId`].
    pub fn add_node(&mut self, node: Box<dyn Node>) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Some(node));
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

    /// Number of live (non-removed) node slots.
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

        self.audio_input_mix.clear();
        self.audio_input_mix.reserve(n);
        for i in 0..n {
            let ins = self.nodes[i]
                .as_ref()
                .map(|p| p.info().sig.inputs as usize)
                .unwrap_or(0);
            self.audio_input_mix
                .push(vec![vec![0.0f32; self.block_size]; ins]);
        }

        self.dirty = false;
        self.needs_prepare = true;
    }

    /// Runs [`Node::prepare`] on every node when topology, block capacity, or sample rate changed.
    pub fn prepare(&mut self, env: &PrepareEnv) -> Result<(), PrepareError> {
        let local = PrepareEnv {
            max_block_frames: self.block_size,
            sample_rate: env.sample_rate,
        };
        for slot in &mut self.nodes {
            if let Some(n) = slot.as_mut() {
                n.prepare(&local)?;
            }
        }
        self.needs_prepare = false;
        self.last_prepared_sr = env.sample_rate;
        self.last_prepared_block = self.block_size;
        Ok(())
    }

    fn prepare_if_needed(&mut self, sample_rate: f64) -> Result<(), PrepareError> {
        let stale_sr =
            (self.last_prepared_sr - sample_rate).abs() > 1e-9 && !self.last_prepared_sr.is_nan();
        if self.needs_prepare || stale_sr || self.last_prepared_block != self.block_size {
            let env = PrepareEnv::new(self.block_size, sample_rate);
            self.prepare(&env)?;
        }
        Ok(())
    }

    /// Runs the graph in topological order for `frames` samples, mixing edges
    /// into inputs and passing `events` to each node. This is the standalone
    /// entry point used by the audio driver. When this Graph is used as a
    /// [`Node`] inside another Graph, `Node::process` delegates here.
    pub fn run(
        &mut self,
        frames: usize,
        sample_rate: f64,
        events: &[TimedEvent],
    ) -> Result<(), PrepareError> {
        if self.dirty {
            self.rebuild();
        }
        if frames > self.block_size {
            self.block_size = frames;
            self.dirty = true;
            self.rebuild();
        }

        self.prepare_if_needed(sample_rate)?;

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

            {
                let input_mix = &mut self.audio_input_mix[ni];
                for buf in input_mix.iter_mut() {
                    buf[..frames].fill(0.0);
                }
                for edge in &self.edges {
                    if edge.dst_node == node_id && (edge.dst_port as usize) < num_inputs {
                        let src_buf_idx =
                            self.buffer_offsets[edge.src_node as usize] + edge.src_port as usize;
                        let src = self.buffer_pool.get(src_buf_idx);
                        let dst = &mut input_mix[edge.dst_port as usize];
                        for i in 0..frames {
                            dst[i] += src[i];
                        }
                    }
                }
            }

            let input_refs: Vec<&[f32]> = self.audio_input_mix[ni]
                .iter()
                .map(|b| &b[..frames])
                .collect();

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

            for (p, buf) in output_bufs.into_iter().enumerate() {
                *self.buffer_pool.get_mut(out_offset + p) = buf;
            }
        }

        Ok(())
    }

    /// Slice of the last [`Graph::process`] output for `node`/`port` (length ≥ last `frames`; only first `frames` are valid).
    pub fn output_buffer(&self, node: NodeId, port: PortIdx) -> &[f32] {
        let idx = self.buffer_offsets[node as usize] + port as usize;
        self.buffer_pool.get(idx)
    }

    /// Resolve a nested graph: `path` is successive container node ids from the root (e.g. `[lead_id]`
    /// for the inner graph of the Lead node). Empty path = this graph.
    pub fn graph_at_path(&self, path: &[NodeId]) -> Option<&Graph> {
        if path.is_empty() {
            return Some(self);
        }
        let idx = path[0] as usize;
        let inner = self.nodes.get(idx)?.as_ref()?.inner_graph()?;
        inner.graph_at_path(&path[1..])
    }

    /// Like [`Self::output_buffer`], but for a node inside [`Self::graph_at_path(path)`](Self::graph_at_path).
    pub fn output_buffer_at_path(
        &self,
        path: &[NodeId],
        node: NodeId,
        port: PortIdx,
    ) -> Option<&[f32]> {
        let g = self.graph_at_path(path)?;
        let _ = g.nodes.get(node as usize)?.as_ref()?;
        Some(g.output_buffer(node, port))
    }

    /// Port signature of a node reached via `path` (for preview / UI).
    pub fn node_sig_at_path(&self, path: &[NodeId], node: NodeId) -> Option<Sig> {
        let g = self.graph_at_path(path)?;
        Some(g.nodes.get(node as usize)?.as_ref()?.info().sig)
    }

    /// Sum all edges feeding `node`:`dst_port` into `out[..frames]` (matches one input bus of the node).
    /// `out` should be at least `frames` long; only the first `frames` samples are written.
    pub fn mix_input_port_at_path(
        &self,
        path: &[NodeId],
        node: NodeId,
        dst_port: PortIdx,
        frames: usize,
        out: &mut [f32],
    ) {
        let n_write = frames.min(out.len());
        out[..n_write].fill(0.0);
        let Some(g) = self.graph_at_path(path) else {
            return;
        };
        let Some(Some(proc)) = g.nodes.get(node as usize) else {
            return;
        };
        if dst_port as usize >= proc.info().sig.inputs as usize {
            return;
        }
        for edge in &g.edges {
            if edge.dst_node != node || edge.dst_port != dst_port {
                continue;
            }
            let src = g.output_buffer(edge.src_node, edge.src_port);
            for i in 0..n_write.min(src.len()) {
                out[i] += src[i];
            }
        }
    }

    /// Calls [`Node::reset`] on every node (e.g. clear oscillator phase, envelopes).
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

    /// Delegates to [`Node::params`]; unknown or empty slots return an empty vec.
    pub fn node_params(&self, node: NodeId) -> Vec<ParamDescriptor> {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.params(),
            _ => vec![],
        }
    }

    /// Delegates to [`Node::param_groups`]; unknown or empty slots return an empty vec.
    pub fn node_param_groups(&self, node: NodeId) -> Vec<ParamGroup> {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.param_groups(),
            _ => vec![],
        }
    }

    /// Reads [`Node::get_param`]; missing nodes return `0.0`.
    pub fn node_param_value(&self, node: NodeId, param_id: u32) -> f64 {
        match self.nodes.get(node as usize) {
            Some(Some(ref p)) => p.get_param(param_id),
            _ => 0.0,
        }
    }

    /// Writes [`Node::set_param`] if the node exists; otherwise no-op.
    pub fn set_node_param(&mut self, node: NodeId, param_id: u32, value: f64) {
        if let Some(Some(ref mut p)) = self.nodes.get_mut(node as usize) {
            p.set_param(param_id, value);
        }
    }

    /// Returns the description string from a node's `NodeInfo`.
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

    /// Check whether a node wraps an inner graph (i.e. it is a nested Graph-as-Node).
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

        let edges = self.edges.clone();

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

    /// Snapshots of every nested graph level reachable from the root, keyed by navigation path.
    ///
    /// Keys are **non-empty** paths: `[outer_id]`, `[outer_id, inner_id]`, … matching how the TUI
    /// builds [`NodePath`] when drilling into nested graphs (see `SetParam.path`).
    pub fn nested_ui_snapshots(&self) -> HashMap<Vec<NodeId>, GraphSnapshot> {
        let mut out = HashMap::new();
        let root = self.snapshot();
        for node in &root.nodes {
            if !node.has_children {
                continue;
            }
            let path = vec![node.id];
            if let Some(snap) = self.snapshot_at_path(&path) {
                Self::collect_nested_snapshots(self, &path, &snap, &mut out);
            }
        }
        out
    }

    fn collect_nested_snapshots(
        graph: &Graph,
        path: &[NodeId],
        snap: &GraphSnapshot,
        out: &mut HashMap<Vec<NodeId>, GraphSnapshot>,
    ) {
        out.insert(path.to_vec(), snap.clone());
        for node in &snap.nodes {
            if !node.has_children {
                continue;
            }
            let mut p = path.to_vec();
            p.push(node.id);
            if let Some(inner) = graph.snapshot_at_path(&p) {
                Self::collect_nested_snapshots(graph, &p, &inner, out);
            }
        }
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

    /// Sequential composition: wire nodes `a → b → c → …`.
    ///
    /// Each node's outputs must match the next's inputs (validated via [`Sig::chain`]).
    /// The resulting graph has the first node's inputs and the last's outputs.
    ///
    /// See also [`Graph::from_chain`] and [`Graph::chain_from_iter`].
    pub fn chain(
        label: &'static str,
        block_size: usize,
        nodes: Vec<Box<dyn Node>>,
    ) -> Result<Self, SigMismatch> {
        assert!(!nodes.is_empty(), "chain requires at least one node");

        let sigs: Vec<Sig> = nodes.iter().map(|p| p.info().sig).collect();
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

        let mut node_ids: Vec<NodeId> = Vec::with_capacity(nodes.len());
        for n in nodes {
            node_ids.push(g.add_node(n));
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

    /// Same as [`Graph::chain`]; reads naturally as “graph from a chain of nodes.”
    pub fn from_chain(
        label: &'static str,
        block_size: usize,
        nodes: Vec<Box<dyn Node>>,
    ) -> Result<Self, SigMismatch> {
        Self::chain(label, block_size, nodes)
    }

    /// Like [`Graph::chain`], but collects any iterator (e.g. `vec![a, b, c]`).
    pub fn chain_from_iter<I>(
        label: &'static str,
        block_size: usize,
        iter: I,
    ) -> Result<Self, SigMismatch>
    where
        I: IntoIterator<Item = Box<dyn Node>>,
    {
        Self::chain(label, block_size, iter.into_iter().collect())
    }

    /// Parallel composition: children run independently; I/O counts sum.
    ///
    /// When used as a [`Node`], parent inputs are split across children in order
    /// and outputs are concatenated.
    ///
    /// See also [`Graph::from_parallel`].
    pub fn parallel(label: &'static str, block_size: usize, nodes: Vec<Box<dyn Node>>) -> Self {
        assert!(!nodes.is_empty(), "parallel requires at least one node");

        let sigs: Vec<Sig> = nodes.iter().map(|p| p.info().sig).collect();
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

        for (idx, n) in nodes.into_iter().enumerate() {
            let sig = sigs[idx];
            let nid = g.add_node(n);

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

    /// Same as [`Graph::parallel`].
    pub fn from_parallel(
        label: &'static str,
        block_size: usize,
        nodes: Vec<Box<dyn Node>>,
    ) -> Self {
        Self::parallel(label, block_size, nodes)
    }

    /// Fluent pipeline builder for sequential chains ([`PipelineBuilder::then`]).
    pub fn pipeline(label: &'static str, block_size: usize) -> PipelineBuilder {
        PipelineBuilder {
            label,
            block_size,
            input_channels: 0,
            chain: Vec::new(),
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
    pub edges: Vec<Edge>,
}

/// Fluent builder for sequential [`Node`] chains ([`Graph::pipeline`]).
pub struct PipelineBuilder {
    label: &'static str,
    block_size: usize,
    input_channels: u16,
    chain: Vec<Box<dyn Node>>,
}

impl PipelineBuilder {
    /// Declare input channels (call before [`Self::then`]).
    pub fn input(mut self, channels: u16) -> Self {
        self.input_channels = channels;
        self
    }

    /// Append a [`Node`] to the chain.
    pub fn then(mut self, node: Box<dyn Node>) -> Self {
        self.chain.push(node);
        self
    }

    /// Build into a [`Graph`]. Panics if empty or signatures are incompatible.
    pub fn build(self) -> Graph {
        assert!(
            !self.chain.is_empty(),
            "pipeline requires at least one node"
        );

        let sigs: Vec<Sig> = self.chain.iter().map(|p| p.info().sig).collect();

        let mut g = Graph::labeled(self.block_size, self.label);

        if self.input_channels > 0 {
            let inp = g.add_node(Box::new(GraphInput::new(self.input_channels)));
            g.set_input(inp, self.input_channels);
        }

        let mut node_ids: Vec<NodeId> = Vec::with_capacity(self.chain.len());
        for n in self.chain {
            node_ids.push(g.add_node(n));
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

// SAFETY: Graph is Send because every stored [`Node`] is Send.
unsafe impl Send for Graph {}

impl Node for Graph {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: self.label,
            sig: Sig {
                inputs: self.num_inputs,
                outputs: self.num_outputs,
            },
            description: "Composite graph (nested nodes)",
        }
    }

    fn prepare(&mut self, env: &PrepareEnv) -> Result<(), PrepareError> {
        Graph::prepare(self, env)
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

        self.run(ctx.frames, ctx.sample_rate, ctx.events)
            .expect("Graph::run");

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

    impl Node for ConstGen {
        fn info(&self) -> NodeInfo {
            NodeInfo {
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

    impl Node for Doubler {
        fn info(&self) -> NodeInfo {
            NodeInfo {
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
        graph.run(64, 44100.0, &[]).unwrap();

        let out = graph.output_buffer(dbl, 0);
        assert_eq!(out.len(), 64);
        for &s in out {
            assert!((s - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn graph_as_node() {
        let mut inner = Graph::labeled(64, "inner");
        let gen = inner.add_node(Box::new(ConstGen { value: 0.25 }));
        let dbl = inner.add_node(Box::new(Doubler));
        inner.connect(gen, 0, dbl, 0);
        inner.set_output(dbl, 1);

        assert_eq!(inner.sig(), Sig::SOURCE1);

        let mut outer = Graph::new(64);
        let inner_id = outer.add_node(Box::new(inner));
        outer.run(64, 44100.0, &[]).unwrap();

        let out = outer.output_buffer(inner_id, 0);
        for &s in &out[..64] {
            assert!((s - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn graph_at_path_mix_input_and_output_buffer() {
        let mut inner = Graph::labeled(64, "inner");
        let g0 = inner.add_node(Box::new(ConstGen { value: 0.1 }));
        let g1 = inner.add_node(Box::new(ConstGen { value: 0.2 }));
        let dbl = inner.add_node(Box::new(Doubler));
        inner.connect(g0, 0, dbl, 0);
        inner.connect(g1, 0, dbl, 0);
        inner.set_output(dbl, 1);

        let mut outer = Graph::new(64);
        let inner_id = outer.add_node(Box::new(inner));
        outer.run(64, 44100.0, &[]).unwrap();

        assert!(outer.graph_at_path(&[]).is_some());
        let inner_g = outer.graph_at_path(&[inner_id]).expect("nested graph");
        assert_eq!(inner_g.label, "inner");
        assert_eq!(outer.node_sig_at_path(&[inner_id], dbl), Some(Sig::MONO));

        let mut mix = vec![0.0f32; 64];
        outer.mix_input_port_at_path(&[inner_id], dbl, 0, 64, &mut mix);
        for &s in &mix[..64] {
            assert!(
                (s - 0.3).abs() < 1e-5,
                "expected summed inputs 0.3, got {s}"
            );
        }

        let ob = outer
            .output_buffer_at_path(&[inner_id], dbl, 0)
            .expect("output buffer");
        for &s in &ob[..64] {
            assert!((s - 0.6).abs() < 1e-5, "expected doubled 0.6, got {s}");
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
        // ConstGen has no params, but we can still test the machinery with a real node
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

        // node_has_children should return false for leaf nodes
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
        outer.run(64, 44100.0, &[]).unwrap();
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
        let procs: Vec<Box<dyn Node>> = vec![Box::new(Doubler), Box::new(Doubler)];
        let s0 = procs[0].info().sig;
        let s1 = procs[1].info().sig;
        let g = Graph::chain("test", 64, procs).unwrap();
        assert_eq!(g.sig(), s0.chain(s1).unwrap());
    }

    #[test]
    fn from_chain_and_chain_from_iter_match_chain() {
        let mk = || {
            vec![
                Box::new(ConstGen { value: 0.5 }) as Box<dyn Node>,
                Box::new(Doubler),
            ]
        };
        let a = Graph::chain("a", 64, mk()).unwrap();
        let b = Graph::from_chain("b", 64, mk()).unwrap();
        let c = Graph::chain_from_iter("c", 64, mk()).unwrap();
        assert_eq!(a.sig(), b.sig());
        assert_eq!(a.sig(), c.sig());
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
        outer_a.run(64, 44100.0, &[]).unwrap();

        let mut outer_b = Graph::new(64);
        let id_b = outer_b.add_node(Box::new(g_pipe));
        outer_b.run(64, 44100.0, &[]).unwrap();

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
        outer.run(64, 44100.0, &[]).unwrap();

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
    fn nested_ui_snapshots_maps_nested_id() {
        let inner = Graph::chain(
            "inner",
            64,
            vec![Box::new(ConstGen { value: 1.0 }), Box::new(Doubler)],
        )
        .unwrap();

        let mut outer = Graph::labeled(64, "root");
        let nested_id = outer.add_node(Box::new(inner));

        let map = outer.nested_ui_snapshots();
        let snap = map.get(&vec![nested_id]).expect("nested path key");
        assert_eq!(snap.label, "inner");
    }

    #[test]
    fn snapshot_at_path_empty_returns_self() {
        let g = Graph::labeled(64, "me");
        let snap = g.snapshot_at_path(&[]).unwrap();
        assert_eq!(snap.label, "me");
    }

    #[test]
    fn set_param_at_path_depth_1() {
        struct TestMonoGain {
            level: f32,
        }

        impl Node for TestMonoGain {
            fn info(&self) -> NodeInfo {
                NodeInfo {
                    name: "test_mono_gain",
                    sig: Sig::MONO,
                    description: "",
                }
            }
            fn process(&mut self, ctx: &mut ProcessContext) {
                for i in 0..ctx.frames {
                    ctx.outputs[0][i] = ctx.inputs[0][i] * self.level;
                }
            }
            fn reset(&mut self) {}
            fn params(&self) -> Vec<ParamDescriptor> {
                vec![ParamDescriptor {
                    id: 0,
                    name: "Level",
                    min: 0.0,
                    max: 2.0,
                    default: 1.0,
                    unit: ParamUnit::Linear,
                    flags: ParamFlags::NONE,
                    step: 0.05,
                    group: None,
                    help: "",
                }]
            }
            fn get_param(&self, id: u32) -> f64 {
                match id {
                    0 => self.level as f64,
                    _ => 0.0,
                }
            }
            fn set_param(&mut self, id: u32, value: f64) {
                if id == 0 {
                    self.level = value.clamp(0.0, 2.0) as f32;
                }
            }
        }

        let mut inner = Graph::labeled(64, "inner");
        let gain_node = inner.add_node(Box::new(TestMonoGain { level: 1.0 }));
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

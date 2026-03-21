//! Graph-as-a-Processor: compose entire signal chains into a single [`Processor`] node.
//!
//! [`SubGraph`] wraps a [`Graph`] so it can be inserted as one node in a parent graph.
//! Parameters from internal nodes are aggregated and re-indexed through [`SubGraphBuilder`].

use crate::graph::{
    Graph, NodeId, ParamDescriptor, ParamGroup, PortIdx, ProcessContext, Processor, ProcessorInfo,
};

struct MappedParam {
    external_id: u32,
    node: NodeId,
    param_id: u32,
    desc: ParamDescriptor,
}

/// A [`Processor`] whose DSP is an entire [`Graph`].
///
/// Use [`SubGraph::builder`] to construct, wire internal nodes, and expose
/// selected parameters under new IDs and labels.
pub struct SubGraph {
    name: &'static str,
    graph: Graph,
    output_node: NodeId,
    num_outputs: u16,
    param_map: Vec<MappedParam>,
    groups: Vec<ParamGroup>,
}

impl SubGraph {
    pub fn builder(name: &'static str, block_size: usize) -> SubGraphBuilder {
        SubGraphBuilder {
            name,
            graph: Graph::new(block_size),
            output_node: 0,
            num_outputs: 1,
            param_map: Vec::new(),
            groups: Vec::new(),
            next_id: 0,
            next_group_id: 0,
        }
    }
}

impl Processor for SubGraph {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: self.name,
            audio_inputs: 0,
            audio_outputs: self.num_outputs,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.graph.process(ctx.frames, ctx.sample_rate, ctx.events);

        for port in 0..self.num_outputs {
            let src = self.graph.output_buffer(self.output_node, port);
            if (port as usize) < ctx.outputs.len() {
                ctx.outputs[port as usize][..ctx.frames].copy_from_slice(&src[..ctx.frames]);
            }
        }
    }

    fn reset(&mut self) {
        self.graph.reset();
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
            .map(|m| self.graph.node_param_value(m.node, m.param_id))
            .unwrap_or(0.0)
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if let Some(m) = self.param_map.iter().find(|m| m.external_id == id) {
            self.graph.set_node_param(m.node, m.param_id, value);
        }
    }
}

/// Incrementally wires an internal [`Graph`] and exposes parameters before
/// sealing it into a [`SubGraph`].
pub struct SubGraphBuilder {
    name: &'static str,
    graph: Graph,
    output_node: NodeId,
    num_outputs: u16,
    param_map: Vec<MappedParam>,
    groups: Vec<ParamGroup>,
    next_id: u32,
    next_group_id: u32,
}

impl SubGraphBuilder {
    pub fn add_node(&mut self, p: Box<dyn Processor>) -> NodeId {
        self.graph.add_node(p)
    }

    pub fn connect(&mut self, src: NodeId, src_port: PortIdx, dst: NodeId, dst_port: PortIdx) {
        self.graph.connect(src, src_port, dst, dst_port);
    }

    pub fn set_output(&mut self, node: NodeId, num_outputs: u16) {
        self.output_node = node;
        self.num_outputs = num_outputs;
    }

    /// Declare a parameter group for this SubGraph's exposed params.
    /// Returns the group ID to pass to [`expose_param_in_group`].
    pub fn add_group(&mut self, group: ParamGroup) -> u32 {
        let id = self.next_group_id;
        self.next_group_id += 1;
        self.groups.push(ParamGroup { id, ..group });
        id
    }

    /// Re-export an internal node's parameter under a new label.
    /// The internal node's group assignment is cleared.
    ///
    /// Returns the external parameter ID assigned (sequential from 0).
    pub fn expose_param(&mut self, node: NodeId, param_id: u32, label: &'static str) -> u32 {
        self.expose_param_inner(node, param_id, label, None)
    }

    /// Re-export a parameter and assign it to a SubGraph-level group.
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
        let descs = self.graph.node_params(node);
        let mut desc = descs
            .into_iter()
            .find(|d| d.id == param_id)
            .unwrap_or_else(|| panic!("param {param_id} not found on node {node}"));

        let ext_id = self.next_id;
        self.next_id += 1;
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

    pub fn build(self) -> SubGraph {
        SubGraph {
            name: self.name,
            graph: self.graph,
            output_node: self.output_node,
            num_outputs: self.num_outputs,
            param_map: self.param_map,
            groups: self.groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::gain::MonoGain;
    use crate::dsp::osc::{Oscillator, Waveform};

    #[test]
    fn subgraph_produces_output() {
        let mut b = SubGraph::builder("test", 64);
        let osc = b.add_node(Box::new(Oscillator::new(Waveform::Sine)));
        let gain = b.add_node(Box::new(MonoGain::new(0.5)));
        b.connect(osc, 0, gain, 0);
        b.set_output(gain, 1);
        b.expose_param(gain, 0, "Level");
        let mut sg = b.build();

        let mut out = vec![vec![0.0f32; 64]];
        let inputs: Vec<&[f32]> = vec![];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut out,
            frames: 64,
            sample_rate: 44100.0,
            events: &[],
        };
        sg.process(&mut ctx);

        let energy: f32 = out[0].iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "subgraph should produce non-silent output");
    }

    #[test]
    fn param_forwarding() {
        let mut b = SubGraph::builder("test", 64);
        let osc = b.add_node(Box::new(Oscillator::new(Waveform::Sine)));
        let gain = b.add_node(Box::new(MonoGain::new(1.0)));
        b.connect(osc, 0, gain, 0);
        b.set_output(gain, 1);
        let lvl = b.expose_param(gain, 0, "Level");
        let mut sg = b.build();

        assert!((sg.get_param(lvl) - 1.0).abs() < 1e-6);
        sg.set_param(lvl, 0.25);
        assert!((sg.get_param(lvl) - 0.25).abs() < 1e-6);
    }
}

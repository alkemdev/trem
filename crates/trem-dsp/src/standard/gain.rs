//! Level and panning utilities: mono-to-stereo pan, stereo pair gain, and simple mono gain.
//!
//! These are lightweight `Node` nodes for routing and level staging without extra dependencies.

use trem::graph::{Node, NodeInfo, ParamDescriptor, ParamFlags, ParamUnit, ProcessContext, Sig};

/// Gain + stereo pan block.
///
/// Input: 1 channel. Output: 2 channels (left, right).
/// Pan is in [-1, 1] where -1 = full left, 1 = full right.
pub struct Gain {
    pub level: f32,
    pub pan: f32,
}

impl Gain {
    /// Mono input scaled by `level`, centered pan (equal left/right).
    pub fn new(level: f32) -> Self {
        Self { level, pan: 0.0 }
    }

    /// Constant-power pan: `pan` in [-1, 1] maps full left to full right at the given `level`.
    pub fn with_pan(level: f32, pan: f32) -> Self {
        Self { level, pan }
    }
}

impl Node for Gain {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "gain",
            sig: Sig {
                inputs: 1,
                outputs: 2,
            },
            description: "Mono to stereo with level and pan",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let angle = (self.pan + 1.0) * 0.25 * std::f32::consts::PI;
        let gain_l = self.level * angle.cos();
        let gain_r = self.level * angle.sin();

        for i in 0..ctx.frames {
            let s = ctx.inputs[0][i];
            ctx.outputs[0][i] = s * gain_l;
            ctx.outputs[1][i] = s * gain_r;
        }
    }

    fn reset(&mut self) {}

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
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
            },
            ParamDescriptor {
                id: 1,
                name: "Pan",
                min: -1.0,
                max: 1.0,
                default: 0.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::BIPOLAR,
                step: 0.05,
                group: None,
                help: "",
            },
        ]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.level as f64,
            1 => self.pan as f64,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.level = value.clamp(0.0, 2.0) as f32,
            1 => self.pan = value.clamp(-1.0, 1.0) as f32,
            _ => {}
        }
    }
}

/// Stereo gain (2 in, 2 out).
pub struct StereoGain {
    pub level: f32,
}

impl StereoGain {
    /// Applies the same gain independently to left and right inputs.
    pub fn new(level: f32) -> Self {
        Self { level }
    }
}

impl Node for StereoGain {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "stereo_gain",
            sig: Sig::STEREO,
            description: "Stereo level control",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let lvl = self.level;
        for i in 0..ctx.frames {
            ctx.outputs[0][i] = ctx.inputs[0][i] * lvl;
            ctx.outputs[1][i] = ctx.inputs[1][i] * lvl;
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
        match id {
            0 => self.level = value.clamp(0.0, 2.0) as f32,
            _ => {}
        }
    }
}

/// Stereo panner (2 in, 2 out).
///
/// Constant-power pan law: `pan` in [-1, 1] crossfades between channels.
/// At 0 (center) both channels pass at unity. At -1, right channel is silent.
pub struct StereoPan {
    pub pan: f32,
}

impl StereoPan {
    /// Centre-panned by default; `pan` in [-1, 1].
    pub fn new(pan: f32) -> Self {
        Self { pan }
    }
}

impl Node for StereoPan {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "stereo_pan",
            sig: Sig::STEREO,
            description: "Stereo panning control",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let angle = (self.pan + 1.0) * 0.25 * std::f32::consts::PI;
        let gl = angle.cos();
        let gr = angle.sin();
        for i in 0..ctx.frames {
            ctx.outputs[0][i] = ctx.inputs[0][i] * gl;
            ctx.outputs[1][i] = ctx.inputs[1][i] * gr;
        }
    }

    fn reset(&mut self) {}

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            id: 0,
            name: "Pan",
            min: -1.0,
            max: 1.0,
            default: 0.0,
            unit: ParamUnit::Linear,
            flags: ParamFlags::BIPOLAR,
            step: 0.05,
            group: None,
            help: "",
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.pan as f64,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.pan = value.clamp(-1.0, 1.0) as f32,
            _ => {}
        }
    }
}

/// Simple mono gain (1 in, 1 out).
pub struct MonoGain {
    pub level: f32,
}

impl MonoGain {
    /// Single-channel multiply; simplest gain stage for one bus.
    pub fn new(level: f32) -> Self {
        Self { level }
    }
}

impl Node for MonoGain {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "mono_gain",
            sig: Sig::MONO,
            description: "Simple mono level control",
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
        match id {
            0 => self.level = value.clamp(0.0, 2.0) as f32,
            _ => {}
        }
    }
}

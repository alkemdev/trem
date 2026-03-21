//! Summing bus for multiple stereo sources into one stereo output with a master level control.

use crate::graph::{
    ParamDescriptor, ParamFlags, ParamUnit, ProcessContext, Processor, ProcessorInfo,
};

/// Stereo mixer — sums N stereo input pairs to one stereo output with level.
///
/// Inputs: 2*N channels (L0, R0, L1, R1, ...).
/// Outputs: 2 channels (L, R).
pub struct StereoMixer {
    pub input_pairs: u16,
    pub level: f32,
}

impl StereoMixer {
    /// `input_pairs` is the number of L/R input pairs (total inputs = 2 × pairs); output level defaults to 1.
    pub fn new(input_pairs: u16) -> Self {
        Self {
            input_pairs,
            level: 1.0,
        }
    }

    /// Same as [`Self::new`] but sets the post-sum gain applied to both channels.
    pub fn with_level(input_pairs: u16, level: f32) -> Self {
        Self { input_pairs, level }
    }
}

impl Processor for StereoMixer {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "stereo_mixer",
            audio_inputs: self.input_pairs * 2,
            audio_outputs: 2,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        for i in 0..ctx.frames {
            ctx.outputs[0][i] = 0.0;
            ctx.outputs[1][i] = 0.0;
        }
        for pair in 0..self.input_pairs as usize {
            let l_idx = pair * 2;
            let r_idx = pair * 2 + 1;
            if l_idx < ctx.inputs.len() {
                for i in 0..ctx.frames {
                    ctx.outputs[0][i] += ctx.inputs[l_idx][i];
                }
            }
            if r_idx < ctx.inputs.len() {
                for i in 0..ctx.frames {
                    ctx.outputs[1][i] += ctx.inputs[r_idx][i];
                }
            }
        }
        let lvl = self.level;
        for i in 0..ctx.frames {
            ctx.outputs[0][i] *= lvl;
            ctx.outputs[1][i] *= lvl;
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

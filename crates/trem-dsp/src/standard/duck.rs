//! Sidechain-aware stereo ducking: carrier L/R gated by a **mono** sidechain level.

use trem::graph::{
    GroupHint, Node, NodeInfo, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, PrepareEnv,
    PrepareError, ProcessContext, Sig,
};

/// Ducks stereo **carrier** (inputs 0–1) by the absolute level of **mono sidechain** (input 2).
///
/// Unlike [`crate::standard::dynamics::Compressor`], detection is only on the sidechain bus, so kicks or other keys
/// can pump a pad without the carrier self-triggering compression.
///
/// # Topology
///
/// Wire carrier L/R to ports 0–1 and a mono trigger (kick, pulse, etc.) to port 2.
///
/// # Examples
///
/// Registry tag: **`duk`**. See the `extreme_sidechain` example in the `trem` crate (`cargo run -p trem --example extreme_sidechain`).
pub struct SidechainDucker {
    env: f32,
    depth: f64,
    attack_ms: f64,
    release_ms: f64,
    /// Minimum linear gain while ducking (prevents complete silence if desired).
    floor: f64,
    attack_coeff: f32,
    release_coeff: f32,
}

impl SidechainDucker {
    /// Feed-forward ducking: `depth` ∈ [0, 1] scales how hard the sidechain pulls gain down.
    pub fn new(depth: f64, attack_ms: f64, release_ms: f64) -> Self {
        let mut s = Self {
            env: 0.0,
            depth: depth.clamp(0.0, 1.0),
            attack_ms: attack_ms.clamp(0.01, 500.0),
            release_ms: release_ms.clamp(1.0, 4000.0),
            floor: 0.02,
            attack_coeff: 0.0,
            release_coeff: 0.0,
        };
        s.set_time_constants(48_000.0);
        s
    }

    fn set_time_constants(&mut self, sample_rate: f64) {
        self.attack_coeff = (1.0 - (-1.0 / (self.attack_ms * 0.001 * sample_rate)).exp()) as f32;
        self.release_coeff = (1.0 - (-1.0 / (self.release_ms * 0.001 * sample_rate)).exp()) as f32;
    }
}

impl Node for SidechainDucker {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "sidechain_duck",
            sig: Sig {
                inputs: 3,
                outputs: 2,
            },
            description: "Stereo carrier ducked by mono sidechain (independent detector path)",
        }
    }

    fn prepare(&mut self, env: &PrepareEnv) -> Result<(), PrepareError> {
        if !env.sample_rate.is_finite() || env.sample_rate <= 0.0 {
            return Err(PrepareError("sample_rate must be positive".into()));
        }
        self.set_time_constants(env.sample_rate);
        Ok(())
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let depth = self.depth as f32;
        let floor = self.floor as f32;
        let sc = ctx.inputs[2];
        for i in 0..ctx.frames {
            let drive = sc[i].abs().min(1.0);
            let c = if drive > self.env {
                self.attack_coeff
            } else {
                self.release_coeff
            };
            self.env += c * (drive - self.env);

            let gain = (1.0 - depth * self.env).max(floor);
            ctx.outputs[0][i] = ctx.inputs[0][i] * gain;
            ctx.outputs[1][i] = ctx.inputs[1][i] * gain;
        }
    }

    fn reset(&mut self) {
        self.env = 0.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Depth",
                min: 0.0,
                max: 1.0,
                default: 0.85,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.01,
                group: Some(0),
                help: "How much the sidechain can reduce carrier gain (1 = strongest duck)",
            },
            ParamDescriptor {
                id: 1,
                name: "Attack",
                min: 0.01,
                max: 500.0,
                default: 0.25,
                unit: ParamUnit::Milliseconds,
                flags: ParamFlags::LOG_SCALE,
                step: 0.5,
                group: Some(0),
                help: "How fast the duck engages when sidechain rises",
            },
            ParamDescriptor {
                id: 2,
                name: "Release",
                min: 1.0,
                max: 4000.0,
                default: 95.0,
                unit: ParamUnit::Milliseconds,
                flags: ParamFlags::LOG_SCALE,
                step: 5.0,
                group: Some(0),
                help: "How fast gain recovers after the sidechain falls",
            },
            ParamDescriptor {
                id: 3,
                name: "Floor",
                min: 0.0,
                max: 1.0,
                default: 0.02,
                unit: ParamUnit::Linear,
                flags: ParamFlags::LOG_SCALE,
                step: 0.01,
                group: Some(0),
                help: "Minimum linear gain applied to the carrier (headroom while ducked)",
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Sidechain duck",
            hint: GroupHint::Level,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.depth,
            1 => self.attack_ms,
            2 => self.release_ms,
            3 => self.floor,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.depth = value.clamp(0.0, 1.0),
            1 => self.attack_ms = value.clamp(0.01, 500.0),
            2 => self.release_ms = value.clamp(1.0, 4000.0),
            3 => self.floor = value.clamp(0.0, 1.0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidechain_reduces_carrier() {
        let mut d = SidechainDucker::new(1.0, 0.5, 80.0);
        d.set_param(3, 0.0);
        let frames = 2048;
        let carrier = vec![0.5f32; frames];
        let silent_sc = vec![0.0f32; frames];
        let inputs: Vec<&[f32]> = vec![&carrier, &carrier, &silent_sc];
        let mut outputs = vec![vec![0.0f32; frames], vec![0.0f32; frames]];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames,
            sample_rate: 48_000.0,
            events: &[],
        };
        d.prepare(&PrepareEnv::new(frames, 48_000.0)).unwrap();
        d.process(&mut ctx);
        assert!(
            outputs[0][frames - 1] > 0.4,
            "no sidechain should pass carrier"
        );

        d.reset();
        let hot_sc = vec![1.0f32; frames];
        let inputs2: Vec<&[f32]> = vec![&carrier, &carrier, &hot_sc];
        let mut outputs2 = vec![vec![0.0f32; frames], vec![0.0f32; frames]];
        let mut ctx2 = ProcessContext {
            inputs: &inputs2,
            outputs: &mut outputs2,
            frames,
            sample_rate: 48_000.0,
            events: &[],
        };
        d.process(&mut ctx2);
        assert!(
            outputs2[0][frames - 1] < outputs[0][frames - 1] * 0.2,
            "strong sidechain should duck carrier"
        );
    }
}

//! Dynamics processors: stereo limiter and stereo compressor.
//!
//! Both operate on stereo pairs (2 in, 2 out) and use linked peak detection
//! so the stereo image is preserved. All time constants are in milliseconds.

use crate::graph::{
    GroupHint, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext, Processor,
    ProcessorInfo, Sig,
};

fn db_to_linear(db: f64) -> f64 {
    10.0f64.powf(db / 20.0)
}

fn linear_to_db(lin: f64) -> f64 {
    if lin > 1e-10 {
        20.0 * lin.log10()
    } else {
        -200.0
    }
}

/// Stereo brickwall limiter with adjustable ceiling and release.
///
/// Uses peak-hold + exponential release gain reduction. Linked L/R detection
/// prevents stereo image shift. Look-ahead is not implemented (zero latency).
pub struct Limiter {
    ceiling_db: f64,
    release_ms: f64,
    env: f64,
}

impl Limiter {
    pub fn new(ceiling_db: f64, release_ms: f64) -> Self {
        Self {
            ceiling_db: ceiling_db.clamp(-30.0, 0.0),
            release_ms: release_ms.clamp(1.0, 1000.0),
            env: 0.0,
        }
    }
}

impl Processor for Limiter {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "limiter",
            sig: Sig::STEREO,
            description: "Brickwall stereo limiter",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let ceiling = db_to_linear(self.ceiling_db) as f32;
        let release_coeff = (-1.0 / (self.release_ms * 0.001 * ctx.sample_rate)).exp();

        for i in 0..ctx.frames {
            let l = ctx.inputs[0][i];
            let r = ctx.inputs[1][i];
            let peak = l.abs().max(r.abs()) as f64;

            let target_gain = if peak > ceiling as f64 {
                ceiling as f64 / peak
            } else {
                1.0
            };

            if target_gain < self.env {
                self.env = target_gain; // instant attack
            } else {
                self.env = target_gain + release_coeff * (self.env - target_gain);
            }

            let g = self.env as f32;
            ctx.outputs[0][i] = l * g;
            ctx.outputs[1][i] = r * g;
        }
    }

    fn reset(&mut self) {
        self.env = 1.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Ceiling",
                min: -30.0,
                max: 0.0,
                default: -0.3,
                unit: ParamUnit::Decibels,
                flags: ParamFlags::NONE,
                step: 0.5,
                group: Some(0),
                help: "Maximum output level; peaks above are clamped",
            },
            ParamDescriptor {
                id: 1,
                name: "Release",
                min: 1.0,
                max: 1000.0,
                default: 100.0,
                unit: ParamUnit::Milliseconds,
                flags: ParamFlags::LOG_SCALE,
                step: 10.0,
                group: Some(0),
                help: "Time for gain to recover after limiting",
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Limiter",
            hint: GroupHint::Level,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.ceiling_db,
            1 => self.release_ms,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.ceiling_db = value.clamp(-30.0, 0.0),
            1 => self.release_ms = value.clamp(1.0, 1000.0),
            _ => {}
        }
    }
}

/// Stereo feed-forward compressor with threshold, ratio, attack, release,
/// and makeup gain.
///
/// Linked L/R peak detection, log-domain gain computation. Classic VCA-style
/// topology: detect -> compute gain -> smooth -> apply.
pub struct Compressor {
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    makeup_db: f64,
    env_db: f64,
}

impl Compressor {
    pub fn new(threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64) -> Self {
        Self {
            threshold_db: threshold_db.clamp(-60.0, 0.0),
            ratio: ratio.clamp(1.0, 20.0),
            attack_ms: attack_ms.clamp(0.1, 200.0),
            release_ms: release_ms.clamp(1.0, 2000.0),
            makeup_db: 0.0,
            env_db: 0.0,
        }
    }
}

impl Processor for Compressor {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "compressor",
            sig: Sig::STEREO,
            description: "Feed-forward stereo compressor",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let attack_coeff = (-1.0 / (self.attack_ms * 0.001 * ctx.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release_ms * 0.001 * ctx.sample_rate)).exp();
        let makeup_lin = db_to_linear(self.makeup_db) as f32;

        for i in 0..ctx.frames {
            let l = ctx.inputs[0][i];
            let r = ctx.inputs[1][i];
            let peak = l.abs().max(r.abs()) as f64;
            let peak_db = linear_to_db(peak);

            let over_db = (peak_db - self.threshold_db).max(0.0);
            let target_db = over_db * (1.0 - 1.0 / self.ratio);

            let coeff = if target_db > self.env_db {
                attack_coeff
            } else {
                release_coeff
            };
            self.env_db = target_db + coeff * (self.env_db - target_db);

            let gain = db_to_linear(-self.env_db) as f32 * makeup_lin;
            ctx.outputs[0][i] = l * gain;
            ctx.outputs[1][i] = r * gain;
        }
    }

    fn reset(&mut self) {
        self.env_db = 0.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Threshold",
                min: -60.0,
                max: 0.0,
                default: -18.0,
                unit: ParamUnit::Decibels,
                flags: ParamFlags::NONE,
                step: 1.0,
                group: Some(0),
                help: "Level above which compression begins",
            },
            ParamDescriptor {
                id: 1,
                name: "Ratio",
                min: 1.0,
                max: 20.0,
                default: 4.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::LOG_SCALE,
                step: 0.5,
                group: Some(0),
                help: "Compression ratio (e.g. 4:1 means 4 dB in = 1 dB out above threshold)",
            },
            ParamDescriptor {
                id: 2,
                name: "Attack",
                min: 0.1,
                max: 200.0,
                default: 10.0,
                unit: ParamUnit::Milliseconds,
                flags: ParamFlags::LOG_SCALE,
                step: 1.0,
                group: Some(0),
                help: "How fast the compressor reacts to peaks",
            },
            ParamDescriptor {
                id: 3,
                name: "Release",
                min: 1.0,
                max: 2000.0,
                default: 150.0,
                unit: ParamUnit::Milliseconds,
                flags: ParamFlags::LOG_SCALE,
                step: 10.0,
                group: Some(0),
                help: "How fast the compressor lets go after the signal drops",
            },
            ParamDescriptor {
                id: 4,
                name: "Makeup",
                min: 0.0,
                max: 30.0,
                default: 0.0,
                unit: ParamUnit::Decibels,
                flags: ParamFlags::NONE,
                step: 0.5,
                group: Some(0),
                help: "Gain applied after compression to restore loudness",
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Compressor",
            hint: GroupHint::Level,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.threshold_db,
            1 => self.ratio,
            2 => self.attack_ms,
            3 => self.release_ms,
            4 => self.makeup_db,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.threshold_db = value.clamp(-60.0, 0.0),
            1 => self.ratio = value.clamp(1.0, 20.0),
            2 => self.attack_ms = value.clamp(0.1, 200.0),
            3 => self.release_ms = value.clamp(1.0, 2000.0),
            4 => self.makeup_db = value.clamp(0.0, 30.0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo_sine(frames: usize, amplitude: f32) -> (Vec<f32>, Vec<f32>) {
        let mut l = vec![0.0f32; frames];
        let mut r = vec![0.0f32; frames];
        for i in 0..frames {
            let s = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin() * amplitude;
            l[i] = s;
            r[i] = s;
        }
        (l, r)
    }

    #[test]
    fn limiter_reduces_peaks() {
        let mut lim = Limiter::new(-6.0, 50.0);
        lim.reset();
        let (inp_l, inp_r) = stereo_sine(1024, 1.0);
        let mut out = vec![vec![0.0f32; 1024], vec![0.0f32; 1024]];
        let refs: Vec<&[f32]> = vec![&inp_l, &inp_r];
        lim.process(&mut ProcessContext {
            inputs: &refs,
            outputs: &mut out,
            frames: 1024,
            sample_rate: 44100.0,
            events: &[],
        });
        let ceiling_lin = db_to_linear(-6.0) as f32;
        let peak = out[0]
            .iter()
            .chain(out[1].iter())
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        assert!(
            peak <= ceiling_lin + 0.01,
            "limiter should cap at ceiling: peak={peak}, ceiling={ceiling_lin}"
        );
    }

    #[test]
    fn limiter_passes_quiet_signal() {
        let mut lim = Limiter::new(-0.3, 100.0);
        lim.reset();
        let (inp_l, inp_r) = stereo_sine(512, 0.1);
        let mut out = vec![vec![0.0f32; 512], vec![0.0f32; 512]];
        let refs: Vec<&[f32]> = vec![&inp_l, &inp_r];
        lim.process(&mut ProcessContext {
            inputs: &refs,
            outputs: &mut out,
            frames: 512,
            sample_rate: 44100.0,
            events: &[],
        });
        let diff: f32 = inp_l
            .iter()
            .zip(out[0].iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff < 0.01, "quiet signal should pass through unchanged");
    }

    #[test]
    fn limiter_params_roundtrip() {
        let mut lim = Limiter::new(-6.0, 50.0);
        lim.set_param(0, -12.0);
        lim.set_param(1, 200.0);
        assert!((lim.get_param(0) - (-12.0)).abs() < 1e-9);
        assert!((lim.get_param(1) - 200.0).abs() < 1e-9);
    }

    #[test]
    fn compressor_reduces_loud_signal() {
        let mut comp = Compressor::new(-12.0, 4.0, 1.0, 50.0);
        comp.reset();
        let (inp_l, inp_r) = stereo_sine(2048, 1.0);
        let mut out = vec![vec![0.0f32; 2048], vec![0.0f32; 2048]];
        let refs: Vec<&[f32]> = vec![&inp_l, &inp_r];
        comp.process(&mut ProcessContext {
            inputs: &refs,
            outputs: &mut out,
            frames: 2048,
            sample_rate: 44100.0,
            events: &[],
        });
        let in_energy: f32 = inp_l.iter().map(|s| s * s).sum();
        let out_energy: f32 = out[0].iter().map(|s| s * s).sum();
        assert!(
            out_energy < in_energy,
            "compressor should reduce loud signal energy: in={in_energy}, out={out_energy}"
        );
    }

    #[test]
    fn compressor_passes_quiet_signal() {
        let mut comp = Compressor::new(-6.0, 4.0, 10.0, 150.0);
        comp.reset();
        let (inp_l, inp_r) = stereo_sine(512, 0.01);
        let mut out = vec![vec![0.0f32; 512], vec![0.0f32; 512]];
        let refs: Vec<&[f32]> = vec![&inp_l, &inp_r];
        comp.process(&mut ProcessContext {
            inputs: &refs,
            outputs: &mut out,
            frames: 512,
            sample_rate: 44100.0,
            events: &[],
        });
        let diff: f32 = inp_l
            .iter()
            .zip(out[0].iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff < 0.05,
            "signal below threshold should pass through nearly unchanged"
        );
    }

    #[test]
    fn compressor_params_roundtrip() {
        let mut comp = Compressor::new(-18.0, 4.0, 10.0, 150.0);
        comp.set_param(0, -24.0);
        comp.set_param(1, 8.0);
        comp.set_param(2, 5.0);
        comp.set_param(3, 300.0);
        comp.set_param(4, 6.0);
        assert!((comp.get_param(0) - (-24.0)).abs() < 1e-9);
        assert!((comp.get_param(1) - 8.0).abs() < 1e-9);
        assert!((comp.get_param(2) - 5.0).abs() < 1e-9);
        assert!((comp.get_param(3) - 300.0).abs() < 1e-9);
        assert!((comp.get_param(4) - 6.0).abs() < 1e-9);
    }
}

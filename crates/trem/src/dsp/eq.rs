//! Three peaking bands per channel, cascaded for broad tone control on a stereo bus.

use crate::graph::{
    GroupHint, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext, Processor,
    ProcessorInfo, Sig,
};
use std::f64::consts::PI;

/// 3-band stereo parametric EQ. Each band is a peaking biquad filter.
///
/// Rust-native, self-describing parameters for automatic UI.
pub struct ParametricEq {
    bands: [EqBand; 3],
    state_l: [BiquadState; 3],
    state_r: [BiquadState; 3],
    sample_rate: f64,
    dirty: bool,
}

#[derive(Clone, Copy)]
struct EqBand {
    freq: f64,
    gain_db: f64,
    q: f64,
}

#[derive(Clone, Copy)]
struct BiquadCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadState {
    fn tick(&mut self, x: f64, c: &BiquadCoeffs) -> f64 {
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

fn compute_peaking(freq: f64, gain_db: f64, q: f64, sr: f64) -> BiquadCoeffs {
    let a = 10.0f64.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq / sr;
    let alpha = w0.sin() / (2.0 * q);

    let a0 = 1.0 + alpha / a;
    BiquadCoeffs {
        b0: (1.0 + alpha * a) / a0,
        b1: (-2.0 * w0.cos()) / a0,
        b2: (1.0 - alpha * a) / a0,
        a1: (-2.0 * w0.cos()) / a0,
        a2: (1.0 - alpha / a) / a0,
    }
}

impl ParametricEq {
    /// Flat response with default corner frequencies (~200 Hz, 1 kHz, 5 kHz) and unity gain on each peaking band.
    pub fn new() -> Self {
        Self {
            bands: [
                EqBand {
                    freq: 200.0,
                    gain_db: 0.0,
                    q: 0.707,
                },
                EqBand {
                    freq: 1000.0,
                    gain_db: 0.0,
                    q: 0.707,
                },
                EqBand {
                    freq: 5000.0,
                    gain_db: 0.0,
                    q: 0.707,
                },
            ],
            state_l: [BiquadState::default(); 3],
            state_r: [BiquadState::default(); 3],
            sample_rate: 44100.0,
            dirty: true,
        }
    }

    /// Same defaults as [`Self::new`] but sets the three band center frequencies before processing.
    pub fn with_bands(lo_freq: f64, mid_freq: f64, hi_freq: f64) -> Self {
        let mut eq = Self::new();
        eq.bands[0].freq = lo_freq;
        eq.bands[1].freq = mid_freq;
        eq.bands[2].freq = hi_freq;
        eq
    }

    fn coeffs(&self) -> [BiquadCoeffs; 3] {
        [
            compute_peaking(
                self.bands[0].freq,
                self.bands[0].gain_db,
                self.bands[0].q,
                self.sample_rate,
            ),
            compute_peaking(
                self.bands[1].freq,
                self.bands[1].gain_db,
                self.bands[1].q,
                self.sample_rate,
            ),
            compute_peaking(
                self.bands[2].freq,
                self.bands[2].gain_db,
                self.bands[2].q,
                self.sample_rate,
            ),
        ]
    }
}

impl Default for ParametricEq {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameter IDs: 3 bands x 3 params each = 9 params.
/// Band 0 (Lo):  0=freq, 1=gain, 2=Q
/// Band 1 (Mid): 3=freq, 4=gain, 5=Q
/// Band 2 (Hi):  6=freq, 7=gain, 8=Q
impl Processor for ParametricEq {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "eq",
            sig: Sig::STEREO,
            description: "3-band parametric equalizer",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        if self.sample_rate != ctx.sample_rate {
            self.sample_rate = ctx.sample_rate;
            self.dirty = true;
        }

        let coeffs = self.coeffs();

        for i in 0..ctx.frames {
            let mut l = ctx.inputs[0][i] as f64;
            let mut r = ctx.inputs[1][i] as f64;

            for band in 0..3 {
                l = self.state_l[band].tick(l, &coeffs[band]);
                r = self.state_r[band].tick(r, &coeffs[band]);
            }

            ctx.outputs[0][i] = l as f32;
            ctx.outputs[1][i] = r as f32;
        }

        self.dirty = false;
    }

    fn reset(&mut self) {
        for s in &mut self.state_l {
            s.reset();
        }
        for s in &mut self.state_r {
            s.reset();
        }
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        let mut p = Vec::with_capacity(9);
        for band in 0..3 {
            let base = band as u32 * 3;
            let gid = Some(band as u32);
            p.push(ParamDescriptor {
                id: base,
                name: match band {
                    0 => "Lo Freq",
                    1 => "Mid Freq",
                    _ => "Hi Freq",
                },
                min: 20.0,
                max: 20000.0,
                default: self.bands[band].freq,
                unit: ParamUnit::Hertz,
                flags: ParamFlags::LOG_SCALE,
                step: 50.0,
                group: gid,
                help: "",
            });
            p.push(ParamDescriptor {
                id: base + 1,
                name: match band {
                    0 => "Lo Gain",
                    1 => "Mid Gain",
                    _ => "Hi Gain",
                },
                min: -24.0,
                max: 24.0,
                default: 0.0,
                unit: ParamUnit::Decibels,
                flags: ParamFlags::BIPOLAR,
                step: 0.5,
                group: gid,
                help: "",
            });
            p.push(ParamDescriptor {
                id: base + 2,
                name: match band {
                    0 => "Lo Q",
                    1 => "Mid Q",
                    _ => "Hi Q",
                },
                min: 0.1,
                max: 10.0,
                default: 0.707,
                unit: ParamUnit::Linear,
                flags: ParamFlags::LOG_SCALE,
                step: 0.1,
                group: gid,
                help: "",
            });
        }
        p
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![
            ParamGroup {
                id: 0,
                name: "Lo Band",
                hint: GroupHint::Filter,
            },
            ParamGroup {
                id: 1,
                name: "Mid Band",
                hint: GroupHint::Filter,
            },
            ParamGroup {
                id: 2,
                name: "Hi Band",
                hint: GroupHint::Filter,
            },
        ]
    }

    fn get_param(&self, id: u32) -> f64 {
        let band = (id / 3) as usize;
        let field = id % 3;
        if band >= 3 {
            return 0.0;
        }
        match field {
            0 => self.bands[band].freq,
            1 => self.bands[band].gain_db,
            2 => self.bands[band].q,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        let band = (id / 3) as usize;
        let field = id % 3;
        if band >= 3 {
            return;
        }
        match field {
            0 => self.bands[band].freq = value.clamp(20.0, 20000.0),
            1 => self.bands[band].gain_db = value.clamp(-24.0, 24.0),
            2 => self.bands[band].q = value.clamp(0.1, 10.0),
            _ => {}
        }
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_eq_passes_signal() {
        let mut eq = ParametricEq::new();
        let input = vec![0.5f32; 512];
        let out_l = vec![0.0f32; 512];
        let out_r = vec![0.0f32; 512];
        let inputs: Vec<&[f32]> = vec![&input, &input];
        let mut outputs = vec![out_l, out_r];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames: 512,
            sample_rate: 44100.0,
            events: &[],
        };
        eq.process(&mut ctx);
        let tail_avg: f32 = outputs[0][256..].iter().sum::<f32>() / 256.0;
        assert!(
            (tail_avg - 0.5).abs() < 0.05,
            "flat EQ should pass signal: got {tail_avg}"
        );
    }

    #[test]
    fn eq_boost_increases_energy() {
        let mut eq = ParametricEq::new();
        eq.set_param(4, 12.0);

        let sr = 44100.0;
        let input: Vec<f32> = (0..4096)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / sr).sin() as f32 * 0.3)
            .collect();
        let out_l = vec![0.0f32; 4096];
        let out_r = vec![0.0f32; 4096];
        let inputs: Vec<&[f32]> = vec![&input, &input];
        let mut outputs = vec![out_l, out_r];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames: 4096,
            sample_rate: sr,
            events: &[],
        };
        eq.process(&mut ctx);

        let in_energy: f32 = input[2048..].iter().map(|s| s * s).sum();
        let out_energy: f32 = outputs[0][2048..].iter().map(|s| s * s).sum();
        assert!(
            out_energy > in_energy * 2.0,
            "12dB boost should significantly increase energy"
        );
    }

    #[test]
    fn eq_nine_params() {
        let eq = ParametricEq::new();
        assert_eq!(eq.params().len(), 9);
    }
}

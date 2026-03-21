//! 7-band graphic equalizer using cascaded biquad filters.
//!
//! Center frequencies: 100, 200, 400, 800, 1600, 3200, 6400 Hz (octave spacing).
//! Each band is a peaking EQ with ±12 dB gain.

use crate::graph::{
    GroupHint, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext, Processor,
    ProcessorInfo, Sig,
};

const NUM_BANDS: usize = 7;
const BAND_FREQS: [f64; NUM_BANDS] = [100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0];
const Q: f64 = 1.414;

#[derive(Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn set_peaking(&mut self, freq: f64, gain_db: f64, q: f64, sr: f64) {
        let a = 10.0f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * freq / sr;
        let sin_w = w0.sin();
        let cos_w = w0.cos();
        let alpha = sin_w / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w;
        let a2 = 1.0 - alpha / a;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    fn process_sample(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// 7-band graphic EQ, mono (1 in, 1 out).
pub struct GraphicEq {
    bands: [Biquad; NUM_BANDS],
    gains_db: [f64; NUM_BANDS],
    last_sr: f64,
}

impl GraphicEq {
    /// Flat EQ with all band gains at 0 dB; coefficients are computed on first `process()`.
    pub fn new() -> Self {
        Self {
            bands: [Biquad::new(); NUM_BANDS],
            gains_db: [0.0; NUM_BANDS],
            last_sr: 0.0,
        }
    }

    fn recalc(&mut self, sr: f64) {
        for (i, band) in self.bands.iter_mut().enumerate() {
            band.set_peaking(BAND_FREQS[i], self.gains_db[i], Q, sr);
        }
        self.last_sr = sr;
    }
}

impl Default for GraphicEq {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for GraphicEq {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "graphic_eq",
            sig: Sig::MONO,
            description: "7-band graphic equalizer",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        if (ctx.sample_rate - self.last_sr).abs() > 1.0 {
            self.recalc(ctx.sample_rate);
        }
        for i in 0..ctx.frames {
            let mut s = ctx.inputs[0][i] as f64;
            for band in &mut self.bands {
                s = band.process_sample(s);
            }
            ctx.outputs[0][i] = s as f32;
        }
    }

    fn reset(&mut self) {
        for b in &mut self.bands {
            b.reset();
        }
        self.last_sr = 0.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        let labels = [
            "100 Hz", "200 Hz", "400 Hz", "800 Hz", "1.6 kHz", "3.2 kHz", "6.4 kHz",
        ];
        labels
            .iter()
            .enumerate()
            .map(|(i, &name)| ParamDescriptor {
                id: i as u32,
                name,
                min: -12.0,
                max: 12.0,
                default: 0.0,
                unit: ParamUnit::Decibels,
                flags: ParamFlags::BIPOLAR,
                step: 0.5,
                group: Some(0),
                help: "",
            })
            .collect()
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Bands",
            hint: GroupHint::Filter,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        self.gains_db.get(id as usize).copied().unwrap_or(0.0)
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if let Some(g) = self.gains_db.get_mut(id as usize) {
            *g = value.clamp(-12.0, 12.0);
            self.last_sr = 0.0; // trigger recalc
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_eq_passes_signal() {
        let mut eq = GraphicEq::new();
        let input: Vec<f32> = (0..128).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut output = vec![vec![0.0f32; 128]];
        let inp_ref: &[f32] = &input;
        eq.process(&mut ProcessContext {
            inputs: &[inp_ref],
            outputs: &mut output,
            frames: 128,
            sample_rate: 44100.0,
            events: &[],
        });
        let energy: f32 = output[0].iter().map(|s| s * s).sum();
        assert!(energy > 0.0);
    }

    #[test]
    fn boost_increases_energy() {
        let mut eq_flat = GraphicEq::new();
        let mut eq_boost = GraphicEq::new();
        for i in 0..NUM_BANDS {
            eq_boost.set_param(i as u32, 12.0);
        }

        let input: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.05).sin()).collect();
        let inp_ref: &[f32] = &input;

        let mut out_flat = vec![vec![0.0f32; 1024]];
        let mut out_boost = vec![vec![0.0f32; 1024]];

        eq_flat.process(&mut ProcessContext {
            inputs: &[inp_ref],
            outputs: &mut out_flat,
            frames: 1024,
            sample_rate: 44100.0,
            events: &[],
        });
        eq_boost.process(&mut ProcessContext {
            inputs: &[inp_ref],
            outputs: &mut out_boost,
            frames: 1024,
            sample_rate: 44100.0,
            events: &[],
        });

        let e_flat: f32 = out_flat[0].iter().map(|s| s * s).sum();
        let e_boost: f32 = out_boost[0].iter().map(|s| s * s).sum();
        assert!(e_boost > e_flat, "boosted EQ should have more energy");
    }

    #[test]
    fn seven_band_params() {
        let eq = GraphicEq::new();
        assert_eq!(eq.params().len(), NUM_BANDS);
    }
}

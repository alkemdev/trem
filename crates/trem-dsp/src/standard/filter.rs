//! Second-order IIR (biquad) filtering for gentle tone shaping on a single channel.
//!
//! Coefficients follow the host sample rate; [`BiquadFilter`] recomputes when the rate changes.

use std::f64::consts::PI;
use trem::graph::{
    GroupHint, Node, NodeInfo, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext,
    Sig,
};

/// Which spectral region the biquad emphasizes or passes; drives coefficient choice in [`BiquadFilter::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    /// Attenuates content above the cutoff, keeping lows.
    LowPass,
    /// Attenuates content below the cutoff, keeping highs.
    HighPass,
    /// Narrow band around the center frequency; `q` controls bandwidth.
    BandPass,
}

/// Biquad filter (2nd order IIR).
pub struct BiquadFilter {
    pub filter_type: FilterType,
    pub frequency: f64,
    pub q: f64,

    // Coefficients
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,

    // State
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,

    sample_rate: f64,
    dirty: bool,
}

impl BiquadFilter {
    /// Builds a filter at `frequency` (Hz) with resonance/bandwidth `q`; state starts zeroed for a clean tail.
    pub fn new(filter_type: FilterType, frequency: f64, q: f64) -> Self {
        let mut f = Self {
            filter_type,
            frequency,
            q,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            sample_rate: 44100.0,
            dirty: true,
        };
        f.compute_coefficients();
        f
    }

    fn compute_coefficients(&mut self) {
        let w0 = 2.0 * PI * self.frequency / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.q);

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::LowPass => {
                let b1 = 1.0 - cos_w0;
                let b0 = b1 / 2.0;
                let b2 = b0;
                (b0, b1, b2, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
            }
            FilterType::HighPass => {
                let b1 = -(1.0 + cos_w0);
                let b0 = (1.0 + cos_w0) / 2.0;
                let b2 = b0;
                (b0, b1, b2, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
            }
            FilterType::BandPass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                (b0, b1, b2, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
            }
        };

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
        self.dirty = false;
    }

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

impl Node for BiquadFilter {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "biquad",
            sig: Sig::MONO,
            description: "Resonant biquad filter with selectable type",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        if self.sample_rate != ctx.sample_rate {
            self.sample_rate = ctx.sample_rate;
            self.dirty = true;
        }
        if self.dirty {
            self.compute_coefficients();
        }

        for i in 0..ctx.frames {
            let x = ctx.inputs[0][i] as f64;
            ctx.outputs[0][i] = self.tick(x) as f32;
        }
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Cutoff",
                min: 20.0,
                max: 20000.0,
                default: 1000.0,
                unit: ParamUnit::Hertz,
                flags: ParamFlags::LOG_SCALE,
                step: 50.0,
                group: Some(0),
                help: "",
            },
            ParamDescriptor {
                id: 1,
                name: "Resonance",
                min: 0.1,
                max: 20.0,
                default: 0.707,
                unit: ParamUnit::Linear,
                flags: ParamFlags::LOG_SCALE,
                step: 0.1,
                group: Some(0),
                help: "",
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Filter",
            hint: GroupHint::Filter,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.frequency,
            1 => self.q,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => {
                self.frequency = value.clamp(20.0, 20000.0);
                self.dirty = true;
            }
            1 => {
                self.q = value.clamp(0.1, 20.0);
                self.dirty = true;
            }
            _ => {}
        }
    }
}

/// Mono low-pass biquad whose cutoff wobbles with an internal sine LFO (musical movement).
pub struct ModulatedLowPass {
    pub base_cutoff: f64,
    pub q: f64,
    pub lfo_rate_hz: f64,
    /// Peak cutoff deviation in Hz (LFO is bipolar; actual range is base ± depth).
    pub lfo_depth_hz: f64,
    lfo_phase: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
    sample_rate: f64,
    coeff_freq: f64,
    coeff_q: f64,
}

impl ModulatedLowPass {
    /// Low-pass at `base_cutoff` Hz with resonance `q`, sine LFO at `lfo_rate_hz` moving cutoff by up to `lfo_depth_hz`.
    pub fn new(base_cutoff: f64, q: f64, lfo_rate_hz: f64, lfo_depth_hz: f64) -> Self {
        let mut s = Self {
            base_cutoff: base_cutoff.clamp(20.0, 20000.0),
            q: q.clamp(0.1, 20.0),
            lfo_rate_hz: lfo_rate_hz.clamp(0.01, 40.0),
            lfo_depth_hz: lfo_depth_hz.clamp(0.0, 8000.0),
            lfo_phase: 0.0,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            sample_rate: 44100.0,
            coeff_freq: -1.0,
            coeff_q: -1.0,
        };
        s.recompute_at_freq(s.base_cutoff);
        s
    }

    fn recompute_at_freq(&mut self, freq: f64) {
        let f = freq.clamp(20.0, 20000.0);
        if (f - self.coeff_freq).abs() < 0.5 && (self.q - self.coeff_q).abs() < 1e-6 {
            return;
        }
        self.coeff_freq = f;
        self.coeff_q = self.q;
        let w0 = 2.0 * PI * f / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.q);
        let b1 = 1.0 - cos_w0;
        let b0 = b1 / 2.0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1c = -2.0 * cos_w0;
        let a2c = 1.0 - alpha;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1c / a0;
        self.a2 = a2c / a0;
    }

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

impl Node for ModulatedLowPass {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "mod_lp",
            sig: Sig::MONO,
            description: "Low-pass with sine LFO on cutoff",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        if self.sample_rate != ctx.sample_rate {
            self.sample_rate = ctx.sample_rate;
            self.coeff_freq = -1.0;
        }
        let inc = self.lfo_rate_hz / ctx.sample_rate;

        for i in 0..ctx.frames {
            let lfo = (self.lfo_phase * std::f64::consts::TAU).sin();
            self.lfo_phase += inc;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }
            let freq = (self.base_cutoff + self.lfo_depth_hz * lfo).clamp(20.0, 20000.0);
            self.recompute_at_freq(freq);

            let x = ctx.inputs[0][i] as f64;
            ctx.outputs[0][i] = self.tick(x) as f32;
        }
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
        self.lfo_phase = 0.0;
        self.coeff_freq = -1.0;
        self.recompute_at_freq(self.base_cutoff);
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Cutoff",
                min: 20.0,
                max: 20000.0,
                default: 2000.0,
                unit: ParamUnit::Hertz,
                flags: ParamFlags::LOG_SCALE,
                step: 50.0,
                group: Some(0),
                help: "Center frequency of the low-pass before LFO offset",
            },
            ParamDescriptor {
                id: 1,
                name: "Resonance",
                min: 0.1,
                max: 20.0,
                default: 1.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::LOG_SCALE,
                step: 0.1,
                group: Some(0),
                help: "",
            },
            ParamDescriptor {
                id: 2,
                name: "LFO Rate",
                min: 0.02,
                max: 12.0,
                default: 0.35,
                unit: ParamUnit::Hertz,
                flags: ParamFlags::LOG_SCALE,
                step: 0.05,
                group: Some(1),
                help: "Speed of filter cutoff modulation",
            },
            ParamDescriptor {
                id: 3,
                name: "LFO Depth",
                min: 0.0,
                max: 4000.0,
                default: 400.0,
                unit: ParamUnit::Hertz,
                flags: ParamFlags::NONE,
                step: 25.0,
                group: Some(1),
                help: "How far the cutoff swings in Hz",
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![
            ParamGroup {
                id: 0,
                name: "Filter",
                hint: GroupHint::Filter,
            },
            ParamGroup {
                id: 1,
                name: "Modulation",
                hint: GroupHint::Generic,
            },
        ]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.base_cutoff,
            1 => self.q,
            2 => self.lfo_rate_hz,
            3 => self.lfo_depth_hz,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => {
                self.base_cutoff = value.clamp(20.0, 20000.0);
                self.coeff_freq = -1.0;
            }
            1 => {
                self.q = value.clamp(0.1, 20.0);
                self.coeff_freq = -1.0;
            }
            2 => self.lfo_rate_hz = value.clamp(0.02, 12.0),
            3 => self.lfo_depth_hz = value.clamp(0.0, 4000.0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod mod_lp_tests {
    use super::*;

    #[test]
    fn modulated_lowpass_outputs_signal() {
        let mut f = ModulatedLowPass::new(1500.0, 1.2, 0.5, 300.0);
        let input = vec![0.1f32; 256];
        let mut output = vec![vec![0.0f32; 256]];
        let inputs: Vec<&[f32]> = vec![&input];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut output,
            frames: 256,
            sample_rate: 44100.0,
            events: &[],
        };
        f.process(&mut ctx);
        let peak = output[0].iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.001);
    }

    #[test]
    fn modulated_lowpass_param_roundtrip() {
        let mut f = ModulatedLowPass::new(1000.0, 1.0, 0.2, 200.0);
        f.set_param(2, 2.5);
        f.set_param(3, 800.0);
        assert!((f.get_param(2) - 2.5).abs() < 1e-6);
        assert!((f.get_param(3) - 800.0).abs() < 1e-6);
    }
}

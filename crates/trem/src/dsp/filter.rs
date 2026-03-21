//! Second-order IIR (biquad) filtering for gentle tone shaping on a single channel.
//!
//! Coefficients follow the host sample rate; [`BiquadFilter`] recomputes when the rate changes.

use crate::graph::{ProcessContext, Processor, ProcessorInfo};
use std::f64::consts::PI;

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

impl Processor for BiquadFilter {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "biquad",
            audio_inputs: 1,
            audio_outputs: 1,
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
}

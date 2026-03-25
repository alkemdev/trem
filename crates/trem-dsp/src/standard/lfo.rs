//! Low-frequency oscillator for modulation: outputs a control signal (0→1 mono).
//!
//! Shapes: sine, triangle, saw-up, saw-down, square, sample-and-hold.

use trem::graph::{Node, NodeInfo, ParamDescriptor, ParamFlags, ParamUnit, ProcessContext, Sig};

/// LFO waveform shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LfoShape {
    /// Smooth sinusoidal modulation.
    Sine = 0,
    /// Linear rise/fall, sharper than sine.
    Triangle = 1,
    /// Ramp from 0 to 1, then reset.
    SawUp = 2,
    /// Ramp from 1 to 0, then reset.
    SawDown = 3,
    /// Alternates between 0 and 1 at the LFO rate.
    Square = 4,
    /// Random value held until the next cycle.
    SampleHold = 5,
}

impl LfoShape {
    fn from_param(v: f64) -> Self {
        match v.round() as u8 {
            0 => Self::Sine,
            1 => Self::Triangle,
            2 => Self::SawUp,
            3 => Self::SawDown,
            4 => Self::Square,
            5 => Self::SampleHold,
            _ => Self::Sine,
        }
    }
}

/// Low-frequency oscillator. Outputs a unipolar [0, 1] signal by default;
/// set depth to go bipolar [-depth, +depth] for modulation targets.
pub struct Lfo {
    phase: f64,
    rate: f64,
    depth: f64,
    shape: LfoShape,
    sh_value: f32,
    sh_last_phase: f64,
}

impl Lfo {
    /// Sine LFO at `rate` Hz with full depth (1.0).
    pub fn new(rate: f64) -> Self {
        Self {
            phase: 0.0,
            rate,
            depth: 1.0,
            shape: LfoShape::Sine,
            sh_value: 0.0,
            sh_last_phase: 0.0,
        }
    }

    fn sample_shape(&self, p: f64) -> f32 {
        match self.shape {
            LfoShape::Sine => ((p * std::f64::consts::TAU).sin() * 0.5 + 0.5) as f32,
            LfoShape::Triangle => {
                let t = (p * 2.0 - 1.0).abs();
                t as f32
            }
            LfoShape::SawUp => p as f32,
            LfoShape::SawDown => (1.0 - p) as f32,
            LfoShape::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            LfoShape::SampleHold => self.sh_value,
        }
    }
}

impl Node for Lfo {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "lfo",
            sig: Sig::SOURCE1,
            description: "Low-frequency oscillator for modulation",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let inc = self.rate / ctx.sample_rate;
        let depth = self.depth as f32;

        for i in 0..ctx.frames {
            if self.shape == LfoShape::SampleHold && self.phase < self.sh_last_phase {
                // Phase wrapped around — pick new random value (deterministic LCG)
                let bits = (self.phase * 1e9) as u32;
                self.sh_value =
                    (bits.wrapping_mul(1103515245).wrapping_add(12345) >> 16) as f32 / 32768.0;
            }
            self.sh_last_phase = self.phase;

            let raw = self.sample_shape(self.phase);
            ctx.outputs[0][i] = raw * depth;

            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.sh_value = 0.0;
        self.sh_last_phase = 0.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Rate",
                min: 0.01,
                max: 50.0,
                default: 1.0,
                unit: ParamUnit::Hertz,
                flags: ParamFlags::LOG_SCALE,
                step: 0.1,
                group: None,
                help: "",
            },
            ParamDescriptor {
                id: 1,
                name: "Depth",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: None,
                help: "",
            },
            ParamDescriptor {
                id: 2,
                name: "Shape",
                min: 0.0,
                max: 5.0,
                default: 0.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 1.0,
                group: None,
                help: "",
            },
        ]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.rate,
            1 => self.depth,
            2 => self.shape as u8 as f64,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.rate = value.clamp(0.01, 50.0),
            1 => self.depth = value.clamp(0.0, 1.0),
            2 => self.shape = LfoShape::from_param(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_lfo_output_range() {
        let mut lfo = Lfo::new(1.0);
        let mut out = vec![vec![0.0f32; 44100]];
        lfo.process(&mut ProcessContext {
            inputs: &[],
            outputs: &mut out,
            frames: 44100,
            sample_rate: 44100.0,
            events: &[],
        });
        for &s in &out[0] {
            assert!(s >= -0.001 && s <= 1.001, "out of range: {s}");
        }
    }

    #[test]
    fn square_lfo_binary() {
        let mut lfo = Lfo::new(1.0);
        lfo.set_param(2, 4.0); // Square
        let mut out = vec![vec![0.0f32; 44100]];
        lfo.process(&mut ProcessContext {
            inputs: &[],
            outputs: &mut out,
            frames: 44100,
            sample_rate: 44100.0,
            events: &[],
        });
        for &s in &out[0] {
            assert!(s == 0.0 || s == 1.0, "square should be 0 or 1, got {s}");
        }
    }

    #[test]
    fn params_roundtrip() {
        let mut lfo = Lfo::new(1.0);
        lfo.set_param(0, 5.0);
        lfo.set_param(1, 0.5);
        lfo.set_param(2, 3.0);
        assert!((lfo.get_param(0) - 5.0).abs() < 1e-9);
        assert!((lfo.get_param(1) - 0.5).abs() < 1e-9);
        assert!((lfo.get_param(2) - 3.0).abs() < 1e-9);
    }
}

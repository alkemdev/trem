//! Mono waveshaping distortion: several mathematically distinct curves (tanh, foldback,
//! rational soft-clip, hard clip, asymmetric “diode” blend) with dry/wet mix and makeup gain.

use trem::graph::{Node, NodeInfo, ParamDescriptor, ParamFlags, ParamUnit, ProcessContext, Sig};

/// Distortion / waveshaper: mono in → mono out.
///
/// Modes differ in how the driven sample \(y = \texttt{drive}\cdot x\) is mapped into \([-1,1]\):
/// **Tanh**, **Hard** clip, **Fold** (iterated foldback), **Soft** rational \(\frac{y}{\sqrt{1+y^2}}\),
/// **Diode** (asymmetric half-wave emphasis + tanh).
#[derive(Clone, Debug)]
pub struct Distortion {
    /// Mode index 0..=4 (see [`Distortion::shape`]).
    pub mode: u32,
    /// Linear gain into the nonlinearity; higher = more saturation.
    pub drive: f32,
    /// Wet amount: 0 = dry, 1 = full wet.
    pub mix: f32,
    /// Makeup gain after mix (compensates level loss when clipping).
    pub output: f32,
}

impl Distortion {
    /// Neutral defaults (light touch).
    pub fn new() -> Self {
        Self {
            mode: 0,
            drive: 2.0,
            mix: 0.5,
            output: 1.0,
        }
    }

    /// Snare-oriented preset: lighter fold + more dry — less hash feeding delay/reverb.
    pub fn snare_default() -> Self {
        Self {
            mode: 2,
            drive: 4.2,
            mix: 0.48,
            output: 0.95,
        }
    }

    fn shape(&self, x: f32) -> f32 {
        let y = self.drive * x;
        match self.mode {
            0 => y.tanh(),
            1 => y.clamp(-1.0, 1.0),
            2 => foldback(y),
            3 => soft_rational(y),
            4 => diode_asym(y),
            _ => y.tanh(),
        }
    }
}

/// Iterated foldback into \([-1,1]\): mirrors energy back when \(|y|>1\) (up to a few bounces).
#[inline]
fn foldback(mut y: f32) -> f32 {
    for _ in 0..12 {
        if y > 1.0 {
            y = 2.0 - y;
        } else if y < -1.0 {
            y = -2.0 - y;
        } else {
            break;
        }
    }
    y.clamp(-1.0, 1.0)
}

/// Smooth saturation: \(y / \sqrt{1 + y^2}\) — no hard corners, stays in \((-1,1)\).
#[inline]
fn soft_rational(y: f32) -> f32 {
    y / (1.0 + y * y).sqrt()
}

/// Asymmetric curve: emphasizes positive excursions (rectifier-ish) then bounded via tanh.
#[inline]
fn diode_asym(y: f32) -> f32 {
    let pos = y.max(0.0);
    let neg = y.min(0.0);
    (pos * 1.35 + neg * 0.65).tanh()
}

impl Default for Distortion {
    fn default() -> Self {
        Self::new()
    }
}

impl Node for Distortion {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "distort",
            sig: Sig::MONO,
            description: "Mono waveshaper: tanh / hard / fold / soft / diode + mix",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let in_ = ctx.inputs[0];
        let out = &mut ctx.outputs[0];
        for i in 0..ctx.frames {
            let x = in_[i];
            let w = self.wet_sample(x);
            out[i] = (x * (1.0 - self.mix) + w * self.mix) * self.output;
        }
    }

    fn reset(&mut self) {}

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Mode",
                min: 0.0,
                max: 4.0,
                default: 0.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 1.0,
                group: None,
                help: "0=Tanh 1=Hard 2=Fold 3=Soft 4=Diode",
            },
            ParamDescriptor {
                id: 1,
                name: "Drive",
                min: 0.25,
                max: 24.0,
                default: 2.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::LOG_SCALE,
                step: 0.25,
                group: None,
                help: "Gain into the waveshaper",
            },
            ParamDescriptor {
                id: 2,
                name: "Mix",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.02,
                group: None,
                help: "Dry/wet blend",
            },
            ParamDescriptor {
                id: 3,
                name: "Out",
                min: 0.1,
                max: 3.0,
                default: 1.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: None,
                help: "Makeup gain after shaping",
            },
        ]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.mode as f64,
            1 => self.drive as f64,
            2 => self.mix as f64,
            3 => self.output as f64,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.mode = value.round().clamp(0.0, 4.0) as u32,
            1 => self.drive = value.clamp(0.25, 24.0) as f32,
            2 => self.mix = value.clamp(0.0, 1.0) as f32,
            3 => self.output = value.clamp(0.1, 3.0) as f32,
            _ => {}
        }
    }
}

impl Distortion {
    #[inline]
    fn wet_sample(&self, x: f32) -> f32 {
        self.shape(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trem::graph::Graph;

    #[test]
    fn distortion_output_bounded() {
        let mut d = Distortion {
            mode: 1,
            drive: 50.0,
            mix: 1.0,
            output: 1.0,
        };
        let input = vec![0.9f32; 8];
        let mut outputs_vec = vec![vec![0.0f32; 8]];
        let inputs: Vec<&[f32]> = vec![&input];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs_vec,
            frames: 8,
            sample_rate: 44100.0,
            events: &[],
        };
        d.process(&mut ctx);
        for &s in &outputs_vec[0] {
            assert!(s.abs() <= 1.01, "hard+full wet should clip near unit: {s}");
        }
    }

    #[test]
    fn snare_default_runs_in_graph() {
        let mut g = Graph::new(64);
        let n = g.add_node(Box::new(Distortion::snare_default()));
        g.run(64, 44100.0, &[]).unwrap();
        let buf = g.output_buffer(n, 0);
        assert_eq!(buf.len(), 64);
    }
}

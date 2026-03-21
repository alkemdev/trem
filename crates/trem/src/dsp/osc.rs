use crate::event::GraphEvent;
use crate::graph::{ProcessContext, Processor, ProcessorInfo};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

/// Band-limited oscillator using polyBLEP for alias reduction.
pub struct Oscillator {
    pub waveform: Waveform,
    pub frequency: f64,
    pub voice_id: Option<u32>,
    phase: f64,
    sample_rate: f64,
}

impl Oscillator {
    pub fn new(waveform: Waveform) -> Self {
        Self {
            waveform,
            frequency: 440.0,
            voice_id: None,
            phase: 0.0,
            sample_rate: 44100.0,
        }
    }

    pub fn with_voice(mut self, id: u32) -> Self {
        self.voice_id = Some(id);
        self
    }

    /// PolyBLEP correction for discontinuities.
    fn poly_blep(t: f64, dt: f64) -> f64 {
        if t < dt {
            let t = t / dt;
            2.0 * t - t * t - 1.0
        } else if t > 1.0 - dt {
            let t = (t - 1.0) / dt;
            t * t + 2.0 * t + 1.0
        } else {
            0.0
        }
    }

    fn generate_sample(&mut self) -> f32 {
        let dt = self.frequency / self.sample_rate;
        let p = self.phase;

        let sample = match self.waveform {
            Waveform::Sine => (2.0 * PI * p).sin(),
            Waveform::Saw => {
                let naive = 2.0 * p - 1.0;
                naive - Self::poly_blep(p, dt)
            }
            Waveform::Square => {
                let naive = if p < 0.5 { 1.0 } else { -1.0 };
                let mut s = naive;
                s += Self::poly_blep(p, dt);
                s -= Self::poly_blep((p + 0.5) % 1.0, dt);
                s
            }
            Waveform::Triangle => {
                // Integrated square wave
                let naive = if p < 0.5 { 1.0 } else { -1.0 };
                let mut sq = naive;
                sq += Self::poly_blep(p, dt);
                sq -= Self::poly_blep((p + 0.5) % 1.0, dt);
                // Leaky integrator to form triangle from square
                sq * 4.0 * dt
            }
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample as f32
    }
}

impl Processor for Oscillator {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "oscillator",
            audio_inputs: 0,
            audio_outputs: 1,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.sample_rate = ctx.sample_rate;

        for te in ctx.events {
            if te.sample_offset < ctx.frames {
                if let GraphEvent::NoteOn {
                    frequency, voice, ..
                } = te.event
                {
                    if self.voice_id.is_none() || self.voice_id == Some(voice) {
                        self.frequency = frequency;
                    }
                }
            }
        }

        for i in 0..ctx.frames {
            ctx.outputs[0][i] = self.generate_sample();
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_output_range() {
        let mut osc = Oscillator::new(Waveform::Sine);
        osc.sample_rate = 44100.0;
        osc.frequency = 440.0;

        let mut ctx_outputs = vec![vec![0.0f32; 1024]];
        let inputs: Vec<&[f32]> = vec![];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut ctx_outputs,
            frames: 1024,
            sample_rate: 44100.0,
            events: &[],
        };
        osc.process(&mut ctx);

        for &s in &ctx_outputs[0] {
            assert!(s >= -1.0 && s <= 1.0, "sample out of range: {s}");
        }
    }
}

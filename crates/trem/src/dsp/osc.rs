//! Band-limited analog-style waveforms and a graph [`Oscillator`] that follows note events.
//!
//! PolyBLEP reduces aliasing on discontinuous shapes so pitched sources stay cleaner at high frequencies.

use crate::event::GraphEvent;
use crate::graph::{
    ParamDescriptor, ParamFlags, ParamUnit, ProcessContext, Processor, ProcessorInfo,
};
use std::f64::consts::PI;

/// Basic periodic shape emitted by [`Oscillator`]; each variant uses the same phase accumulator
/// but different band-limiting or integration (triangle is derived from a corrected square).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waveform {
    /// Smooth sinusoid; no BLEP needed.
    Sine,
    /// Rising ramp with falling discontinuity removed via polyBLEP.
    Saw,
    /// Alternating ±1 with edges softened by polyBLEP at both transitions.
    Square,
    /// Integrated square through a leaky integrator for a rounded triangle character.
    Triangle,
}

/// Band-limited oscillator using polyBLEP for alias reduction.
pub struct Oscillator {
    pub waveform: Waveform,
    pub frequency: f64,
    pub detune: f64,
    pub voice_id: Option<u32>,
    phase: f64,
    sample_rate: f64,
}

impl Oscillator {
    /// Builds an oscillator at 440 Hz, default sample rate, with no voice filter (responds to all notes).
    pub fn new(waveform: Waveform) -> Self {
        Self {
            waveform,
            frequency: 440.0,
            detune: 0.0,
            voice_id: None,
            phase: 0.0,
            sample_rate: 44100.0,
        }
    }

    /// Restricts frequency updates to `NoteOn` events whose voice matches `id`, for polyphonic graphs.
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
        let freq = self.frequency * 2.0_f64.powf(self.detune / 12.0);
        let dt = freq / self.sample_rate;
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

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            id: 0,
            name: "Detune",
            min: -24.0,
            max: 24.0,
            default: 0.0,
            unit: ParamUnit::Semitones,
            flags: ParamFlags::BIPOLAR,
            step: 0.1,
            group: None,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.detune,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.detune = value.clamp(-24.0, 24.0),
            _ => {}
        }
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

//! Event-driven drum voices: kick, snare, and hat synthesizers wired to `NoteOn` per voice id.
//!
//! Each voice is self-contained (no audio inputs); trigger velocity scales excitation and decay character.

use crate::event::GraphEvent;
use crate::graph::{ProcessContext, Processor, ProcessorInfo};
use std::f64::consts::PI;

// --- Shared helpers ---

fn exp_decay(level: f64, rate: f64, sr: f64) -> f64 {
    level * (-rate / sr).exp()
}

fn noise_sample(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state as f32 / u32::MAX as f32) * 2.0 - 1.0
}

// =====================================================================
// KickSynth — sine oscillator with pitch sweep + amplitude envelope
// =====================================================================

/// Sine body whose pitch glides from a hit-dependent high toward a low fundamental while amplitude decays exponentially.
pub struct KickSynth {
    pub voice_id: u32,
    phase: f64,
    freq: f64,
    freq_target: f64,
    freq_decay: f64,
    amp: f64,
    amp_decay: f64,
    sample_rate: f64,
}

impl KickSynth {
    /// Drum voice listening for `NoteOn` on `voice_id`; internal pitch and amp envelopes define the default kick shape.
    pub fn new(voice_id: u32) -> Self {
        Self {
            voice_id,
            phase: 0.0,
            freq: 50.0,
            freq_target: 50.0,
            freq_decay: 30.0,
            amp: 0.0,
            amp_decay: 8.0,
            sample_rate: 44100.0,
        }
    }

    fn trigger(&mut self, velocity: f64) {
        self.freq = 150.0 * velocity.max(0.3);
        self.amp = velocity;
        self.phase = 0.0;
    }

    fn tick(&mut self) -> f32 {
        if self.amp < 1e-6 {
            return 0.0;
        }
        let sample = (2.0 * PI * self.phase).sin() * self.amp;
        self.phase += self.freq / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        self.freq = self.freq_target
            + (self.freq - self.freq_target) * exp_decay(1.0, self.freq_decay, self.sample_rate);
        self.amp = exp_decay(self.amp, self.amp_decay, self.sample_rate);
        sample as f32
    }
}

impl Processor for KickSynth {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "kick",
            audio_inputs: 0,
            audio_outputs: 1,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.sample_rate = ctx.sample_rate;
        let mut event_idx = 0;
        for i in 0..ctx.frames {
            while event_idx < ctx.events.len() && ctx.events[event_idx].sample_offset <= i {
                if let GraphEvent::NoteOn {
                    velocity, voice, ..
                } = ctx.events[event_idx].event
                {
                    if voice == self.voice_id {
                        self.trigger(velocity);
                    }
                }
                event_idx += 1;
            }
            ctx.outputs[0][i] = self.tick();
        }
    }

    fn reset(&mut self) {
        self.amp = 0.0;
        self.phase = 0.0;
        self.freq = self.freq_target;
    }
}

// =====================================================================
// SnareSynth — sine body + bandpass-filtered noise burst
// =====================================================================

/// Short tonal thunk (sine) blended with band-limited noise around ~1 kHz for the snare wire crack.
pub struct SnareSynth {
    pub voice_id: u32,
    // Body: sine tone
    body_phase: f64,
    body_freq: f64,
    body_amp: f64,
    body_decay: f64,
    // Noise: bandpass filtered
    noise_amp: f64,
    noise_decay: f64,
    noise_state: u32,
    // Biquad state for bandpass
    bq_b0: f64,
    bq_b1: f64,
    bq_b2: f64,
    bq_a1: f64,
    bq_a2: f64,
    bq_x1: f64,
    bq_x2: f64,
    bq_y1: f64,
    bq_y2: f64,
    sample_rate: f64,
}

impl SnareSynth {
    /// Configures the snare for `voice_id` and initializes the body/noise paths and bandpass coefficients.
    pub fn new(voice_id: u32) -> Self {
        let mut s = Self {
            voice_id,
            body_phase: 0.0,
            body_freq: 200.0,
            body_amp: 0.0,
            body_decay: 25.0,
            noise_amp: 0.0,
            noise_decay: 15.0,
            noise_state: 0xDEADBEEF,
            bq_b0: 0.0,
            bq_b1: 0.0,
            bq_b2: 0.0,
            bq_a1: 0.0,
            bq_a2: 0.0,
            bq_x1: 0.0,
            bq_x2: 0.0,
            bq_y1: 0.0,
            bq_y2: 0.0,
            sample_rate: 44100.0,
        };
        s.compute_bandpass(1000.0, 1.5);
        s
    }

    fn compute_bandpass(&mut self, freq: f64, q: f64) {
        let w0 = 2.0 * PI * freq / self.sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        self.bq_b0 = alpha / a0;
        self.bq_b1 = 0.0;
        self.bq_b2 = -alpha / a0;
        self.bq_a1 = (-2.0 * w0.cos()) / a0;
        self.bq_a2 = (1.0 - alpha) / a0;
    }

    fn filter(&mut self, x: f64) -> f64 {
        let y = self.bq_b0 * x + self.bq_b1 * self.bq_x1 + self.bq_b2 * self.bq_x2
            - self.bq_a1 * self.bq_y1
            - self.bq_a2 * self.bq_y2;
        self.bq_x2 = self.bq_x1;
        self.bq_x1 = x;
        self.bq_y2 = self.bq_y1;
        self.bq_y1 = y;
        y
    }

    fn trigger(&mut self, velocity: f64) {
        self.body_amp = velocity * 0.7;
        self.noise_amp = velocity;
        self.body_phase = 0.0;
        self.bq_x1 = 0.0;
        self.bq_x2 = 0.0;
        self.bq_y1 = 0.0;
        self.bq_y2 = 0.0;
    }

    fn tick(&mut self) -> f32 {
        let body = if self.body_amp > 1e-6 {
            let s = (2.0 * PI * self.body_phase).sin() * self.body_amp;
            self.body_phase += self.body_freq / self.sample_rate;
            if self.body_phase >= 1.0 {
                self.body_phase -= 1.0;
            }
            self.body_amp = exp_decay(self.body_amp, self.body_decay, self.sample_rate);
            s
        } else {
            0.0
        };

        let noise = if self.noise_amp > 1e-6 {
            let raw = noise_sample(&mut self.noise_state) as f64;
            let filtered = self.filter(raw) * self.noise_amp;
            self.noise_amp = exp_decay(self.noise_amp, self.noise_decay, self.sample_rate);
            filtered
        } else {
            0.0
        };

        (body + noise) as f32
    }
}

impl Processor for SnareSynth {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "snare",
            audio_inputs: 0,
            audio_outputs: 1,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.sample_rate = ctx.sample_rate;
        self.compute_bandpass(1000.0, 1.5);
        let mut event_idx = 0;
        for i in 0..ctx.frames {
            while event_idx < ctx.events.len() && ctx.events[event_idx].sample_offset <= i {
                if let GraphEvent::NoteOn {
                    velocity, voice, ..
                } = ctx.events[event_idx].event
                {
                    if voice == self.voice_id {
                        self.trigger(velocity);
                    }
                }
                event_idx += 1;
            }
            ctx.outputs[0][i] = self.tick();
        }
    }

    fn reset(&mut self) {
        self.body_amp = 0.0;
        self.noise_amp = 0.0;
        self.body_phase = 0.0;
    }
}

// =====================================================================
// HatSynth — highpass-filtered noise with short envelope
// =====================================================================

/// Bright, metallic character from highpass-filtered noise and a fast decay; velocity tweaks decay rate.
pub struct HatSynth {
    pub voice_id: u32,
    amp: f64,
    decay: f64,
    noise_state: u32,
    // Highpass biquad state
    bq_b0: f64,
    bq_b1: f64,
    bq_b2: f64,
    bq_a1: f64,
    bq_a2: f64,
    bq_x1: f64,
    bq_x2: f64,
    bq_y1: f64,
    bq_y2: f64,
    sample_rate: f64,
}

impl HatSynth {
    /// Hat voice for `voice_id` with an ~8 kHz highpass and short exponential ring-off by default.
    pub fn new(voice_id: u32) -> Self {
        let mut s = Self {
            voice_id,
            amp: 0.0,
            decay: 40.0,
            noise_state: 0xCAFEBABE,
            bq_b0: 0.0,
            bq_b1: 0.0,
            bq_b2: 0.0,
            bq_a1: 0.0,
            bq_a2: 0.0,
            bq_x1: 0.0,
            bq_x2: 0.0,
            bq_y1: 0.0,
            bq_y2: 0.0,
            sample_rate: 44100.0,
        };
        s.compute_highpass(8000.0, 0.7);
        s
    }

    fn compute_highpass(&mut self, freq: f64, q: f64) {
        let w0 = 2.0 * PI * freq / self.sample_rate;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        self.bq_b0 = ((1.0 + cos_w0) / 2.0) / a0;
        self.bq_b1 = (-(1.0 + cos_w0)) / a0;
        self.bq_b2 = ((1.0 + cos_w0) / 2.0) / a0;
        self.bq_a1 = (-2.0 * cos_w0) / a0;
        self.bq_a2 = (1.0 - alpha) / a0;
    }

    fn filter(&mut self, x: f64) -> f64 {
        let y = self.bq_b0 * x + self.bq_b1 * self.bq_x1 + self.bq_b2 * self.bq_x2
            - self.bq_a1 * self.bq_y1
            - self.bq_a2 * self.bq_y2;
        self.bq_x2 = self.bq_x1;
        self.bq_x1 = x;
        self.bq_y2 = self.bq_y1;
        self.bq_y1 = y;
        y
    }

    fn trigger(&mut self, velocity: f64) {
        self.amp = velocity;
        self.decay = 20.0 + velocity * 40.0;
        self.bq_x1 = 0.0;
        self.bq_x2 = 0.0;
        self.bq_y1 = 0.0;
        self.bq_y2 = 0.0;
    }

    fn tick(&mut self) -> f32 {
        if self.amp < 1e-6 {
            return 0.0;
        }
        let raw = noise_sample(&mut self.noise_state) as f64;
        let filtered = self.filter(raw) * self.amp;
        self.amp = exp_decay(self.amp, self.decay, self.sample_rate);
        filtered as f32
    }
}

impl Processor for HatSynth {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "hat",
            audio_inputs: 0,
            audio_outputs: 1,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.sample_rate = ctx.sample_rate;
        self.compute_highpass(8000.0, 0.7);
        let mut event_idx = 0;
        for i in 0..ctx.frames {
            while event_idx < ctx.events.len() && ctx.events[event_idx].sample_offset <= i {
                if let GraphEvent::NoteOn {
                    velocity, voice, ..
                } = ctx.events[event_idx].event
                {
                    if voice == self.voice_id {
                        self.trigger(velocity);
                    }
                }
                event_idx += 1;
            }
            ctx.outputs[0][i] = self.tick();
        }
    }

    fn reset(&mut self) {
        self.amp = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_synth(synth: &mut dyn Processor, voice: u32, frames: usize) -> Vec<f32> {
        let mut output = vec![vec![0.0f32; frames]];
        let events = vec![crate::event::TimedEvent {
            sample_offset: 0,
            event: GraphEvent::NoteOn {
                frequency: 0.0,
                velocity: 0.75,
                voice,
            },
        }];
        let inputs: Vec<&[f32]> = vec![];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut output,
            frames,
            sample_rate: 44100.0,
            events: &events,
        };
        synth.process(&mut ctx);
        output.into_iter().next().unwrap()
    }

    #[test]
    fn kick_produces_audio() {
        let mut kick = KickSynth::new(0);
        let out = run_synth(&mut kick, 0, 4096);
        let energy: f32 = out.iter().map(|s| s * s).sum();
        assert!(energy > 0.1, "kick should produce audio");
    }

    #[test]
    fn snare_produces_audio() {
        let mut snare = SnareSynth::new(1);
        let out = run_synth(&mut snare, 1, 4096);
        let energy: f32 = out.iter().map(|s| s * s).sum();
        assert!(energy > 0.01, "snare should produce audio");
    }

    #[test]
    fn hat_produces_audio() {
        let mut hat = HatSynth::new(2);
        let out = run_synth(&mut hat, 2, 4096);
        let energy: f32 = out.iter().map(|s| s * s).sum();
        assert!(energy > 0.001, "hat should produce audio");
    }

    #[test]
    fn voice_filtering() {
        let mut kick = KickSynth::new(0);
        let out = run_synth(&mut kick, 99, 4096);
        let energy: f32 = out.iter().map(|s| s * s).sum();
        assert!(energy < 1e-10, "kick should ignore wrong voice");
    }
}

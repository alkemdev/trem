//! Schroeder-style “plate” stereo reverb: parallel combs per channel, then allpass diffusion, dry/wet blend.

use crate::graph::{
    GroupHint, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext, Processor,
    ProcessorInfo,
};

/// Schroeder reverb with 4 comb filters and 2 allpass filters per channel.
///
/// Rust-native, allocation-free in the hot path. Self-describing parameters.
pub struct PlateReverb {
    combs_l: [CombFilter; 4],
    combs_r: [CombFilter; 4],
    allpass_l: [AllpassFilter; 2],
    allpass_r: [AllpassFilter; 2],
    room_size: f64,
    damping: f64,
    mix: f64,
}

const COMB_DELAYS: [usize; 4] = [1116, 1188, 1277, 1356];
const ALLPASS_DELAYS: [usize; 2] = [556, 225];
const STEREO_SPREAD: usize = 23;

/// Max comb delay at room_size=1.0: largest base * 1.0 + spread.
const MAX_COMB_DELAY: usize = COMB_DELAYS[3] + STEREO_SPREAD + 1;
const MAX_ALLPASS_DELAY: usize = ALLPASS_DELAYS[0] + STEREO_SPREAD + 1;

fn comb_delay_for(base: usize, spread: usize, room_size: f64) -> usize {
    let d = (base as f64 * (0.5 + room_size * 0.5)) as usize + spread;
    d.min(MAX_COMB_DELAY - 1).max(1)
}

impl PlateReverb {
    /// `room_size` scales comb delay lengths and feedback; `damping` darkens the tail; `mix` is wet amount in [0, 1].
    pub fn new(room_size: f64, damping: f64, mix: f64) -> Self {
        let rs = room_size.clamp(0.0, 1.0);
        let dp = damping.clamp(0.0, 1.0);
        let fb = rs * 0.85 + 0.1;

        let make_comb = |base: usize, spread: usize| {
            CombFilter::new(comb_delay_for(base, spread, rs), MAX_COMB_DELAY, fb, dp)
        };

        let make_allpass =
            |base: usize, spread: usize| AllpassFilter::new(base + spread, MAX_ALLPASS_DELAY, 0.5);

        Self {
            combs_l: [
                make_comb(COMB_DELAYS[0], 0),
                make_comb(COMB_DELAYS[1], 0),
                make_comb(COMB_DELAYS[2], 0),
                make_comb(COMB_DELAYS[3], 0),
            ],
            combs_r: [
                make_comb(COMB_DELAYS[0], STEREO_SPREAD),
                make_comb(COMB_DELAYS[1], STEREO_SPREAD),
                make_comb(COMB_DELAYS[2], STEREO_SPREAD),
                make_comb(COMB_DELAYS[3], STEREO_SPREAD),
            ],
            allpass_l: [
                make_allpass(ALLPASS_DELAYS[0], 0),
                make_allpass(ALLPASS_DELAYS[1], 0),
            ],
            allpass_r: [
                make_allpass(ALLPASS_DELAYS[0], STEREO_SPREAD),
                make_allpass(ALLPASS_DELAYS[1], STEREO_SPREAD),
            ],
            room_size: rs,
            damping: dp,
            mix: mix.clamp(0.0, 1.0),
        }
    }

    fn update_comb_params(&mut self) {
        let fb = self.room_size * 0.85 + 0.1;
        for (i, comb) in self.combs_l.iter_mut().enumerate() {
            comb.feedback = fb;
            comb.damp = self.damping;
            comb.delay = comb_delay_for(COMB_DELAYS[i], 0, self.room_size);
        }
        for (i, comb) in self.combs_r.iter_mut().enumerate() {
            comb.feedback = fb;
            comb.damp = self.damping;
            comb.delay = comb_delay_for(COMB_DELAYS[i], STEREO_SPREAD, self.room_size);
        }
    }
}

impl Processor for PlateReverb {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "reverb",
            audio_inputs: 2,
            audio_outputs: 2,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let wet = self.mix as f32;
        let dry = 1.0 - wet;

        for i in 0..ctx.frames {
            let in_l = ctx.inputs[0][i];
            let in_r = ctx.inputs[1][i];
            let mono_in = (in_l + in_r) * 0.5;

            let mut wet_l = 0.0f32;
            let mut wet_r = 0.0f32;

            for comb in &mut self.combs_l {
                wet_l += comb.tick(mono_in);
            }
            for comb in &mut self.combs_r {
                wet_r += comb.tick(mono_in);
            }

            wet_l *= 0.25;
            wet_r *= 0.25;

            for ap in &mut self.allpass_l {
                wet_l = ap.tick(wet_l);
            }
            for ap in &mut self.allpass_r {
                wet_r = ap.tick(wet_r);
            }

            let out_l = in_l * dry + wet_l * wet;
            let out_r = in_r * dry + wet_r * wet;
            ctx.outputs[0][i] = if out_l.is_finite() { out_l } else { 0.0 };
            ctx.outputs[1][i] = if out_r.is_finite() { out_r } else { 0.0 };
        }
    }

    fn reset(&mut self) {
        for c in &mut self.combs_l {
            c.reset();
        }
        for c in &mut self.combs_r {
            c.reset();
        }
        for a in &mut self.allpass_l {
            a.reset();
        }
        for a in &mut self.allpass_r {
            a.reset();
        }
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Size",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: Some(0),
            },
            ParamDescriptor {
                id: 1,
                name: "Damping",
                min: 0.0,
                max: 1.0,
                default: 0.5,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: Some(0),
            },
            ParamDescriptor {
                id: 2,
                name: "Mix",
                min: 0.0,
                max: 1.0,
                default: 0.2,
                unit: ParamUnit::Percent,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: Some(0),
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Reverb",
            hint: GroupHint::TimeBased,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.room_size,
            1 => self.damping,
            2 => self.mix,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => {
                self.room_size = value.clamp(0.0, 1.0);
                self.update_comb_params();
            }
            1 => {
                self.damping = value.clamp(0.0, 1.0);
                self.update_comb_params();
            }
            2 => self.mix = value.clamp(0.0, 1.0),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Internal filter components
// ---------------------------------------------------------------------------

struct CombFilter {
    buf: Vec<f32>,
    pos: usize,
    delay: usize,
    feedback: f64,
    damp: f64,
    damp_state: f32,
}

impl CombFilter {
    fn new(delay: usize, max_delay: usize, feedback: f64, damp: f64) -> Self {
        let size = max_delay.max(delay).max(1).next_power_of_two();
        Self {
            buf: vec![0.0; size],
            pos: 0,
            delay: delay.min(size - 1).max(1),
            feedback,
            damp,
            damp_state: 0.0,
        }
    }

    fn tick(&mut self, input: f32) -> f32 {
        let mask = self.buf.len() - 1;
        let d = self.delay.min(mask);
        let read = (self.pos + self.buf.len() - d) & mask;
        let output = self.buf[read];

        let damp = self.damp as f32;
        self.damp_state = output * (1.0 - damp) + self.damp_state * damp;

        let fb_sample = input + self.damp_state * self.feedback as f32;
        self.buf[self.pos] = if fb_sample.is_finite() {
            fb_sample
        } else {
            0.0
        };
        self.pos = (self.pos + 1) & mask;
        output
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.damp_state = 0.0;
        self.pos = 0;
    }
}

struct AllpassFilter {
    buf: Vec<f32>,
    pos: usize,
    delay: usize,
    feedback: f64,
}

impl AllpassFilter {
    fn new(delay: usize, max_delay: usize, feedback: f64) -> Self {
        let size = max_delay.max(delay).max(1).next_power_of_two();
        Self {
            buf: vec![0.0; size],
            pos: 0,
            delay: delay.min(size - 1).max(1),
            feedback,
        }
    }

    fn tick(&mut self, input: f32) -> f32 {
        let mask = self.buf.len() - 1;
        let d = self.delay.min(mask);
        let read = (self.pos + self.buf.len() - d) & mask;
        let buffered = self.buf[read];
        let fb = self.feedback as f32;

        let output = buffered - input;
        let fb_sample = input + buffered * fb;
        self.buf[self.pos] = if fb_sample.is_finite() {
            fb_sample
        } else {
            0.0
        };
        self.pos = (self.pos + 1) & mask;
        output
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_produces_tail() {
        let mut reverb = PlateReverb::new(0.5, 0.5, 1.0);

        let mut impulse = vec![0.0f32; 4096];
        impulse[0] = 1.0;
        let out_l = vec![0.0f32; 4096];
        let out_r = vec![0.0f32; 4096];
        let inputs: Vec<&[f32]> = vec![&impulse, &impulse];
        let mut outputs = vec![out_l, out_r];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames: 4096,
            sample_rate: 44100.0,
            events: &[],
        };
        reverb.process(&mut ctx);

        let tail_energy: f32 = outputs[0][2000..].iter().map(|s| s * s).sum();
        assert!(tail_energy > 1e-6, "reverb should produce a decaying tail");
    }

    #[test]
    fn reverb_params_round_trip() {
        let mut reverb = PlateReverb::new(0.5, 0.5, 0.2);
        assert!((reverb.get_param(0) - 0.5).abs() < 1e-6);
        reverb.set_param(0, 0.8);
        assert!((reverb.get_param(0) - 0.8).abs() < 1e-6);
    }
}

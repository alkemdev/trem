//! Stereo delay with feedback and wet/dry mix; fixed maximum buffer sized for long times at 44.1 kHz.

use crate::graph::{
    GroupHint, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext, Processor,
    ProcessorInfo,
};

const MAX_DELAY_SAMPLES: usize = 44100 * 2; // 2 seconds at 44.1kHz

/// Stereo delay line with feedback and dry/wet mix.
///
/// All processing is Rust-native; no external dependencies.
/// Parameters are self-describing for automatic UI generation.
pub struct StereoDelay {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write_pos: usize,
    time_ms: f64,
    feedback: f64,
    mix: f64,
    sample_rate: f64,
}

impl StereoDelay {
    /// Delay time in ms, feedback amount clamped to [0, 0.95], and wet mix in [0, 1]; L/R share timing, separate buffers.
    pub fn new(time_ms: f64, feedback: f64, mix: f64) -> Self {
        Self {
            buf_l: vec![0.0; MAX_DELAY_SAMPLES],
            buf_r: vec![0.0; MAX_DELAY_SAMPLES],
            write_pos: 0,
            time_ms,
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
            sample_rate: 44100.0,
        }
    }

    fn delay_samples(&self) -> usize {
        let samples = (self.time_ms * 0.001 * self.sample_rate) as usize;
        samples.min(MAX_DELAY_SAMPLES - 1).max(1)
    }
}

impl Processor for StereoDelay {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "delay",
            audio_inputs: 2,
            audio_outputs: 2,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.sample_rate = ctx.sample_rate;
        let delay = self.delay_samples();
        let fb = self.feedback as f32;
        let wet = self.mix as f32;
        let dry = 1.0 - wet;
        let len = self.buf_l.len();

        for i in 0..ctx.frames {
            let in_l = ctx.inputs[0][i];
            let in_r = ctx.inputs[1][i];

            let read_pos = (self.write_pos + len - delay) % len;
            let tap_l = self.buf_l[read_pos];
            let tap_r = self.buf_r[read_pos];

            self.buf_l[self.write_pos] = in_l + tap_l * fb;
            self.buf_r[self.write_pos] = in_r + tap_r * fb;

            ctx.outputs[0][i] = in_l * dry + tap_l * wet;
            ctx.outputs[1][i] = in_r * dry + tap_r * wet;

            self.write_pos = (self.write_pos + 1) % len;
        }
    }

    fn reset(&mut self) {
        self.buf_l.fill(0.0);
        self.buf_r.fill(0.0);
        self.write_pos = 0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Time",
                min: 1.0,
                max: 2000.0,
                default: 250.0,
                unit: ParamUnit::Milliseconds,
                flags: ParamFlags::LOG_SCALE,
                step: 5.0,
                group: Some(0),
            },
            ParamDescriptor {
                id: 1,
                name: "Feedback",
                min: 0.0,
                max: 0.95,
                default: 0.4,
                unit: ParamUnit::Percent,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: Some(0),
            },
            ParamDescriptor {
                id: 2,
                name: "Mix",
                min: 0.0,
                max: 1.0,
                default: 0.3,
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
            name: "Delay",
            hint: GroupHint::TimeBased,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.time_ms,
            1 => self.feedback,
            2 => self.mix,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.time_ms = value.clamp(1.0, 2000.0),
            1 => self.feedback = value.clamp(0.0, 0.95),
            2 => self.mix = value.clamp(0.0, 1.0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_passes_dry_signal() {
        let mut delay = StereoDelay::new(100.0, 0.0, 0.0);
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
        delay.process(&mut ctx);
        let result = &outputs[0];
        for &s in result.iter() {
            assert!((s - 0.5).abs() < 1e-6, "dry signal should pass through");
        }
    }

    #[test]
    fn delay_produces_echo() {
        let mut delay = StereoDelay::new(10.0, 0.5, 1.0);
        delay.sample_rate = 44100.0;

        let mut impulse = vec![0.0f32; 1024];
        impulse[0] = 1.0;
        let silence = vec![0.0f32; 1024];

        let out_l = vec![0.0f32; 1024];
        let out_r = vec![0.0f32; 1024];
        let inputs: Vec<&[f32]> = vec![&impulse, &silence];
        let mut outputs = vec![out_l, out_r];
        let mut ctx = ProcessContext {
            inputs: &inputs,
            outputs: &mut outputs,
            frames: 1024,
            sample_rate: 44100.0,
            events: &[],
        };
        delay.process(&mut ctx);

        let delay_samples = (10.0 * 0.001 * 44100.0) as usize;
        assert!(
            outputs[0][delay_samples].abs() > 0.3,
            "should have echo at delay offset"
        );
    }

    #[test]
    fn self_describing_params() {
        let delay = StereoDelay::new(250.0, 0.4, 0.3);
        let params = delay.params();
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name, "Time");
        assert!((delay.get_param(0) - 250.0).abs() < 1e-6);
    }
}

//! ADSR envelope as a graph processor: shapes an audio stream by gate times from note events.
//!
//! Output is 0–1; multiply happens per sample so you can drive amps or filters from the same block.

use crate::event::GraphEvent;
use crate::graph::{
    GroupHint, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext, Processor,
    ProcessorInfo,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// ADSR envelope generator.
///
/// Times are in seconds. Output is in [0, 1].
/// Processes NoteOn/NoteOff events from the event stream.
pub struct Adsr {
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
    pub voice_id: Option<u32>,

    stage: Stage,
    level: f64,
    sample_rate: f64,
}

impl Adsr {
    /// Creates an envelope: `attack`, `decay`, `release` are durations in seconds; `sustain` is a held level in [0, 1].
    pub fn new(attack: f64, decay: f64, sustain: f64, release: f64) -> Self {
        Self {
            attack,
            decay,
            sustain,
            release,
            voice_id: None,
            stage: Stage::Idle,
            level: 0.0,
            sample_rate: 44100.0,
        }
    }

    /// Only reacts to `NoteOn`/`NoteOff` for the given voice when set; omit for global (omni) triggering.
    pub fn with_voice(mut self, id: u32) -> Self {
        self.voice_id = Some(id);
        self
    }

    /// Jumps into the attack stage from idle or any stage; use for manual or test triggering outside the graph.
    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
    }

    /// Begins release if not idle; matches `NoteOff` handling so tails decay instead of snapping off.
    pub fn release_note(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    /// True while the envelope is past idle (attack through release), useful to know if output may be non-zero.
    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    fn tick(&mut self) -> f32 {
        let rate = self.sample_rate;
        match self.stage {
            Stage::Idle => 0.0,
            Stage::Attack => {
                let inc = if self.attack > 0.0 {
                    1.0 / (self.attack * rate)
                } else {
                    1.0
                };
                self.level += inc;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
                self.level as f32
            }
            Stage::Decay => {
                let dec = if self.decay > 0.0 {
                    (1.0 - self.sustain) / (self.decay * rate)
                } else {
                    1.0
                };
                self.level -= dec;
                if self.level <= self.sustain {
                    self.level = self.sustain;
                    self.stage = Stage::Sustain;
                }
                self.level as f32
            }
            Stage::Sustain => self.sustain as f32,
            Stage::Release => {
                let dec = if self.release > 0.0 {
                    self.sustain / (self.release * rate)
                } else {
                    1.0
                };
                self.level -= dec;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
                self.level as f32
            }
        }
    }
}

impl Processor for Adsr {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "adsr",
            audio_inputs: 1,
            audio_outputs: 1,
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        self.sample_rate = ctx.sample_rate;

        // Process all events at the block level (sample-accurate triggers)
        let mut event_idx = 0;

        for i in 0..ctx.frames {
            while event_idx < ctx.events.len() && ctx.events[event_idx].sample_offset <= i {
                let voice_match = match ctx.events[event_idx].event {
                    GraphEvent::NoteOn { voice, .. } | GraphEvent::NoteOff { voice } => {
                        self.voice_id.is_none() || self.voice_id == Some(voice)
                    }
                    _ => true,
                };
                if voice_match {
                    match ctx.events[event_idx].event {
                        GraphEvent::NoteOn { .. } => self.trigger(),
                        GraphEvent::NoteOff { .. } => self.release_note(),
                        _ => {}
                    }
                }
                event_idx += 1;
            }
            let env_val = self.tick();
            ctx.outputs[0][i] = ctx.inputs[0][i] * env_val;
        }
    }

    fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.level = 0.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Attack",
                min: 0.001,
                max: 5.0,
                default: 0.01,
                unit: ParamUnit::Seconds,
                flags: ParamFlags::LOG_SCALE,
                step: 0.005,
                group: Some(0),
            },
            ParamDescriptor {
                id: 1,
                name: "Decay",
                min: 0.001,
                max: 5.0,
                default: 0.1,
                unit: ParamUnit::Seconds,
                flags: ParamFlags::LOG_SCALE,
                step: 0.01,
                group: Some(0),
            },
            ParamDescriptor {
                id: 2,
                name: "Sustain",
                min: 0.0,
                max: 1.0,
                default: 0.7,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: Some(0),
            },
            ParamDescriptor {
                id: 3,
                name: "Release",
                min: 0.001,
                max: 5.0,
                default: 0.3,
                unit: ParamUnit::Seconds,
                flags: ParamFlags::LOG_SCALE,
                step: 0.01,
                group: Some(0),
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Envelope",
            hint: GroupHint::Envelope,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.attack,
            1 => self.decay,
            2 => self.sustain,
            3 => self.release,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.attack = value.clamp(0.001, 5.0),
            1 => self.decay = value.clamp(0.001, 5.0),
            2 => self.sustain = value.clamp(0.0, 1.0),
            3 => self.release = value.clamp(0.001, 5.0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_shape() {
        let mut env = Adsr::new(0.01, 0.01, 0.5, 0.01);
        env.sample_rate = 44100.0;
        env.trigger();

        let mut samples = Vec::new();
        for _ in 0..2000 {
            samples.push(env.tick());
        }

        // Should rise from 0, peak at 1, decay to 0.5
        assert!(samples[0] > 0.0);
        let peak = samples.iter().cloned().fold(0.0f32, f32::max);
        assert!((peak - 1.0).abs() < 0.01);

        // After attack+decay, should settle near sustain
        let tail = samples[1500];
        assert!((tail - 0.5).abs() < 0.05);
    }
}

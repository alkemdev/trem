//! Wavetable oscillator: loads a single-cycle waveform table and reads it with
//! linear interpolation at an arbitrary frequency.

use trem::event::TimedEvent;
use trem::graph::{
    GroupHint, Node, NodeInfo, ParamDescriptor, ParamFlags, ParamGroup, ParamUnit, ProcessContext,
    Sig,
};

const TABLE_SIZE: usize = 2048;

fn generate_table(shape: u32) -> Vec<f32> {
    let mut table = vec![0.0f32; TABLE_SIZE];
    match shape {
        0 => {
            // Sine
            for i in 0..TABLE_SIZE {
                table[i] = (2.0 * std::f32::consts::PI * i as f32 / TABLE_SIZE as f32).sin();
            }
        }
        1 => {
            // Saw (additive, band-limited to 64 harmonics)
            for h in 1..=64u32 {
                let amp = 1.0 / h as f32;
                for i in 0..TABLE_SIZE {
                    let phase =
                        2.0 * std::f32::consts::PI * h as f32 * i as f32 / TABLE_SIZE as f32;
                    table[i] += amp * phase.sin();
                }
            }
            let peak = table.iter().copied().fold(0.0f32, |a, b| a.max(b.abs()));
            if peak > 0.0 {
                for s in &mut table {
                    *s /= peak;
                }
            }
        }
        2 => {
            // Square (odd harmonics)
            for h in (1..=63u32).step_by(2) {
                let amp = 1.0 / h as f32;
                for i in 0..TABLE_SIZE {
                    let phase =
                        2.0 * std::f32::consts::PI * h as f32 * i as f32 / TABLE_SIZE as f32;
                    table[i] += amp * phase.sin();
                }
            }
            let peak = table.iter().copied().fold(0.0f32, |a, b| a.max(b.abs()));
            if peak > 0.0 {
                for s in &mut table {
                    *s /= peak;
                }
            }
        }
        3 => {
            // Triangle (odd harmonics, alternating sign)
            for (k, h) in (1..=63u32).step_by(2).enumerate() {
                let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
                let amp = sign / (h as f32 * h as f32);
                for i in 0..TABLE_SIZE {
                    let phase =
                        2.0 * std::f32::consts::PI * h as f32 * i as f32 / TABLE_SIZE as f32;
                    table[i] += amp * phase.sin();
                }
            }
            let peak = table.iter().copied().fold(0.0f32, |a, b| a.max(b.abs()));
            if peak > 0.0 {
                for s in &mut table {
                    *s /= peak;
                }
            }
        }
        _ => {}
    }
    table
}

/// Wavetable oscillator with morphable shape and detune.
///
/// The table is 2048 samples, read with linear interpolation.
/// Shape parameter crossfades between precomputed waveforms.
pub struct Wavetable {
    tables: [Vec<f32>; 4],
    phase: f64,
    frequency: f64,
    detune: f64,
    shape: f64,
    level: f64,
}

impl Wavetable {
    /// Default wavetable at 440 Hz with shape centred on sine. Four tables are
    /// precomputed (sine, triangle, saw, square) and crossfaded by the shape parameter.
    pub fn new() -> Self {
        Self {
            tables: [
                generate_table(0),
                generate_table(1),
                generate_table(2),
                generate_table(3),
            ],
            phase: 0.0,
            frequency: 440.0,
            detune: 0.0,
            shape: 0.0,
            level: 1.0,
        }
    }

    fn read_table(&self, table_idx: usize, phase: f64) -> f32 {
        let t = &self.tables[table_idx];
        let pos = phase * TABLE_SIZE as f64;
        let idx = pos as usize;
        let frac = pos - idx as f64;
        let a = t[idx % TABLE_SIZE];
        let b = t[(idx + 1) % TABLE_SIZE];
        a + (b - a) * frac as f32
    }
}

impl Default for Wavetable {
    fn default() -> Self {
        Self::new()
    }
}

impl Node for Wavetable {
    fn info(&self) -> NodeInfo {
        NodeInfo {
            name: "wavetable",
            sig: Sig::SOURCE1,
            description: "Wavetable oscillator with shape morphing",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        for ev in ctx.events {
            if let TimedEvent {
                event: trem::event::GraphEvent::NoteOn { frequency, .. },
                ..
            } = ev
            {
                self.frequency = *frequency;
            }
        }

        let freq = self.frequency * 2.0f64.powf(self.detune / 12.0);
        let inc = freq / ctx.sample_rate;
        let shape = self.shape.clamp(0.0, 3.0);
        let idx_a = shape as usize;
        let idx_b = (idx_a + 1).min(3);
        let mix = (shape - idx_a as f64) as f32;
        let lvl = self.level as f32;

        for i in 0..ctx.frames {
            let a = self.read_table(idx_a, self.phase);
            let b = self.read_table(idx_b, self.phase);
            ctx.outputs[0][i] = (a + (b - a) * mix) * lvl;
            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn params(&self) -> Vec<ParamDescriptor> {
        vec![
            ParamDescriptor {
                id: 0,
                name: "Shape",
                min: 0.0,
                max: 3.0,
                default: 0.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.1,
                group: Some(0),
                help: "",
            },
            ParamDescriptor {
                id: 1,
                name: "Detune",
                min: -24.0,
                max: 24.0,
                default: 0.0,
                unit: ParamUnit::Semitones,
                flags: ParamFlags::BIPOLAR,
                step: 0.1,
                group: Some(0),
                help: "",
            },
            ParamDescriptor {
                id: 2,
                name: "Level",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                unit: ParamUnit::Linear,
                flags: ParamFlags::NONE,
                step: 0.05,
                group: None,
                help: "",
            },
        ]
    }

    fn param_groups(&self) -> Vec<ParamGroup> {
        vec![ParamGroup {
            id: 0,
            name: "Oscillator",
            hint: GroupHint::Oscillator,
        }]
    }

    fn get_param(&self, id: u32) -> f64 {
        match id {
            0 => self.shape,
            1 => self.detune,
            2 => self.level,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            0 => self.shape = value.clamp(0.0, 3.0),
            1 => self.detune = value.clamp(-24.0, 24.0),
            2 => self.level = value.clamp(0.0, 1.0),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wavetable_produces_output() {
        let mut wt = Wavetable::new();
        let mut out_buf = vec![vec![0.0f32; 128]];
        wt.process(&mut ProcessContext {
            inputs: &[],
            outputs: &mut out_buf,
            frames: 128,
            sample_rate: 44100.0,
            events: &[TimedEvent {
                sample_offset: 0,
                event: trem::event::GraphEvent::NoteOn {
                    frequency: 440.0,
                    voice: 0,
                    velocity: 1.0,
                },
            }],
        });
        let energy: f32 = out_buf[0].iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "wavetable should produce signal");
    }

    #[test]
    fn shape_morphing_changes_timbre() {
        let mut wt_a = Wavetable::new();
        wt_a.set_param(0, 0.0);
        let mut wt_b = Wavetable::new();
        wt_b.set_param(0, 2.0);

        let event = TimedEvent {
            sample_offset: 0,
            event: trem::event::GraphEvent::NoteOn {
                frequency: 440.0,
                voice: 0,
                velocity: 1.0,
            },
        };

        let mut out_a = vec![vec![0.0f32; 256]];
        let mut out_b = vec![vec![0.0f32; 256]];

        wt_a.process(&mut ProcessContext {
            inputs: &[],
            outputs: &mut out_a,
            frames: 256,
            sample_rate: 44100.0,
            events: &[event.clone()],
        });
        wt_b.process(&mut ProcessContext {
            inputs: &[],
            outputs: &mut out_b,
            frames: 256,
            sample_rate: 44100.0,
            events: &[event],
        });

        assert_ne!(
            out_a[0], out_b[0],
            "different shapes should produce different output"
        );
    }

    #[test]
    fn params_roundtrip() {
        let mut wt = Wavetable::new();
        wt.set_param(0, 1.5);
        wt.set_param(1, -3.0);
        wt.set_param(2, 0.8);
        assert!((wt.get_param(0) - 1.5).abs() < 1e-9);
        assert!((wt.get_param(1) - (-3.0)).abs() < 1e-9);
        assert!((wt.get_param(2) - 0.8).abs() < 1e-9);
    }
}

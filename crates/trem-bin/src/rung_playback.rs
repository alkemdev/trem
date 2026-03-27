//! Default polyphonic synth + pattern playback for `trem rung edit`.
//!
//! Maps each [`trem::rung::ClipNote::class`] to **MIDI-like pitch** (12-TET, A4=440) for
//! preview only; the Rung format does not embed tuning.

use crate::demo::graph::instrument_channel;
use crate::demo::levels::BLOCK_SIZE;
use num_rational::Rational64;
use trem::event::{cmp_timed_event_delivery, GraphEvent, TimedEvent};
use trem::graph::{Graph, Node};
use trem::rung::Clip;
use trem_dsp::standard as dsp;
use trem_rta::{create_bridge, AudioEngine, Bridge, Command, Notification};

const VOICES: u32 = 16;
const SAMPLE_RATE: f64 = 44100.0;

/// Realtime preview: 16 [`analog_voice`](trem_dsp::analog_voice) lanes + simple mix/limiter.
pub struct RungPlayback {
    bridge: Bridge,
    _engine: AudioEngine,
    sample_rate: f64,
    pub bpm: f64,
    pub playing: bool,
    pub last_beat: f64,
}

pub fn build_rung_preview_graph() -> (Graph, u32) {
    let mut g = Graph::new(BLOCK_SIZE);
    let mut ch_ids = Vec::new();
    for i in 0..VOICES {
        let mut synth = dsp::analog_voice(i, BLOCK_SIZE);
        let pan = ((i as f32) - ((VOICES - 1) as f32) / 2.0) * 0.06;
        synth.set_param(8, 0.32);
        let ch = instrument_channel("rung", Box::new(synth), 0.18, pan);
        let id = g.add_node(Box::new(ch));
        ch_ids.push(id);
    }
    let mix = g.add_node(Box::new(dsp::StereoMixer::with_level(VOICES as u16, 0.55)));
    for (i, &ch) in ch_ids.iter().enumerate() {
        let o = (i * 2) as u16;
        g.connect(ch, 0, mix, o);
        g.connect(ch, 1, mix, o + 1);
    }
    let lim = g.add_node(Box::new(dsp::Limiter::new(-1.2, 45.0)));
    g.connect(mix, 0, lim, 0);
    g.connect(mix, 1, lim, 1);
    g.set_output(lim, 2);
    (g, lim)
}

impl RungPlayback {
    /// Opens default output at 44.1 kHz. Returns `None` if no device / wrong format.
    pub fn try_new() -> Option<Self> {
        let (mut bridge, audio_bridge) = create_bridge(4096);
        let (graph, out_id) = build_rung_preview_graph();
        let engine = AudioEngine::new(audio_bridge, graph, out_id, None, SAMPLE_RATE).ok()?;
        bridge.send(Command::SetBpm(120.0));
        Some(Self {
            bridge,
            _engine: engine,
            sample_rate: SAMPLE_RATE,
            bpm: 120.0,
            playing: false,
            last_beat: 0.0,
        })
    }

    pub fn reload_clip(&mut self, clip: &Clip) {
        let events = clip_to_timed_events(clip, self.sample_rate, self.bpm);
        self.bridge.send(Command::LoadEvents {
            loop_len: clip_loop_len_samples(clip, self.sample_rate, self.bpm),
            events,
        });
    }

    pub fn set_bpm(&mut self, bpm: f64, clip: &Clip) {
        self.bpm = bpm.clamp(20.0, 320.0);
        self.bridge.send(Command::SetBpm(self.bpm));
        self.reload_clip(clip);
    }

    pub fn nudge_bpm(&mut self, delta: f64, clip: &Clip) {
        self.set_bpm(self.bpm + delta, clip);
    }

    pub fn toggle_playback(&mut self) {
        if self.playing {
            self.bridge.send(Command::Pause);
        } else {
            self.bridge.send(Command::Play);
        }
        self.playing = !self.playing;
    }

    pub fn stop(&mut self) {
        self.bridge.send(Command::Stop);
        self.playing = false;
    }

    pub fn drain_ui(&mut self) {
        while let Some(n) = self.bridge.try_recv() {
            if let Notification::Position { beat } = n {
                self.last_beat = beat;
            }
        }
    }
}

fn beat_to_samples(beats: Rational64, sample_rate: f64, bpm: f64) -> usize {
    let b = *beats.numer() as f64 / *beats.denom() as f64;
    let sec = b * 60.0 / bpm.max(1e-6);
    (sec * sample_rate).round().max(0.0) as usize
}

/// Preview mapping: treat `class` as a MIDI key number (12-TET, A4=440).
fn class_to_hz(class: i32) -> f64 {
    let m = class.clamp(0, 127) as f64;
    440.0 * 2.0_f64.powf((m - 69.0) / 12.0)
}

/// Maps clip notes to graph voices: each [`GraphEvent`] voice is monophonic, so overlapping notes
/// need distinct slots. Uses first-fit across `VOICES` lanes; if all are busy, steals the lane that
/// frees soonest and inserts a [`GraphEvent::NoteOff`] at the new attack time.
fn clip_to_timed_events(clip: &Clip, sample_rate: f64, bpm: f64) -> Vec<TimedEvent> {
    let mut order: Vec<usize> = (0..clip.notes.len()).collect();
    order.sort_by(|&i, &j| {
        let a = &clip.notes[i];
        let b = &clip.notes[j];
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.t_off.rational().cmp(&b.t_off.rational()))
    });

    let mut voice_free_sample: Vec<usize> = vec![0; VOICES as usize];
    let mut events: Vec<TimedEvent> = Vec::with_capacity(clip.notes.len().saturating_mul(3));

    for idx in order {
        let n = &clip.notes[idx];
        let on = beat_to_samples(n.t_on.rational(), sample_rate, bpm);
        let off = beat_to_samples(n.t_off.rational(), sample_rate, bpm).max(on.saturating_add(1));

        let chosen = (0..VOICES as usize).find(|&v| voice_free_sample[v] <= on);
        let v = chosen.unwrap_or_else(|| {
            (0..VOICES as usize)
                .min_by_key(|&vi| voice_free_sample[vi])
                .unwrap_or(0)
        });

        if voice_free_sample[v] > on {
            events.push(TimedEvent {
                sample_offset: on,
                event: GraphEvent::NoteOff { voice: v as u32 },
            });
        }
        voice_free_sample[v] = off;

        let vel = n.velocity.clamp(0.0, 1.0);
        let hz = class_to_hz(n.class);
        events.push(TimedEvent {
            sample_offset: on,
            event: GraphEvent::NoteOn {
                frequency: hz,
                velocity: vel,
                voice: v as u32,
            },
        });
        events.push(TimedEvent {
            sample_offset: off,
            event: GraphEvent::NoteOff { voice: v as u32 },
        });
    }

    events.sort_by(cmp_timed_event_delivery);
    events
}

fn clip_loop_len_samples(clip: &Clip, sample_rate: f64, bpm: f64) -> usize {
    clip.length_beats
        .map(|beats| beat_to_samples(beats.rational(), sample_rate, bpm))
        .unwrap_or_else(|| {
            clip.notes
                .iter()
                .map(|note| beat_to_samples(note.t_off.rational(), sample_rate, bpm))
                .max()
                .unwrap_or(0)
        })
}

#[cfg(test)]
mod clip_voice_tests {
    use super::*;
    use trem::rung::{BeatTime, ClipNote, NoteMeta};

    #[test]
    fn overlapping_same_midi_channel_gets_distinct_graph_voices() {
        let clip = Clip {
            notes: vec![
                ClipNote {
                    id: None,
                    class: 60,
                    t_on: BeatTime::new(0, 1),
                    t_off: BeatTime::new(2, 1),
                    voice: 0,
                    velocity: 0.8,
                    meta: NoteMeta::default(),
                },
                ClipNote {
                    id: None,
                    class: 64,
                    t_on: BeatTime::new(1, 1),
                    t_off: BeatTime::new(3, 1),
                    voice: 0,
                    velocity: 0.8,
                    meta: NoteMeta::default(),
                },
            ],
            length_beats: None,
        };
        let ev = clip_to_timed_events(&clip, 48_000.0, 120.0);
        let voices_on: Vec<u32> = ev
            .iter()
            .filter_map(|e| match &e.event {
                GraphEvent::NoteOn { voice, .. } => Some(*voice),
                _ => None,
            })
            .collect();
        assert_eq!(voices_on.len(), 2);
        assert_ne!(
            voices_on[0], voices_on[1],
            "overlapping notes must not share a monophonic graph voice"
        );
    }
}

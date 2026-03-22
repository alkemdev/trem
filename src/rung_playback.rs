//! Default polyphonic synth + pattern playback for `trem rung edit`.
//!
//! Maps each [`trem_rung::ClipNote::class`] to **MIDI-like pitch** (12-TET, A4=440) for
//! preview only; the Rung format does not embed tuning.

use crate::demo::graph::instrument_channel;
use crate::demo::levels::BLOCK_SIZE;
use num_rational::Rational64;
use trem::dsp;
use trem::event::{GraphEvent, TimedEvent};
use trem::graph::{Graph, Processor};
use trem_cpal::{create_bridge, AudioEngine, Bridge, Command, Notification};
use trem_rung::Clip;

const VOICES: u32 = 16;
const SAMPLE_RATE: f64 = 44100.0;

/// Realtime preview: 16 [`trem::dsp::analog_voice`] lanes + simple mix/limiter.
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
        self.bridge.send(Command::LoadEvents(events));
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

/// Preview mapping: treat `class` as a MIDI note number (12-TET, A4=440).
fn class_to_hz(class: i32) -> f64 {
    let m = class.clamp(-48, 127) as f64;
    440.0 * 2.0_f64.powf((m - 69.0) / 12.0)
}

fn event_sort_key(e: &TimedEvent) -> (usize, u8, u32) {
    let p = match &e.event {
        GraphEvent::NoteOn { voice, .. } => (0, *voice),
        GraphEvent::NoteOff { voice } => (1, *voice),
        GraphEvent::Param { .. } => (2, 0),
    };
    (e.sample_offset, p.0, p.1)
}

fn clip_to_timed_events(clip: &Clip, sample_rate: f64, bpm: f64) -> Vec<TimedEvent> {
    let mut events = Vec::with_capacity(clip.notes.len().saturating_mul(2));
    for n in &clip.notes {
        let v = n.voice % VOICES;
        let on = beat_to_samples(n.t_on.rational(), sample_rate, bpm);
        let off = beat_to_samples(n.t_off.rational(), sample_rate, bpm).max(on.saturating_add(1));
        let vel = n.velocity.clamp(0.0, 1.0);
        let hz = class_to_hz(n.class);
        events.push(TimedEvent {
            sample_offset: on,
            event: GraphEvent::NoteOn {
                frequency: hz,
                velocity: vel,
                voice: v,
            },
        });
        events.push(TimedEvent {
            sample_offset: off,
            event: GraphEvent::NoteOff { voice: v },
        });
    }
    events.sort_by_key(event_sort_key);
    events
}

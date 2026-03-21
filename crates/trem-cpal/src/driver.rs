//! `cpal` output stream driving a [`trem::graph::Graph`] with command/notification bridging.

use crate::bridge::{AudioBridge, Command, Notification};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use trem::event::{GraphEvent, TimedEvent};
use trem::graph::Graph;

struct CallbackState {
    cmd_rx: rtrb::Consumer<Command>,
    notif_tx: rtrb::Producer<Notification>,
    graph: Graph,
    output_node: u32,
    sample_rate: f64,
    playing: bool,
    bpm: f64,
    pattern_events: Vec<TimedEvent>,
    pattern_len: usize,
    playhead: usize,
    block_events: Vec<TimedEvent>,
    meter_acc: usize,
    meter_peak_l: f32,
    meter_peak_r: f32,
    pos_acc: usize,
}

impl CallbackState {
    fn pattern_len_from_events(events: &[TimedEvent]) -> usize {
        events
            .iter()
            .map(|e| e.sample_offset.saturating_add(1))
            .max()
            .unwrap_or(0)
    }

    fn drain_commands(&mut self) {
        while let Ok(cmd) = self.cmd_rx.pop() {
            match cmd {
                Command::NoteOn {
                    frequency,
                    velocity,
                    voice,
                } => {
                    self.block_events.push(TimedEvent {
                        sample_offset: 0,
                        event: GraphEvent::NoteOn {
                            frequency,
                            velocity,
                            voice,
                        },
                    });
                }
                Command::NoteOff { voice } => {
                    self.block_events.push(TimedEvent {
                        sample_offset: 0,
                        event: GraphEvent::NoteOff { voice },
                    });
                }
                Command::SetBpm(bpm) => {
                    self.bpm = bpm;
                }
                Command::Play => {
                    self.playing = true;
                }
                Command::Stop => {
                    self.playing = false;
                    self.playhead = 0;
                    self.graph.reset();
                }
                Command::LoadEvents(mut events) => {
                    std::mem::swap(&mut self.pattern_events, &mut events);
                    drop(events);
                    self.pattern_len = Self::pattern_len_from_events(&self.pattern_events);
                    self.playhead = 0;
                    self.block_events
                        .reserve(self.pattern_events.len().saturating_mul(8).max(256));
                }
                Command::SetParam {
                    node,
                    param_id,
                    value,
                } => {
                    self.graph.set_node_param(node, param_id, value);
                }
            }
        }
    }

    /// All sample offsets `k` in `[0, frames)` where `(playhead + k) % len == target`.
    fn schedule_pattern_event(
        block_events: &mut Vec<TimedEvent>,
        playhead: usize,
        pattern_len: usize,
        frames: usize,
        event: &TimedEvent,
    ) {
        if pattern_len == 0 {
            return;
        }
        let target = event.sample_offset % pattern_len;
        let first = if target >= playhead {
            target - playhead
        } else {
            pattern_len - playhead + target
        };
        let mut k = first;
        while k < frames {
            block_events.push(TimedEvent {
                sample_offset: k,
                event: event.event.clone(),
            });
            k = k.saturating_add(pattern_len);
        }
    }

    fn collect_pattern_events_for_block(&mut self, frames: usize) {
        if !self.playing || self.pattern_len == 0 {
            return;
        }
        let playhead = self.playhead % self.pattern_len;
        for e in &self.pattern_events {
            Self::schedule_pattern_event(
                &mut self.block_events,
                playhead,
                self.pattern_len,
                frames,
                e,
            );
        }
    }

    fn sort_block_events(&mut self) {
        self.block_events.sort_by_key(|e| e.sample_offset);
    }

    fn advance_playhead(&mut self, frames: usize) {
        if !self.playing || self.pattern_len == 0 {
            return;
        }
        self.playhead = (self.playhead + frames) % self.pattern_len;
    }

    fn push_position_if_due(&mut self, frames: usize) {
        self.pos_acc += frames;
        const INTERVAL: usize = 256;
        if self.pos_acc < INTERVAL {
            return;
        }
        self.pos_acc = 0;
        if !self.playing {
            return;
        }
        let beat = self.playhead as f64 * self.bpm / (60.0 * self.sample_rate);
        let _ = self.notif_tx.push(Notification::Position { beat });
    }

    fn flush_meter_if_due(&mut self) {
        const METER_INTERVAL: usize = 1024;
        if self.meter_acc < METER_INTERVAL {
            return;
        }
        self.meter_acc = 0;
        let _ = self.notif_tx.push(Notification::Meter {
            peak_l: self.meter_peak_l,
            peak_r: self.meter_peak_r,
        });
        self.meter_peak_l = 0.0;
        self.meter_peak_r = 0.0;
    }

    fn process_output(&mut self, data: &mut [f32], channels: usize) {
        let frames = data.len() / channels;

        self.block_events.clear();
        self.drain_commands();
        self.collect_pattern_events_for_block(frames);
        self.sort_block_events();

        self.graph
            .process(frames, self.sample_rate, &self.block_events);
        let l = self.graph.output_buffer(self.output_node, 0);
        let r = self.graph.output_buffer(self.output_node, 1);

        for i in 0..frames {
            let li = l.get(i).copied().unwrap_or(0.0);
            let ri = r.get(i).copied().unwrap_or(li);
            let al = li.abs();
            let ar = ri.abs();
            if al > self.meter_peak_l {
                self.meter_peak_l = al;
            }
            if ar > self.meter_peak_r {
                self.meter_peak_r = ar;
            }
            if channels >= 2 {
                data[i * channels] = li;
                data[i * channels + 1] = ri;
            } else {
                data[i] = 0.5 * (li + ri);
            }
        }
        self.meter_acc += frames;
        self.flush_meter_if_due();
        if self.playing {
            self.advance_playhead(frames);
        }

        self.push_position_if_due(frames);
    }
}

pub struct AudioEngine {
    /// Kept so the device keeps playing until the engine is dropped.
    pub stream: Stream,
}

impl AudioEngine {
    pub fn new(
        audio_bridge: AudioBridge,
        graph: Graph,
        output_node: u32,
        sample_rate: f64,
    ) -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no default output device"))?;

        let supported = device.default_output_config()?;
        if supported.sample_format() != SampleFormat::F32 {
            return Err(anyhow::anyhow!(
                "default output is not f32; pick an F32-capable device or config"
            ));
        }

        let mut stream_config: StreamConfig = supported.config();
        stream_config.channels = stream_config.channels.max(2);
        stream_config.sample_rate = sample_rate.round() as u32;

        let channels = stream_config.channels as usize;

        let AudioBridge {
            mut cmd_rx,
            notif_tx,
        } = audio_bridge;

        while cmd_rx.pop().is_ok() {}

        let mut state = CallbackState {
            cmd_rx,
            notif_tx,
            graph,
            output_node,
            sample_rate,
            playing: false,
            bpm: 120.0,
            pattern_events: Vec::new(),
            pattern_len: 0,
            playhead: 0,
            block_events: Vec::with_capacity(1024),
            meter_acc: 0,
            meter_peak_l: 0.0,
            meter_peak_r: 0.0,
            pos_acc: 0,
        };
        state.block_events.reserve(4096);

        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                state.process_output(data, channels);
            },
            |err| {
                eprintln!("trem-cpal stream error: {err}");
            },
            None,
        )?;

        stream.play()?;

        Ok(Self { stream })
    }
}

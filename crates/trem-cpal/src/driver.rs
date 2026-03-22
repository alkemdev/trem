//! `cpal` output stream driving a [`trem::graph::Graph`] with [`crate::bridge`] command/notification bridging.
//!
//! [`AudioEngine`] builds the device stream, drains any stale commands, and runs the graph in the callback.

use crate::bridge::{AudioBridge, Command, Notification, ScopeFocus};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use trem::event::{GraphEvent, TimedEvent};
use trem::graph::{Graph, Sig};

struct CallbackState {
    cmd_rx: rtrb::Consumer<Command>,
    notif_tx: rtrb::Producer<Notification>,
    graph: Graph,
    output_node: u32,
    /// Inst-bus node id for [`ScopeFocus::PatchBuses`] (pre-master submix).
    scope_input_node: Option<u32>,
    scope_focus: ScopeFocus,
    preview_scratch_l: Vec<f32>,
    preview_scratch_r: Vec<f32>,
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
    scope_master: Box<[f32]>,
    scope_master_len: usize,
    scope_graph_in: Box<[f32]>,
    scope_graph_in_len: usize,
    scope_acc: usize,
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
                    path,
                    param_id,
                    value,
                } => {
                    self.graph.set_param_at_path(&path, param_id, value);
                }
                Command::SetScopeFocus(focus) => {
                    self.scope_focus = focus;
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

    fn flush_scope_if_due(&mut self) {
        const SCOPE_INTERVAL: usize = 2048;
        if self.scope_acc < SCOPE_INTERVAL {
            return;
        }
        self.scope_acc = 0;
        if self.scope_master_len > 0 {
            let master = self.scope_master[..self.scope_master_len].to_vec();
            let graph_in = if self.scope_graph_in_len > 0 {
                self.scope_graph_in[..self.scope_graph_in_len].to_vec()
            } else {
                master.clone()
            };
            self.scope_master_len = 0;
            self.scope_graph_in_len = 0;
            let _ = self
                .notif_tx
                .push(Notification::ScopeData(crate::bridge::ScopeSnapshot {
                    master,
                    graph_in,
                }));
        }
    }

    fn process_output(&mut self, data: &mut [f32], channels: usize) {
        let frames = data.len() / channels;

        self.block_events.clear();
        self.drain_commands();
        self.collect_pattern_events_for_block(frames);
        self.sort_block_events();

        self.graph.run(frames, self.sample_rate, &self.block_events);
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

        self.append_scope_samples(frames);
        self.scope_acc += frames;
        self.flush_scope_if_due();
        self.meter_acc += frames;
        self.flush_meter_if_due();
        if self.playing {
            self.advance_playhead(frames);
        }

        self.push_position_if_due(frames);
    }

    /// Fills `scope_graph_in` (left pane = “in”) and `scope_master` (right = “out”) from the
    /// active [`ScopeFocus`].
    fn append_scope_samples(&mut self, frames: usize) {
        let cap = self.scope_master.len();
        match &self.scope_focus {
            ScopeFocus::PatchBuses => {
                let ml = self.graph.output_buffer(self.output_node, 0);
                let mr = self.graph.output_buffer(self.output_node, 1);
                let (il, ir) = if let Some(nid) = self.scope_input_node {
                    (
                        self.graph.output_buffer(nid, 0),
                        self.graph.output_buffer(nid, 1),
                    )
                } else {
                    (ml, mr)
                };
                for i in 0..frames {
                    if self.scope_master_len + 2 > cap {
                        break;
                    }
                    let gli = il.get(i).copied().unwrap_or(0.0);
                    let gri = ir.get(i).copied().unwrap_or(gli);
                    self.scope_graph_in[self.scope_graph_in_len] = gli;
                    self.scope_graph_in[self.scope_graph_in_len + 1] = gri;
                    self.scope_graph_in_len += 2;
                    let m0 = ml.get(i).copied().unwrap_or(0.0);
                    let m1 = mr.get(i).copied().unwrap_or(m0);
                    self.scope_master[self.scope_master_len] = m0;
                    self.scope_master[self.scope_master_len + 1] = m1;
                    self.scope_master_len += 2;
                }
            }
            ScopeFocus::GraphNode { graph_path, node } => {
                if self.preview_scratch_l.len() < frames {
                    self.preview_scratch_l.resize(frames, 0.0);
                    self.preview_scratch_r.resize(frames, 0.0);
                }
                let sig = self
                    .graph
                    .node_sig_at_path(graph_path, *node)
                    .unwrap_or(Sig {
                        inputs: 0,
                        outputs: 0,
                    });
                let ins = sig.inputs as usize;
                let outs = sig.outputs as usize;
                if ins >= 1 {
                    self.graph.mix_input_port_at_path(
                        graph_path,
                        *node,
                        0,
                        frames,
                        &mut self.preview_scratch_l[..frames],
                    );
                } else {
                    self.preview_scratch_l[..frames].fill(0.0);
                }
                if ins >= 2 {
                    self.graph.mix_input_port_at_path(
                        graph_path,
                        *node,
                        1,
                        frames,
                        &mut self.preview_scratch_r[..frames],
                    );
                } else {
                    self.preview_scratch_r[..frames]
                        .copy_from_slice(&self.preview_scratch_l[..frames]);
                }
                for i in 0..frames {
                    if self.scope_master_len + 2 > cap {
                        break;
                    }
                    self.scope_graph_in[self.scope_graph_in_len] = self.preview_scratch_l[i];
                    self.scope_graph_in[self.scope_graph_in_len + 1] = self.preview_scratch_r[i];
                    self.scope_graph_in_len += 2;

                    let ol0 = self
                        .graph
                        .output_buffer_at_path(graph_path, *node, 0)
                        .and_then(|b| b.get(i))
                        .copied()
                        .unwrap_or(0.0);
                    let ol1 = if outs >= 2 {
                        self.graph
                            .output_buffer_at_path(graph_path, *node, 1)
                            .and_then(|b| b.get(i))
                            .copied()
                            .unwrap_or(ol0)
                    } else if outs == 1 {
                        ol0
                    } else {
                        0.0
                    };
                    self.scope_master[self.scope_master_len] = ol0;
                    self.scope_master[self.scope_master_len + 1] = ol1;
                    self.scope_master_len += 2;
                }
            }
        }
    }
}

/// Live output stream; dropping it stops audio and releases the device.
pub struct AudioEngine {
    /// Kept so the device keeps playing until the engine is dropped.
    pub stream: Stream,
}

impl AudioEngine {
    /// Opens the default F32 stereo output at `sample_rate`, wiring `audio_bridge` and `graph` into the callback.
    ///
    /// `scope_input_node`: optional graph node id whose stereo output is copied into
    /// [`crate::bridge::ScopeSnapshot::graph_in`] (e.g. instrument bus before master FX).
    pub fn new(
        audio_bridge: AudioBridge,
        graph: Graph,
        output_node: u32,
        scope_input_node: Option<u32>,
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
            scope_input_node,
            scope_focus: ScopeFocus::PatchBuses,
            preview_scratch_l: Vec::new(),
            preview_scratch_r: Vec::new(),
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
            scope_master: vec![0.0f32; 8192].into_boxed_slice(),
            scope_master_len: 0,
            scope_graph_in: vec![0.0f32; 8192].into_boxed_slice(),
            scope_graph_in_len: 0,
            scope_acc: 0,
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

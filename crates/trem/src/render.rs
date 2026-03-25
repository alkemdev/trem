//! Turn [`Tree`]s and [`Grid`]s into [`TimedEvent`]s and offline multi-block [`Graph`] renders.
//!
//! Beat positions are converted via [`crate::time::beat_to_sample`]; each flat leaf becomes a note on/off pair.
//!
//! For fixed-length offline passes, prefer [`render_captures`] (or [`render`] for one node / many
//! ports) over hand-rolled `Graph::run` loops. Use [`loop_timed_events`] to repeat a short pattern
//! (e.g. one bar of drum hits) across a long render.

use crate::event::{cmp_timed_event_delivery, GraphEvent, NoteEvent, TimedEvent};
use crate::graph::{Graph, NodeId, PortIdx, PrepareError};
use crate::grid::Grid;
use crate::math::Rational;
use crate::pitch::Scale;
use crate::time::beat_to_sample;
use crate::tree::Tree;

/// Default block size for [`render`] (see [`render_captures`] to choose another).
pub const DEFAULT_RENDER_BLOCK_SIZE: usize = 512;

/// Resolve a NoteEvent against a Scale to produce a frequency.
fn resolve_frequency(event: &NoteEvent, scale: &Scale, reference_hz: f64) -> f64 {
    let pitch = scale.resolve(event.degree);
    let octave_pitch = crate::pitch::Pitch(pitch.0 + event.octave as f64);
    octave_pitch.to_hz(reference_hz)
}

/// Convert a tree of NoteEvents to sample-timed graph events.
///
/// Events are positioned by flattening the tree over a given beat duration,
/// then converting rational beat positions to sample offsets.
pub fn tree_to_timed_events(
    tree: &Tree<NoteEvent>,
    beats: Rational,
    bpm: f64,
    sample_rate: f64,
    scale: &Scale,
    reference_hz: f64,
) -> Vec<TimedEvent> {
    let flat = tree.flatten();
    let mut events = Vec::new();
    let mut voice = 0u32;

    for fe in &flat {
        let beat_start = fe.start * beats;
        let beat_end = (fe.start + fe.duration) * beats;

        let sample_on = beat_to_sample(beat_start, bpm, sample_rate) as usize;
        let sample_off = beat_to_sample(beat_end, bpm, sample_rate) as usize;

        let freq = resolve_frequency(fe.event, scale, reference_hz);
        let vel = fe.event.velocity.to_f64();

        events.push(TimedEvent {
            sample_offset: sample_on,
            event: GraphEvent::NoteOn {
                frequency: freq,
                velocity: vel,
                voice,
            },
        });
        events.push(TimedEvent {
            sample_offset: sample_off,
            event: GraphEvent::NoteOff { voice },
        });
        voice += 1;
    }

    events.sort_by(cmp_timed_event_delivery);
    events
}

/// Tiles `pattern` every `loop_len_samples` until `duration_samples`, sorted by absolute time.
///
/// Use one bar (or one beat) of [`TimedEvent`]s with offsets in `[0, loop_len_samples)`; this is the
/// offline equivalent of looping a short MIDI clip.
///
/// Events at or beyond `duration_samples` are omitted. If `loop_len_samples` is zero, returns
/// empty.
pub fn loop_timed_events(
    pattern: &[TimedEvent],
    loop_len_samples: usize,
    duration_samples: usize,
) -> Vec<TimedEvent> {
    if loop_len_samples == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut base = 0usize;
    while base < duration_samples {
        for e in pattern {
            let t = base.saturating_add(e.sample_offset);
            if t < duration_samples {
                out.push(TimedEvent {
                    sample_offset: t,
                    event: e.event.clone(),
                });
            }
        }
        base += loop_len_samples;
    }
    out.sort_by(cmp_timed_event_delivery);
    out
}

/// Offline-render the graph for `duration_samples`, recording one buffer per `(node, port)` tap.
///
/// Runs in blocks of `block_size` (clamped to at least 1). [`TimedEvent::sample_offset`] values are
/// absolute; each block receives only events falling inside that window, re-based to frame 0.
///
/// Returns `Ok` with `captures.len()` vectors, each of length `duration_samples` (possibly zero).
///
/// # Examples
///
/// ```ignore
/// use trem::graph::Graph;
/// use trem::render::render_captures;
///
/// let mut graph = Graph::new(512);
/// // ... build graph, `pad` and `duck` are NodeIds ...
/// let buf = render_captures(
///     &mut graph,
///     &[],
///     48_000,
///     48_000.0,
///     512,
///     &[(pad, 0), (pad, 1), (duck, 0), (duck, 1)],
/// )?;
/// ```
pub fn render_captures(
    graph: &mut Graph,
    events: &[TimedEvent],
    duration_samples: usize,
    sample_rate: f64,
    block_size: usize,
    captures: &[(NodeId, PortIdx)],
) -> Result<Vec<Vec<f32>>, PrepareError> {
    let block_size = block_size.max(1);
    let mut output = vec![vec![0.0f32; duration_samples]; captures.len()];

    let mut pos = 0;
    while pos < duration_samples {
        let frames = (duration_samples - pos).min(block_size);

        let block_events: Vec<TimedEvent> = events
            .iter()
            .filter(|e| e.sample_offset >= pos && e.sample_offset < pos + frames)
            .map(|e| TimedEvent {
                sample_offset: e.sample_offset - pos,
                event: e.event.clone(),
            })
            .collect();

        graph.run(frames, sample_rate, &block_events)?;

        for (ci, &(node, port)) in captures.iter().enumerate() {
            let buf = graph.output_buffer(node, port);
            output[ci][pos..pos + frames].copy_from_slice(&buf[..frames]);
        }

        pos += frames;
    }

    Ok(output)
}

/// Render a graph offline to sample buffers for one node's ports.
///
/// Same as [`render_captures`] with `block_size` [`DEFAULT_RENDER_BLOCK_SIZE`] and taps
/// `(output_node, p)` for each `p` in `output_ports`.
pub fn render(
    graph: &mut Graph,
    events: &[TimedEvent],
    duration_samples: usize,
    sample_rate: f64,
    output_node: NodeId,
    output_ports: &[PortIdx],
) -> Result<Vec<Vec<f32>>, PrepareError> {
    let captures: Vec<(NodeId, PortIdx)> = output_ports.iter().map(|&p| (output_node, p)).collect();
    render_captures(
        graph,
        events,
        duration_samples,
        sample_rate,
        DEFAULT_RENDER_BLOCK_SIZE,
        &captures,
    )
}

/// Convenience: render a pattern tree through a graph to stereo output (ports 0 and 1).
pub fn render_pattern(
    tree: &Tree<NoteEvent>,
    beats: Rational,
    bpm: f64,
    sample_rate: f64,
    scale: &Scale,
    reference_hz: f64,
    graph: &mut Graph,
    output_node: NodeId,
) -> Result<Vec<Vec<f32>>, PrepareError> {
    let duration_beats = beats.to_f64();
    let duration_seconds = duration_beats * 60.0 / bpm;
    let duration_samples = (duration_seconds * sample_rate).ceil() as usize;

    let events = tree_to_timed_events(tree, beats, bpm, sample_rate, scale, reference_hz);

    render(
        graph,
        &events,
        duration_samples,
        sample_rate,
        output_node,
        &[0, 1],
    )
}

/// Convert a Grid to timed events, assigning each column a fixed voice_id.
///
/// `voice_ids[col]` specifies the voice id for that grid column.
/// This lets each column address a different instrument in the graph.
pub fn grid_to_timed_events(
    grid: &Grid,
    beats: Rational,
    bpm: f64,
    sample_rate: f64,
    scale: &Scale,
    reference_hz: f64,
    voice_ids: &[u32],
    swing: f64,
) -> Vec<TimedEvent> {
    let mut events = Vec::new();
    let rows = grid.rows as i64;
    let step_beats = beats.to_f64() / grid.rows as f64;
    let swing_offset_samples =
        (swing * 0.5 * step_beats * (60.0 / bpm) * sample_rate).round() as usize;

    for col in 0..grid.columns {
        let vid = voice_ids.get(col as usize).copied().unwrap_or(col);
        let tree = grid.column_tree(col);
        let flat = tree.flatten();

        for fe in &flat {
            let step_idx = (fe.start * Rational::integer(rows)).floor();
            let swing_samples = if step_idx.rem_euclid(2) == 1 {
                swing_offset_samples
            } else {
                0
            };

            let beat_start = fe.start * beats;
            let gated_dur = fe.duration * fe.event.gate;
            let beat_end = (fe.start + gated_dur) * beats;
            let sample_on = beat_to_sample(beat_start, bpm, sample_rate) as usize + swing_samples;
            let sample_off = beat_to_sample(beat_end, bpm, sample_rate) as usize + swing_samples;
            let freq = resolve_frequency(fe.event, scale, reference_hz);
            let vel = fe.event.velocity.to_f64();

            events.push(TimedEvent {
                sample_offset: sample_on,
                event: GraphEvent::NoteOn {
                    frequency: freq,
                    velocity: vel,
                    voice: vid,
                },
            });
            events.push(TimedEvent {
                sample_offset: sample_off,
                event: GraphEvent::NoteOff { voice: vid },
            });
        }
    }

    events.sort_by(cmp_timed_event_delivery);
    events
}

#[cfg(test)]
mod loop_tests {
    use super::*;
    use crate::event::GraphEvent;

    #[test]
    fn loop_timed_events_two_bars() {
        let pattern = [TimedEvent {
            sample_offset: 0,
            event: GraphEvent::NoteOn {
                frequency: 100.0,
                velocity: 1.0,
                voice: 0,
            },
        }];
        let out = loop_timed_events(&pattern, 100, 250);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].sample_offset, 0);
        assert_eq!(out[1].sample_offset, 100);
        assert_eq!(out[2].sample_offset, 200);
    }
}

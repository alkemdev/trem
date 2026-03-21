//! Turn [`Tree`]s and [`Grid`]s into [`TimedEvent`]s and offline multi-block [`Graph`] renders.
//!
//! Beat positions are converted via [`crate::time::beat_to_sample`]; each flat leaf becomes a note on/off pair.

use crate::event::{GraphEvent, NoteEvent, TimedEvent};
use crate::graph::Graph;
use crate::grid::Grid;
use crate::math::Rational;
use crate::pitch::Scale;
use crate::time::beat_to_sample;
use crate::tree::Tree;

const DEFAULT_BLOCK_SIZE: usize = 512;

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

    events.sort_by_key(|e| e.sample_offset);
    events
}

/// Render a graph offline to sample buffers.
///
/// Returns one `Vec<f32>` per output channel of the final node in the graph.
/// The `output_node` and `output_ports` specify which node/ports to capture.
pub fn render(
    graph: &mut Graph,
    events: &[TimedEvent],
    duration_samples: usize,
    sample_rate: f64,
    output_node: u32,
    output_ports: &[u16],
) -> Vec<Vec<f32>> {
    let block_size = DEFAULT_BLOCK_SIZE;
    let num_channels = output_ports.len();
    let mut output = vec![vec![0.0f32; duration_samples]; num_channels];

    let mut pos = 0;
    while pos < duration_samples {
        let frames = (duration_samples - pos).min(block_size);

        // Collect events for this block, adjusting offsets to be block-relative
        let block_events: Vec<TimedEvent> = events
            .iter()
            .filter(|e| e.sample_offset >= pos && e.sample_offset < pos + frames)
            .map(|e| TimedEvent {
                sample_offset: e.sample_offset - pos,
                event: e.event.clone(),
            })
            .collect();

        graph.run(frames, sample_rate, &block_events);

        for (ch, &port) in output_ports.iter().enumerate() {
            let buf = graph.output_buffer(output_node, port);
            output[ch][pos..pos + frames].copy_from_slice(&buf[..frames]);
        }

        pos += frames;
    }

    output
}

/// Convenience: render a pattern tree through a graph to stereo output.
pub fn render_pattern(
    tree: &Tree<NoteEvent>,
    beats: Rational,
    bpm: f64,
    sample_rate: f64,
    scale: &Scale,
    reference_hz: f64,
    graph: &mut Graph,
    output_node: u32,
) -> Vec<Vec<f32>> {
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

    events.sort_by_key(|e| e.sample_offset);
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{Adsr, Gain, Oscillator, Waveform};
    use crate::pitch::Tuning;

    #[test]
    fn render_simple_pattern() {
        let scale = Tuning::edo12().to_scale();

        let tree = Tree::seq(vec![
            Tree::leaf(NoteEvent::simple(0)),
            Tree::rest(),
            Tree::leaf(NoteEvent::simple(4)),
            Tree::rest(),
        ]);

        let mut graph = Graph::new(DEFAULT_BLOCK_SIZE);
        let osc = graph.add_node(Box::new(Oscillator::new(Waveform::Saw)));
        let env = graph.add_node(Box::new(Adsr::new(0.005, 0.05, 0.3, 0.1)));
        let gain = graph.add_node(Box::new(Gain::new(0.5)));
        graph.connect(osc, 0, env, 0);
        graph.connect(env, 0, gain, 0);

        let output = render_pattern(
            &tree,
            Rational::integer(4),
            120.0,
            44100.0,
            &scale,
            440.0,
            &mut graph,
            gain,
        );

        assert_eq!(output.len(), 2);
        // At 120 BPM, 4 beats = 2 seconds = 88200 samples
        assert!(output[0].len() >= 88200);
        // Should have non-zero audio (the oscillator produces sound)
        let energy: f32 = output[0].iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "output should contain audio");
    }
}

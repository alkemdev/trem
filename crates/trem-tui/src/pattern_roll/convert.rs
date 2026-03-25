//! Bidirectional conversion between the step [`Grid`] and a beat-time [`trem::rung::Clip`] for the piano roll.

use num_rational::Rational64;
use trem::event::NoteEvent;
use trem::grid::Grid;
use trem::math::Rational;
use trem::pitch::Scale;
use trem::rung::{BeatTime, Clip, ClipNote, NoteMeta};

fn resolve_frequency(event: &NoteEvent, scale: &Scale, reference_hz: f64) -> f64 {
    use trem::pitch::Pitch;
    let pitch = scale.resolve(event.degree);
    let octave_pitch = Pitch(pitch.0 + event.octave as f64);
    octave_pitch.to_hz(reference_hz)
}

fn hz_to_midi_class(hz: f64) -> i32 {
    (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32
}

pub(crate) fn rational64_to_rational(r: Rational64) -> Rational {
    Rational::new(*r.numer(), *r.denom() as u64)
}

fn rational_to_rational64(r: Rational) -> Rational64 {
    Rational64::new(r.numer(), r.denom() as i64)
}

fn clip_note_from_cell(
    row: u32,
    col: u32,
    ne: NoteEvent,
    scale: &Scale,
    reference_hz: f64,
    voice_ids: &[u32],
) -> ClipNote {
    let voice = voice_ids.get(col as usize).copied().unwrap_or(col);
    let hz = resolve_frequency(&ne, scale, reference_hz);
    let class = hz_to_midi_class(hz).clamp(0, 127);
    let t_on = BeatTime(Rational64::from_integer(row as i64));
    let gate = rational_to_rational64(ne.gate);
    let t_off = BeatTime(t_on.rational() + gate);
    ClipNote {
        id: None,
        class,
        t_on,
        t_off,
        voice,
        velocity: ne.velocity.to_f64().clamp(0.0, 1.0),
        meta: NoteMeta::default(),
    }
}

/// Build a clip for the piano roll: one beat per grid row, loop length = `rows` beats (**all** columns).
pub fn clip_from_grid(grid: &Grid, scale: &Scale, reference_hz: f64, voice_ids: &[u32]) -> Clip {
    let rows = grid.rows as i64;
    let mut notes = Vec::new();

    for col in 0..grid.columns {
        for row in 0..grid.rows {
            let Some(ne) = grid.get(row, col).cloned() else {
                continue;
            };
            notes.push(clip_note_from_cell(
                row,
                col,
                ne,
                scale,
                reference_hz,
                voice_ids,
            ));
        }
    }

    notes.sort_by(|a, b| {
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.voice.cmp(&b.voice))
            .then_with(|| a.class.cmp(&b.class))
    });

    Clip {
        notes,
        length_beats: Some(BeatTime(Rational64::from_integer(rows))),
    }
}

/// One **voice lane** (step grid column): notes in that column only; same loop length as the grid.
pub fn clip_from_grid_column(
    grid: &Grid,
    scale: &Scale,
    reference_hz: f64,
    voice_ids: &[u32],
    column: u32,
) -> Clip {
    let rows = grid.rows as i64;
    let mut notes = Vec::new();
    if column < grid.columns {
        for row in 0..grid.rows {
            let Some(ne) = grid.get(row, column).cloned() else {
                continue;
            };
            notes.push(clip_note_from_cell(
                row,
                column,
                ne,
                scale,
                reference_hz,
                voice_ids,
            ));
        }
    }

    notes.sort_by(|a, b| {
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.class.cmp(&b.class))
    });

    Clip {
        notes,
        length_beats: Some(BeatTime(Rational64::from_integer(rows))),
    }
}

fn voice_to_col(voice: u32, voice_ids: &[u32], cols: u32) -> u32 {
    if cols == 0 {
        return 0;
    }
    voice_ids
        .iter()
        .position(|&v| v == voice)
        .map(|i| i as u32)
        .unwrap_or_else(|| (voice as usize % cols as usize) as u32)
}

fn midi_to_nearest_note_event(midi: i32, scale: &Scale, reference_hz: f64) -> NoteEvent {
    let target = 440.0 * 2.0_f64.powf((midi.clamp(0, 127) as f64 - 69.0) / 12.0);
    let mut best = NoteEvent::simple(0);
    let mut best_err = f64::MAX;
    for deg in -8i32..24 {
        for oct in -3..8 {
            let ne = NoteEvent::new(deg, oct, Rational::new(3, 4));
            let hz = resolve_frequency(&ne, scale, reference_hz);
            let err = (hz - target).abs();
            if err < best_err {
                best_err = err;
                best = ne;
            }
        }
    }
    best
}

fn place_in_column(grid: &mut Grid, preferred_row: u32, col: u32, ne: NoteEvent) {
    let rows = grid.rows;
    if rows == 0 || col >= grid.columns {
        return;
    }
    for dr in 0..rows {
        let r = (preferred_row + dr) % rows;
        if grid.get(r, col).is_none() {
            grid.set(r, col, Some(ne));
            return;
        }
    }
}

fn apply_sorted_clip_notes_to_columns(
    sorted: &[&ClipNote],
    grid: &mut Grid,
    scale: &Scale,
    reference_hz: f64,
    voice_ids: &[u32],
    fixed_column: Option<u32>,
) {
    let rows = grid.rows;
    let cols = grid.columns;

    for n in sorted {
        let b = n.t_on.rational();
        let bn = *b.numer();
        let bd = *b.denom();
        let row_i = if bd == 0 { 0 } else { bn.div_euclid(bd) }
            .clamp(0, rows.saturating_sub(1) as i64) as u32;
        let row = row_i;
        let col = fixed_column.unwrap_or_else(|| voice_to_col(n.voice, voice_ids, cols));
        if col >= cols {
            continue;
        }
        let dur = n.t_off.rational() - n.t_on.rational();
        let mut gate = rational64_to_rational(dur);
        if gate > Rational::one() {
            gate = Rational::one();
        }
        if gate <= Rational::zero() {
            gate = Rational::new(1, 64);
        }
        let mut ne = midi_to_nearest_note_event(n.class, scale, reference_hz);
        let v = (n.velocity.clamp(0.0, 1.0) * 1000.0).round() as i64;
        ne.velocity = Rational::new(v.max(0), 1000);
        ne.gate = gate;
        place_in_column(grid, row, col, ne);
    }
}

/// Replace grid contents from a clip edited in the roll (MIDI class → nearest scale degree).
pub fn apply_clip_to_grid(
    clip: &Clip,
    grid: &mut Grid,
    scale: &Scale,
    reference_hz: f64,
    voice_ids: &[u32],
) {
    let rows = grid.rows;
    let cols = grid.columns;
    for r in 0..rows {
        for c in 0..cols {
            grid.set(r, c, None);
        }
    }

    let mut sorted: Vec<&ClipNote> = clip.notes.iter().collect();
    sorted.sort_by(|a, b| {
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.voice.cmp(&b.voice))
    });

    apply_sorted_clip_notes_to_columns(&sorted, grid, scale, reference_hz, voice_ids, None);
}

/// Write roll notes into **one** step column; other columns unchanged.
pub fn apply_clip_to_grid_column(
    clip: &Clip,
    grid: &mut Grid,
    scale: &Scale,
    reference_hz: f64,
    voice_ids: &[u32],
    column: u32,
) {
    let rows = grid.rows;
    let cols = grid.columns;
    if column >= cols {
        return;
    }
    for r in 0..rows {
        grid.set(r, column, None);
    }

    let mut sorted: Vec<&ClipNote> = clip.notes.iter().collect();
    sorted.sort_by(|a, b| {
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.voice.cmp(&b.voice))
    });

    apply_sorted_clip_notes_to_columns(&sorted, grid, scale, reference_hz, voice_ids, Some(column));
}

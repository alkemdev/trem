//! Standard MIDI File (**SMF**) → [`Clip`].
//!
//! Only **channel voice** note messages are interpreted: **Note On** (velocity `1..=127`) and
//! **Note Off**, plus **Note On velocity `0`** (MIDI running “note off”). All other channel voice
//! traffic (control change, program change, channel pressure, polyphonic aftertouch, pitch bend)
//! and non-MIDI track events are ignored.
//!
//! Default mapping:
//! - **1 beat = 1 MIDI quarter note** → `BeatTime = tick / ppqn` (exact rational).
//! - **`class` = MIDI key number + `class_offset`** (default 7-bit `0..=127`).
//! - **`voice` = MIDI channel** (`0..=15`).
//! - **Velocity** `1..=127` → `velocity = vel / 127.0`.

use super::{BeatTime, Clip, ClipNote, NoteMeta, Provenance, RungError, RungFile};
use midly::{MidiMessage, Smf, TrackEventKind};
use num_rational::Rational64;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChannelNoteAction {
    KeyOn { key: u8, velocity: u8 },
    KeyOff { key: u8 },
}

/// Interprets a [`MidiMessage`] as a note on/off if it is one of the MIDI 1.0 note messages we
/// support; returns `None` for every other message type (filtered out).
fn note_action_from_midi_message(msg: &MidiMessage) -> Option<ChannelNoteAction> {
    match msg {
        MidiMessage::NoteOn { key, vel } => {
            let k = key.as_int();
            let v = vel.as_int();
            if v == 0 {
                Some(ChannelNoteAction::KeyOff { key: k })
            } else {
                Some(ChannelNoteAction::KeyOn {
                    key: k,
                    velocity: v,
                })
            }
        }
        MidiMessage::NoteOff { key, .. } => Some(ChannelNoteAction::KeyOff { key: key.as_int() }),
        _ => None,
    }
}

/// Options for [`import_midi`].
#[derive(Clone, Debug)]
pub struct MidiImportOptions {
    /// Added to every MIDI key to form `ClipNote::class`.
    pub class_offset: i32,
}

impl Default for MidiImportOptions {
    fn default() -> Self {
        Self { class_offset: 0 }
    }
}

/// Parse SMF bytes and build a [`Clip`].
pub fn import_midi(bytes: &[u8], opts: MidiImportOptions) -> Result<Clip, RungError> {
    let smf = Smf::parse(bytes).map_err(|e| RungError::Midi(e.to_string()))?;

    let ppq = match smf.header.timing {
        midly::Timing::Metrical(ppq) => ppq.as_int().max(1) as i64,
        midly::Timing::Timecode(_, _) => {
            return Err(RungError::Midi(
                "timecode timing not supported; use metrical (PPQN) MIDI files".into(),
            ));
        }
    };

    #[derive(Clone, Copy)]
    struct Pending {
        tick_on: u32,
        vel: u8,
    }

    let mut open: HashMap<(u8, u8), Pending> = HashMap::new();
    let mut notes: Vec<ClipNote> = Vec::new();
    let mut next_id: u64 = 1;
    let mut max_tick: u32 = 0;

    for track in &smf.tracks {
        let mut abs_tick: u32 = 0;
        for ev in track {
            abs_tick = abs_tick.saturating_add(ev.delta.as_int());
            max_tick = max_tick.max(abs_tick);

            let TrackEventKind::Midi { channel, message } = ev.kind else {
                continue;
            };
            let ch = channel.as_int();
            let Some(action) = note_action_from_midi_message(&message) else {
                continue;
            };

            match action {
                ChannelNoteAction::KeyOn { key, velocity } => {
                    open.insert(
                        (ch, key),
                        Pending {
                            tick_on: abs_tick,
                            vel: velocity,
                        },
                    );
                }
                ChannelNoteAction::KeyOff { key } => {
                    if let Some(p) = open.remove(&(ch, key)) {
                        push_note(
                            &mut notes,
                            &mut next_id,
                            ppq,
                            p.tick_on,
                            abs_tick,
                            ch,
                            key,
                            p.vel,
                            &opts,
                        );
                    }
                }
            }
        }
    }

    // Close hanging notes at end of file
    for ((ch, k), p) in open {
        push_note(
            &mut notes,
            &mut next_id,
            ppq,
            p.tick_on,
            max_tick,
            ch,
            k,
            p.vel,
            &opts,
        );
    }

    notes.sort_by(|a, b| {
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.voice.cmp(&b.voice))
            .then_with(|| a.class.cmp(&b.class))
    });

    let length_beats = Some(BeatTime(Rational64::new(max_tick as i64, ppq)));

    Ok(Clip {
        notes,
        length_beats,
    })
}

fn push_note(
    notes: &mut Vec<ClipNote>,
    next_id: &mut u64,
    ppq: i64,
    tick_on: u32,
    tick_off: u32,
    channel: u8,
    key: u8,
    vel: u8,
    opts: &MidiImportOptions,
) {
    if tick_off <= tick_on {
        return;
    }
    let id = *next_id;
    *next_id += 1;
    let t_on = BeatTime(Rational64::new(tick_on as i64, ppq));
    let t_off = BeatTime(Rational64::new(tick_off as i64, ppq));
    let class = opts.class_offset + i32::from(key);
    let vel_f = (f64::from(vel) / 127.0).clamp(0.0, 1.0);
    notes.push(ClipNote {
        id: Some(id),
        class,
        t_on,
        t_off,
        voice: u32::from(channel),
        velocity: vel_f,
        meta: NoteMeta::default(),
    });
}

/// Import MIDI bytes into a full [`RungFile`] with provenance filled in.
pub fn import_midi_file(bytes: &[u8], opts: MidiImportOptions) -> Result<RungFile, RungError> {
    let clip = import_midi(bytes, opts)?;
    let mut file = RungFile::new(clip);
    file.provenance = Some(Provenance {
        source: Some("midi".into()),
        mapping: Some(
            "class=midi_key+class_offset voice=channel beat=tick/ppqn (quarter=1 beat); \
             messages=NoteOn/NoteOff+vel0-only"
                .into(),
        ),
    });
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal SMF type 0: one track, middle C quarter at tick 0, ppq=480.
    fn minimal_midi_quarter() -> Vec<u8> {
        // Delta 480 ticks = VLQ bytes 0x83 0x60.
        vec![
            0x4d, 0x54, 0x68, 0x64, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x01, 0xe0,
            0x4d, 0x54, 0x72, 0x6b, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x90, 0x3c, 0x40, 0x83, 0x60,
            0x80, 0x3c, 0x00, 0x00, 0xff, 0x2f, 0x00,
        ]
    }

    #[test]
    fn import_one_note() {
        let bytes = minimal_midi_quarter();
        let clip = import_midi(&bytes, MidiImportOptions::default()).unwrap();
        assert_eq!(clip.notes.len(), 1);
        let n = &clip.notes[0];
        assert_eq!(n.class, 60);
        assert_eq!(n.voice, 0);
        assert_eq!(n.t_on, BeatTime::new(0, 1));
        assert_eq!(n.t_off, BeatTime::new(1, 1)); // 480/480 beats = 1 quarter
    }

    /// Type 0, one track: note on, **control change** (filtered), then note off after 480 ticks.
    fn midi_quarter_with_spurious_cc() -> Vec<u8> {
        vec![
            0x4d, 0x54, 0x68, 0x64, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x01, 0xe0,
            0x4d, 0x54, 0x72, 0x6b, 0x00, 0x00, 0x00, 0x11, 0x00, 0x90, 0x3c, 0x40, 0x00, 0xb0,
            0x01, 0x7f, 0x83, 0x60, 0x80, 0x3c, 0x00, 0x00, 0xff, 0x2f, 0x00,
        ]
    }

    #[test]
    fn import_ignores_control_change() {
        let clip = import_midi(
            &midi_quarter_with_spurious_cc(),
            MidiImportOptions::default(),
        )
        .unwrap();
        assert_eq!(clip.notes.len(), 1);
        assert_eq!(clip.notes[0].t_off, BeatTime::new(1, 1));
    }
}

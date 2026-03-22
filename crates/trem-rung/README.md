# trem-rung (Rung format)

Rust crate **`trem-rung`** implements the **Rung** interchange format: notes live on a **time × class** grid (see [`prop/piano-roll-editor-model.md`](../../prop/piano-roll-editor-model.md)).

- **File:** JSON, conventional suffix **`.rung.json`** (or `.rung` if you prefer).
- **Purpose:** share patterns between tools, write **reusable transforms** (serde types in this crate), and **import MIDI** into the same representation.

## On-disk shape

Top-level envelope:

```json
{
  "format": "rung",
  "schema_version": 1,
  "clip": { "notes": [], "length_beats": null },
  "provenance": null
}
```

- **`class`** (integer): vertical row index — meaning comes from your **ladder** / host (not from this file).
- **`t_on` / `t_off`**: **beats** as exact fractions `"numerator/denominator"` (e.g. `"1/4"`, `"3/1"`).
- **`voice`**: unsigned lane (e.g. polyphony / instrument).
- **`velocity`**: `0.0..=1.0` in the interchange layer.
- **`meta`**: `[[param_id, value], …]` float pairs.

## MIDI import

Enable feature **`midi`**. **Mapping (simple, documented default):**

| MIDI | Rung |
|------|------|
| Note number `0..=127` | `class = note + class_offset` (default offset `0`) |
| Channel `0..=15` | `voice = channel` |
| Velocity `1..=127` | `velocity = vel / 127` |
| Time | **1 beat = 1 MIDI quarter note** → `t = tick / ppqn` as an exact rational |

Tempo map is **not** applied to warp wall-clock; timings stay in **quarter-note beat space** (standard for DAW interchange). A future option could scale beats from tempo events.

```rust
use trem_rung::{Clip, midi};

let bytes = std::fs::read("piece.mid")?;
let clip = midi::import_midi(&bytes, midi::MidiImportOptions::default())?;
```

From the **trem** repo root (bare **`cargo run`** starts the synth TUI; add **`-- rung …`** for these):

```bash
cargo run -- rung import piece.mid -o piece.rung.json
cargo run -- rung edit piece.rung.json   # piano roll + looped preview (16 analog voices; class→MIDI pitch)
```

## Crate API

- Package name: **`trem-rung`** (Rust import: **`trem_rung`**).
- `Clip`, `ClipNote`, `NoteMeta`, `RungFile` — serde-ready
- `RungFile::to_json` / `from_json`
- Optional: `midi::import_midi`

## Naming

- **Crate `trem-rung`** — matches `trem`, `trem-cpal`, `trem-tui` in the workspace.
- **Format “Rung”** — one step on a ladder; the vertical axis in the editor model is a **class index**, not “pitch” until a ladder resolves it. On-disk `format` field stays **`"rung"`**.

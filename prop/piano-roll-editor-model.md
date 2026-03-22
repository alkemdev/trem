# Piano roll / clip editor — shared model (iteration 2)

**Status:** proposal · **not implemented**  
**Prev:** iteration 1 — same goals, tighter API sketch + UX + edge cases.

---

## Executive summary

1. **Clip** = notes with **`pitch: i32`** (opaque), **`t_on` / `t_off`** in beats ([`Rational`](crates/trem/src/math.rs)), **`voice`**, **`velocity`**, **`meta`**. No octave/degree on the note.
2. **`PitchSystem`** (pluggable) provides **`resolve` → Hz**, **highlight tiers** for the vertical grid, optional **labels** and **snap**. All tuning theory stays here.
3. **`MetaSchema`** lists **`MetaFieldDescriptor`**s (like graph [`ParamDescriptor`](crates/trem/src/graph.rs)); **inspector UI** is mandatory for “native” meta — not JSON-only.
4. **Today’s** [`NoteEvent`](crates/trem/src/event.rs) is an **adapter**: encode/decode `(degree, octave) ↔ pitch`, reuse **`params`** as `meta` floats.
5. **Implementation path:** types + schema + `PitchSystem` + inspector binding **before** a full graphical roll; TUI can ship inspector + list view first.

---

## 1. Clip and note (canonical)

### 1.1 Fields

| Field | Type (sketch) | Role |
|--------|----------------|------|
| `id` | `u64` (optional v1) | Stable selection / undo / drag |
| `pitch` | `i32` | Opaque index until `PitchSystem` interprets |
| `t_on`, `t_off` | `Rational` | Beats; **invariant:** `t_off > t_on` |
| `voice` | `u32` | Synth lane / polyphony target |
| `velocity` | match [`NoteEvent`](crates/trem/src/event.rs) | Keep one policy (e.g. `Rational` in [0,1]) |
| `meta` | `NoteMeta` | §2 |

### 1.2 Clip container

```text
Clip {
    notes: Vec<ClipNote>,
    length_beats: Rational,      // loop/export horizon (optional if inferred from max t_off)
    // future: automation curves
}
```

**Ordering:** sorted by `(t_on, voice, id)` for playback; editor may keep insertion order for undo — implementation detail.

### 1.3 Pseudo-Rust

```rust
// Illustrative only — names and crate placement TBD.
struct ClipNote {
    id: Option<u64>,
    pitch: i32,
    t_on: Rational,
    t_off: Rational,
    voice: u32,
    velocity: Rational,          // or f64 if we align with GraphEvent
    meta: NoteMeta,
}

struct NoteMeta {
    /// Graph param overrides and other float slots (matches NoteEvent::params spirit).
    pairs: Vec<(u32, f64)>,
    /// v2+: enums, strings — only after schema knows how to edit them.
    // extras: ...
}
```

**Duplicate `(param_id, value)` keys:** policy = **last wins** on load **or** reject on save — pick one in impl; document in serde tests.

---

## 2. Metadata + native editing

### 2.1 Schema

`MetaSchema` is a **`Vec<MetaFieldDescriptor>`** (or map by id). Fields mirror graph params where possible:

- `id: u32`, `label`, `help`, `kind` (Float, Int, Bool, Choice { variants }), `min`/`max`/`step`, `default`.

**Per-voice schemas (recommended):** `voice` → `MetaSchema`. Drum lane vs melodic lane get different fields without forking the clip format.

### 2.2 Inspector (minimum “native”)

| Requirement | Detail |
|---------------|--------|
| Single selection | All declared fields → widgets bound to `meta.pairs` |
| Multi-selection | Value shown if **all agree**; else **∅ mixed**; edit **applies to all** |
| Undo | Each commit = one undo group; slider drag may coalesce (match graph edit) |
| Focus | Roll ↔ inspector; **Esc** returns to canvas |
| Undeclared ids | **Extra** table (read-only or string edit) so nothing is dropped on save |

### 2.3 Playback bridge

`clip → TimedEvent`: for each note onset, `PitchSystem::resolve(pitch, ref_hz)` + `NoteOn`/`NoteOff` voice routing; merge **declared** meta ids into whatever the graph expects (today: param overrides on the note event path). Undeclared ids: keep in file; engine ignores unless a node reads them.

---

## 3. `PitchSystem` trait (sketch)

Single object injected into editor + renderer.

```rust
trait PitchSystem {
    fn resolve(&self, pitch: i32, reference_hz: f64) -> f64;

    /// Vertical ruler: which integer rows get emphasis.
    fn highlight_tier(&self, pitch: i32) -> HighlightTier;

    fn label(&self, pitch: i32) -> Option<String> { None }

    /// Optional: magnetic rows when dragging / arrow nudging pitch.
    fn snap_pitch(&self, pitch: i32, direction: SnapDir) -> i32 { pitch }
}

enum HighlightTier { None, Weak, Strong }
enum SnapDir { Nearest, Up, Down }
```

**Periodic systems** can implement `highlight_tier` via `pitch.rem_euclid(period)` internally — no separate `period()` required on the trait if you don’t need it in the UI.

**Optional extension:** `fn transpose(&self, pitch: i32, diatonic_steps: i32) -> i32` for scale-aware ↑/↓ (else editor uses `pitch ± 1`).

---

## 4. View: roll projection (implementation notes)

- **X:** `t` beats ↔ pixels with zoom; snap to beat grid from transport (independent of pitch system).
- **Y:** integer **pitch** ↔ pixel row: **uniform row height** per index in the **visible** window `[pitch_min, pitch_max]`; `highlight_tier` picks line weight / background stripe.
- **Hit-test:** `(x,y) → (t, pitch)` then find note under cursor (z-order: shorter notes on top or voice order — define once).
- **No** baked-in “octave height = 12 rows”; if the system wants a 13-step period, the ruler shows 13 repeating visual pattern via `highlight_tier`.

---

## 5. Layer stack (one screen)

| Piece | Responsibility |
|--------|----------------|
| `Clip` | Data |
| `MetaSchema` per voice | What inspector shows |
| `PitchSystem` | Hz + ruler + optional snap/transpose |
| Roll view | Geometry + selection |
| Inspector | Meta + optional read-only pitch / time fields |
| Transport | BPM, loop |

---

## 6. Mapping from today’s stack

| Current | Clip world |
|---------|------------|
| [`NoteEvent::degree` + `octave`](crates/trem/src/event.rs) | `pitch = f(degree, octave, scale_len)` — **adapter** |
| [`NoteEvent::params`](crates/trem/src/event.rs) | `NoteMeta::pairs` |
| Graph `ParamDescriptor` | Rows in `MetaSchema` for that voice |
| [`Scale::resolve`](crates/trem/src/pitch.rs) | Inside `ScalePitchSystem::resolve` after decode |
| [`Grid`](crates/trem/src/grid.rs) | Other view; may **quantize** clip → lossy |

---

## 7. Editing ops (checklist)

- [ ] Note CRUD, drag move (time / pitch), resize end, split at playhead  
- [ ] `pitch` nudge ±1 or `PitchSystem::snap_pitch` / `transpose`  
- [ ] Time quantize to beat grid  
- [ ] Meta edit via inspector (+ bulk)  
- [ ] Copy/paste (clip fragment); warn if `PitchSystem` changed  
- [ ] Undo/redo for all of the above  

---

## 8. Decisions (iteration 2 recommendations)

| Topic | Recommendation |
|--------|----------------|
| Time canonical | `t_on` + `t_off` (derive duration) |
| Velocity type | Match `NoteEvent` / `GraphEvent` (avoid two representations) |
| Meta duplicate keys | **Last wins** on merge; normalize on save |
| Schema scope | **Per `voice`** |
| Fractional pitch | **Not** on v1 note; use a **schema field** (e.g. detune cents) if needed |
| Monophony | Per-voice flag in session or graph metadata — outside clip |

**Non-goals (v1):** MIDI file I/O, MPE, tempo map, automation lanes, clip → [`Tree`](crates/trem/src/tree.rs) compilation.

---

## 9. Acceptance criteria (when implemented)

- Same `Clip` bytes render **different Hz** when swapping `PitchSystem` only.  
- Same clip shows **different ruler emphasis** when swapping systems.  
- Inspector edits **round-trip** in project save without losing undeclared `u32` keys.  
- At least one test: `ScalePitchSystem` + bridge matches current `grid_to_timed_events` for a fixed pattern (golden or property).

---

## 10. Suggested build order

1. `NoteMeta`, `ClipNote`, `Clip` + serde + duplicate-key policy tests  
2. `MetaFieldDescriptor` / `MetaSchema` + “merge schema for selected voices” helper  
3. `PitchSystem` + `Edo12PitchSystem` + `ScalePitchSystem` (adapter)  
4. **Inspector panel** (TUI or GUI) wired to selection + undo  
5. Minimal roll or **timeline list** (same clip model)  
6. `clip_to_timed_events` + audio path experiment  
7. Project version bump  

---

*Iteration 2 — tighten further in `todo/` when this is accepted.*

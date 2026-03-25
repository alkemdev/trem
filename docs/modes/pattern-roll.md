# Mode: Pattern roll (SEQ fullscreen)

**User story:** As someone shaping a pattern, I want to leave the step grid, open a piano roll for **the voice lane I have selected** (not every lane at once), see **that lane’s notes** in continuous time and pitch, scrub with the **same playhead** as the rest of the app, hear **the rest of the pattern still playing** in preview, and **apply** back to **only that column** when I’m done.

**Parent context:** **SEQ → Navigate → Enter** opens this mode for **`cursor_col`** (the highlighted voice column). The roll edits a `trem::rung::Clip` slice of that column only; **Esc** merges into the step grid **without touching other columns**.

---

## Entry / exit

| Action | Behavior |
|--------|----------|
| **Enter** (SEQ NAV) | Open roll for **current voice column**; preview = full pattern with this column replaced by the roll; no undo snapshot on open. |
| **Esc** | Validate clip → **undo snapshot** → write **only that column** from the roll → `send_pattern` → close. |
| **?** | Full keymap overlay (same global help component). |

**Future (not v1):** explicit **Cancel** (discard roll edits, no snapshot) vs **Apply**—requires UX + binding decision.

---

## Transport & playhead (v1 shipped / v2 polish)

| Requirement | Notes |
|-------------|--------|
| **Single transport** | Space play/pause uses the same bridge commands as the main UI. |
| **Playhead** | Vertical highlight (ruler + note area) at `beat_position` **wrapped** to pattern length (`grid.rows` as beats, matching one row = one beat in grid export). |
| **Follow playhead** | **v2:** optional `Tab` or `'` to scroll viewport so playhead stays centered. |
| **Swing** | Merged preview uses the same **`grid_to_timed_events`** path as the main SEQ player (includes project swing). |

---

## Selection model

### v1 (current)

- **Single note** index: **f** / **b** cycle “primary” note.
- Only primary note is visually distinct for edits that depend on selection.

### v2 (target)

| Intent | Binding (proposal) | Behavior |
|--------|-------------------|----------|
| **Extend selection** | **Shift+f** / **Shift+b** | Add prev/next note in time order to set (or range in list). |
| **Select all** | **Ctrl+a** | All notes in clip. |
| **Clear selection** | **Escape** (when not “close modal”—needs **double-Esc** or **Ctrl+Esc** story) *or* **Ctrl+Shift+a** | Deselect all except keep primary for compatibility. |
| **Toggle note under “cursor”** | Click N/A in TUI—use **n** / **Shift+n** to move a **time cursor** across note starts and toggle membership. |
| **Marquee** (optional) | **v3:** two-corner mark (`mark` key + move + `mark` again) to select all notes intersecting rect. |

**Rule:** Any **mutation** applies to **all selected** notes unless we add a “solo primary” toggle.

---

## Mutation verbs (v2 target)

All deltas respect clip validation (positive length, ordering, etc.).

| Verb | Binding (proposal) | Notes |
|------|-------------------|--------|
| **Nudge time** | **`[`** / **`]`** (t_off) and **`,`** / **`.`** (t_on) **× selection** | Today: primary only → v2: all selected, same delta. |
| **Nudge pitch** | **`+`** / **`-`** (class) × selection | Same. |
| **Nudge velocity** | **`1`** / **`2`** × selection | Same. |
| **Nudge voice** | **`e`** / **`r`** × selection | Same. |
| **Move selection in time** | **`H`** / **`L`** (coarse beat or grid snap) **`Shift+H/L`** fine | **v2:** move all selected by same Δt; clamp at 0. |
| **Move selection in pitch** | **`K`** / **`J`** | **v2:** transpose all selected by same Δclass; clamp 0–127. |
| **Duplicate** | **`d`** | Copy selected notes, offset by **1 beat** (or snap), append to clip, select duplicates. |
| **Delete** | **`Del`** or **`x`** | Remove selected. |
| **Quantize** | **`q`** (cycle grid: 1/16, 1/8, 1/4) | **v2:** quantize **t_on** (and optionally **t_off** length preserve). |
| **Legato / gap** | **`;`** / **`'`** | Stretch **t_off** to next note or add gap—**v2+**. |

---

## Navigation (viewport)

| Key | Action |
|-----|--------|
| **h** **l** | Pan time |
| **k** **j** | Pan pitch (scroll classes) |
| **z** **x** | Zoom time in/out |
| **g** | Center on primary selection |
| **a** | Fit all notes |

---

## Global chords while roll is open

| Chord | Behavior |
|-------|----------|
| **Space** | Play/pause |
| **s** | Re-sync preview (`LoadEvents`) |
| **Ctrl+C/Q** | Quit app |
| **?** | Help overlay; **Esc** closes help first |
| **Tab** | **v2 decision:** either no-op, or “follow playhead” toggle—avoid clashing with **Tab** = cycle editor unless we nest mode stack clearly |

---

## Implementation checklist

- [x] **v1:** Playhead column synced to `beat_position`, loop = pattern rows (beats).
- [x] **v1:** One **voice column** only; apply merges that column; preview merges roll + **grid snapshot at open** (other lanes + swing).
- [ ] **v2:** Multi-select set + apply mutations to set.
- [ ] **v2:** Duplicate / delete selection.
- [ ] **v2:** Quantize + optional follow playhead.
- [ ] **Future:** Cancel without apply; marquee; mouse (terminal permitting).

---

## Known limitations (honesty)

- Grid ↔ roll **pitch** goes through **MIDI class** and “nearest scale degree” on apply—microtonal / non-12-TEDO scales are approximate.
- **Other voice columns** in the roll preview are frozen from the grid **at open** until you close the roll (you can’t edit them without exiting).
- **`e` / `r` “voice”** in the roll still tweak `ClipNote.voice`, but **apply** always maps notes into the opened column; output routing follows that column’s `voice_ids[ col ]`, not the edited field.

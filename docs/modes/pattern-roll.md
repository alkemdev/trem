# Mode: ROL (SEQ fullscreen)

**User story:** As someone shaping a lane clip, I want to open a focused roll editor for that clip, move around it with a clear navigation model, select one or many notes, edit pitch/time/attrs without fighting the camera, keep the shared playhead visible, and apply back to the parent scene with one explicit exit.

**Parent context:** **Overview/SEQ → Enter** opens `ROL` for the selected clip or lane slice. `ROL` edits one clip at a time. **Esc** validates, applies, and returns to the parent surface.

---

## Entry / exit

- `Enter`: Open `ROL` for the selected clip.
- `Esc`: Validate clip, apply to the parent model, re-sync preview, and close `ROL`.
- `i`: Show the sidebar `INFO` pane.
- `?`: Show the sidebar `HELP` pane.

**Future:** explicit **Cancel** path and macro recording remain follow-up work.

---

## Shared transport

- `Single transport`: `Space` uses the same engine state as the rest of the app.
- `Playhead`: The ruler/grid show the shared playhead wrapped to the clip/scene loop.
- `Preview`: Project clips preview against the rest of the authored scene; grid clips preview against the step-grid snapshot.
- `Re-sync`: `s` rebuilds and reloads the preview pattern.

---

## Sidebar story

The right rail is a real sidebar now:

- `INFO` shows focus stack, current `ROL` submode, active attr field, selection count, and primary-note details.
- `HELP` shows the current `ROL` interaction map instead of a giant modal overlay.

This keeps fullscreen editing readable without hiding the main canvas behind a blocker.

---

## Submodes

`Tab` and `Shift+Tab` cycle `ROL` submodes:

- `PAN`: move the viewport; plain `hjkl` / arrows pan time and pitch.
- `JUMP`: move between note targets; plain `hjkl` / arrows jump to note neighbors.
- `EDIT`: transform the selection; plain `hjkl` / arrows move notes in time and pitch.
- `ATTR`: edit per-note attrs; plain `hjkl` / arrows change the active attr field or its value.

The rule is simple: **plain motion does one thing per submode**.

---

## Selection model

`ROL` has a **primary note** plus a **selection set**.

- `f / b`: Next / previous note in time order.
- `Shift+f / Shift+b`: Extend selection while moving in time order.
- `JUMP mode + Shift+hjkl`: Extend selection while jumping to directional note neighbors.
- `Ctrl+a`: Select all notes.

Any transform in `EDIT` or `ATTR` applies to the whole selected set unless documented otherwise.

---

## Movement and jumps

### PAN

- `h / l`: Pan time.
- `j / k`: Pan pitch.
- `Shift+hjkl`: Coarse pan.
- `z / x`: Zoom time in / out.
- `g`: Center on the primary note.
- `a`: Fit all notes.

### JUMP

- `h / l`: Previous / next note in time order.
- `j / k`: Lower / higher note neighbor by pitch proximity.
- `Shift+hjkl`: Same jumps, but extend selection.

---

## Editing

### EDIT

- `h / l`: Move selected notes by one snap step.
- `j / k`: Move selected notes by scale step.
- `Shift+h / Shift+l`: Move selected notes by one beat.
- `Shift+j / Shift+k`: Move selected notes by one octave.
- `Ctrl+Left / Ctrl+Right`: Snap selected notes to the previous / next note start.
- `[ / ]`: Shorten / lengthen note duration.
- `+ / -`: Semitone transpose.
- `d`: Duplicate the selection one beat later.
- `Del / Backspace`: Delete the selection.
- `n`: Insert a new note near the primary note or viewport center.

### ATTR

- `h / l`: Change the active attr field.
- `j / k`: Adjust the active attr field.
- `Shift+j / Shift+k`: Coarse attr adjustment.

Current attr fields:

- `VEL`
- `VOICE`
- `DUR`

Direct aliases stay available across submodes:

- `1 / 2` velocity down / up
- `e / r` voice down / up

---

## Known limitations

- Grid ↔ `ROL` pitch still round-trips through MIDI class plus nearest scale-degree mapping, so microtonal / non-12-TET authored intent is still approximate on apply.
- `VOICE` editing changes `ClipNote.voice`, but lane-based apply/preview rules still constrain how that is heard in some host paths.
- Quantize, cancel-without-apply, marquee selection, and macro recording are still TODOs.

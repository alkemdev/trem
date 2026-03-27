# Zone / Mode / Tool

`Zone / Mode / Tool` is the shared control language for the rebuilt TUI.

- **Zone** answers: "what kind of thing am I working on?"
- **Mode** answers: "what control grammar is active inside that zone?"
- **Tool** answers: "what operation is the keys cluster currently aimed at?"

This is intentionally **not** a rule that every zone must share the same mode names. A graph editor and a roll editor may both expose `EDIT`, but a roll may also ship `PAN` and `JUMP` because those are clearer for musical time/pitch work.

## Shared semantics

- `Tab` changes the most local mode when a zone has multiple working grammars.
- `Esc` walks back up the focus stack and should name the return target.
- `Space` always controls shared transport.
- `Shift+Enter` toggles fullscreen for the current view.
- `?` explains the current zone.
- `i` returns to the info pane.
- `Shift` means extend, coarse, or stronger motion.
- `Ctrl` means snap, structural action, or app-level command.

## Current live zones

### `PRJ` project root

- Intent: move around the project timeline and open deeper editors.
- Current surface: overview scene root.
- Current mode: `VIEW`
- Current tools:
- `block-focus` for lane/block navigation
- Planned future tools:
- `lane-shape` for resizing / ordering lanes
- `range` for time-span selection

### `GRF` graph

- Intent: route signal flow and tune processor parameters.
- Current modes:
- `VIEW` for node focus and graph traversal
- `PARAM` for parameter edits
- Current tools:
- `node-focus` for selecting nodes / nested graphs
- `param` for the active parameter
- Planned future tools:
- `cable` for explicit wiring gestures
- `macro` for grouped control surfaces

### `ROL` piano-roll style clip editor

- Intent: edit note events against the shared transport.
- Current modes:
- `PAN` for viewport motion
- `JUMP` for note-relative navigation and multiselect growth
- `EDIT` for moving / duplicating / resizing note selections
- `ATTR` for per-note attribute edits
- Current tools:
- `pan`
- `note-jump`
- `move`
- `velocity`
- `voice`
- `duration`
- Planned future tools:
- `draw`
- `erase`
- `marquee`
- `macro` (`TODO`)

## Planned zones

### `RAK` rack

- Intent: work with playable instruments, buses, sends, and reusable chains.
- Likely modes:
- `VIEW` for slot focus
- `PATCH` for insert / replace / reorder
- `MIX` for gain / pan / send controls
- Likely tools:
- `slot`
- `device`
- `send`
- `macro`

### `DSL` text / command layer

- Intent: structured command entry, textual patching, and automation.
- Likely modes:
- `TYPE`
- `COMPLETE`
- `REVIEW`
- Likely tools:
- `buffer`
- `completion`
- `command`

### `SMP` sample editor

- Intent: inspect and reshape recorded audio.
- Likely modes:
- `VIEW`
- `TRIM`
- `SLICE`
- `ATTR`
- Likely tools:
- `cursor`
- `range`
- `slice`
- `gain`
- `fade`

## Rules for adding a new zone

- Pick a short code first, then a human label.
- Keep mode names local to the zone if that makes the editor clearer.
- Keep tools narrower than modes; tools are what the next key cluster acts on.
- Show the active `Zone / Mode / Tool` in shell chrome.
- Add or update the matching doc in `docs/modes/`.

# Editing modes (TUI)

In **trem-tui**, the control model is **`Zone / Mode / Tool`**:

- **Zone** = what kind of thing you are editing (`PRJ`, `GRF`, `ROL`, ...)
- **Mode** = the local control grammar inside that zone
- **Tool** = the immediate operation the next key cluster targets

Modes stack on top of the same transport and project (BPM, play state, undo where applicable) but define their **own** keymap, selection model, and how changes commit back to the project.

This folder holds **user stories and input specs**—what each mode is for, how it feels to use, and the binding set we intend to converge on. Implementation may lag the doc; the doc is the target.

## Index

- [principles.md](./principles.md) — shared rules for every mode
- [zone-mode-tool.md](./zone-mode-tool.md) — shared control language across zones
- [pattern-roll.md](./pattern-roll.md) — SEQ -> fullscreen MIDI-style pattern roll

## Relationship to the app

- **Tab** switches the high-level editor (**SEQ** vs **GRAPH**); each has navigate / edit substates.
- **Modes** in this folder are **deeper surfaces** (e.g. pattern roll) opened from a parent context and closed with an explicit **commit** or **cancel** story (today: **Esc** = apply + close for pattern roll).
- See also: [tui-editor-roadmap.md](../tui-editor-roadmap.md) for planned modes.

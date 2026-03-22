# Editing modes (TUI)

In **trem-tui**, a **mode** is a fullscreen or primary-surface way of working on one kind of data. Modes stack on top of the same transport and project (BPM, play state, undo where applicable) but define their **own** keymap, selection model, and how changes commit back to the project.

This folder holds **user stories and input specs**—what each mode is for, how it feels to use, and the binding set we intend to converge on. Implementation may lag the doc; the doc is the target.

## Index

| Document | Mode |
|----------|------|
| [principles.md](./principles.md) | Shared rules for every mode |
| [pattern-roll.md](./pattern-roll.md) | SEQ → fullscreen MIDI-style pattern roll |

## Relationship to the app

- **Tab** switches the high-level editor (**SEQ** vs **GRAPH**); each has navigate / edit substates.
- **Modes** in this folder are **deeper surfaces** (e.g. pattern roll) opened from a parent context and closed with an explicit **commit** or **cancel** story (today: **Esc** = apply + close for pattern roll).
- See also: [tui-editor-roadmap.md](../tui-editor-roadmap.md) for planned modes.

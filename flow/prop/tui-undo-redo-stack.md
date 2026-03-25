# Unified undo/redo across TUI editors

## What

A single command stack (or per-surface stacks with clear UX) for pattern edits, graph parameter changes, and piano-roll edits, with discoverable keybindings.

## Why

Music editing without undo is fragile; users blame the tool first. Formalizing this early avoids incompatible per-editor histories.

## Notes

- Prefer coarse "transactions" (e.g. drag ends) over per-keystroke for piano roll.
- Consider memory caps for large clip edits.

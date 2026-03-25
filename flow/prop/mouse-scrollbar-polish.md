# Mouse support and scroll polish

## What

Audit crossterm mouse events for list panes, piano roll, and graph; add consistent click-to-focus, drag selection where feasible, scroll wheel mapping for zoom/scroll.

## Why

Many users run terminals with mouse; small UX wins reduce "keyboard-only cliff" without abandoning keyboard-first design.

## Notes

- Keep behavior identical across macOS Terminal, iTerm, Kitty, foot.
- Document conflicts with terminal copy mode.

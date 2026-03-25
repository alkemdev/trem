# TUI golden snapshot tests (buffer dumps)

## What

Extend `docs/tui-testing.md` ideas: deterministic small-terminal snapshots of key views (help overlay, empty graph) compared in tests.

## Why

Ratatui refactors silently break layout; golden tests catch regressions cheaply.

## Notes

- Pin terminal size; strip volatile timestamps if any.
- Keep count low — maintainability over coverage obsession.

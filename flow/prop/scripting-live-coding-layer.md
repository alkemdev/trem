# Scripting / live-coding layer (Rhai, Lua, or IPC)

## What

Embed a small scripting language *or* document a stable JSON-RPC over stdio to mutate graph/pattern at runtime for live coding performances.

## Why

Recompile cycles kill flow; many electronic musicians expect a REPL-like loop.

## Notes

- IPC keeps sandboxing simpler than embedding; embedding reduces latency.
- Must not violate audio-thread safety; all mutations via existing command bridge patterns.

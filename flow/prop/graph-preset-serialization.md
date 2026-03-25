# Graph preset serialization (save/load)

## What

Stable, versioned serialization for `Graph` topology, processor tags, and parameter values so users can save patches and share them (JSON or a small binary format).

## Why

Today the demo graph lives in Rust code; that does not scale for artists or regression fixtures. Presets also unlock automated graph diffing in tests.

## Notes

- Define a schema version field and migration story before v1.
- Consider separating "factory defaults" from "user snapshot" for merge semantics.

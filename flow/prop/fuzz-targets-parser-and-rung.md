# Fuzz targets (Rung JSON, future project files)

## What

`cargo fuzz` harnesses for Rung import and any future text/binary project formats; optionally fuzz `midly` paths behind `--features midi`.

## Why

User-supplied files are untrusted; panics on bad input erode trust. Fuzzing finds crashes before users do.

## Notes

- Start with `RungFile` parse + validate; add seeds from `assets/`.
- CI optional (fuzz is slow); nightly job is enough initially.

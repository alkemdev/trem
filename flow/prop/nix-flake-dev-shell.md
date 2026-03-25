# Nix flake for reproducible dev shells

## What

`flake.nix` providing Rust toolchain, ALSA libs on Linux, optional `midly` build inputs — matching `docs/install.md` prerequisites.

## Why

Lowers "works on my machine" friction for contributors; pairs well with headless CI matrix thoughts.

## Notes

- Keep optional; not everyone uses Nix.
- Document `direnv` one-liner in install doc if accepted.

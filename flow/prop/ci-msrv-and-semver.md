# CI: MSRV pin and semver checks

## What

Declare minimum supported Rust version in README and CI; optional `cargo-semver-checks` on release PRs for `trem` public API.

## Why

Library-first implies consumers; MSRV and API stability are part of the contract.

## Notes

- Align MSRV with dependencies (ratatui, cpal) — verify before advertising.
- Start with manual MSRV job; add semver tool when API surface stabilizes.

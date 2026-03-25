# OSC / UDP parameter control

## What

Listen on a localhost UDP port for OSC (or a simple JSON line protocol) messages that map to graph parameter IDs, same as the TUI sends via the cpal bridge.

## Why

Enables TouchOSC, Max/MSP, custom controllers, and automation without expanding the TUI. Useful for installations and live performance.

## Notes

- Reuse existing parameter addressing; avoid parallel naming schemes.
- Security: bind to `127.0.0.1` by default; document LAN risks.

# scripts

- **`tui-smoke.expect`** — builds `trem-bin`, runs `./target/debug/trem` under `expect`, sends **`q`** after a short delay. Use on a machine with a real TTY/audio. See `docs/tui-testing.md`.

```bash
expect scripts/tui-smoke.expect
```

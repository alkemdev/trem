# Trem Web TUI

Browser-hosted Trem TUI powered by WebAssembly.

## Overview

This app runs the same Trem TUI state/render logic in the browser using:

- `trem-web` (wasm entrypoint crate)
- `trem-tui` (shared app state + rendering)
- `ratzilla` (web terminal backend)
- React + Vite (web shell)

The web app currently serves the terminal route only.

## Prerequisites

- Rust toolchain
- `wasm-pack` (`0.14+`)
- Node.js (`18+`)
- `pnpm`

## Development

From the repo root:

1. Build the wasm package:

```bash
wasm-pack build crates/trem-web --target web --out-dir ../../web/pkg
```

2. Start the web app:

```bash
cd web
pnpm install
pnpm dev
```

3. Open the printed local URL (usually `http://localhost:5173`).

## Production build

```bash
# from repo root
wasm-pack build crates/trem-web --target web --out-dir ../../web/pkg

cd web
pnpm build
pnpm preview
```

## Notes

- Audio initialization is gated by browser autoplay policy. Click or press a key once in the page to start audio.
- If audio device init fails, the UI still runs (best-effort audio behavior).

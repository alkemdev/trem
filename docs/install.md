# Installing and running trem

**trem** is a Rust workspace. You need the **stable** Rust toolchain and a working **audio output** device for the full demo (real-time synthesis + terminal UI).

## 1. Install Rust

Use [rustup](https://rustup.rs/) (recommended on all platforms):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Windows (PowerShell): see https://rustup.rs/
rustup default stable
rustc --version   # should be 1.70+ (edition 2021)
```

No pinned MSRV today; if `cargo build` fails, `rustup update stable` first.

## 2. Clone and run

```bash
git clone <this-repository-url>
cd trem
cargo run
```

- **Binary name:** `trem` (workspace package `trem-bin`).
- **First run** compiles dependencies; later runs are fast.
- **Demo patch** lives in `src/demo/`; `src/main.rs` only wires graph + TUI.

### Prebuilt Linux binary (GitHub Actions)

On each successful CI run on `main` / `master` (and on pull requests), the workflow uploads **`trem-linux-x86_64`** as a workflow artifact:

1. Open the repo on GitHub → **Actions** → pick the run → **Artifacts** at the bottom.
2. Download and unzip; you get `trem` plus `README.txt`.
3. **Ubuntu / WSL2 (Ubuntu):** install runtime ALSA, then run:

   ```bash
   sudo apt install -y libasound2
   chmod +x trem
   ./trem
   ```

   The binary is **x86_64** glibc (same family as Ubuntu on WSL2). Very old distros might need a newer glibc or a local `cargo build` instead.

### Run tests (optional)

```bash
cargo test --workspace
cargo fmt --all -- --check
```

## 3. Platform notes

### macOS

- **Audio:** Uses CoreAudio via [cpal](https://crates.io/crates/cpal). Usually works out of the box.
- **Build:** Xcode Command Line Tools are enough (`xcode-select --install`) for the C toolchain some crates link to.
- **Terminal:** iTerm2, Terminal.app, or Kitty; use a **UTF-8** locale and a decent size (e.g. ≥100×28).

### Linux

- **Audio:** cpal commonly uses **ALSA**. Install dev headers so `alsa-sys` can link:

  | Distro        | Package (typical)   |
  |---------------|---------------------|
  | Debian/Ubuntu | `libasound2-dev`    |
  | Fedora        | `alsa-lib-devel`    |
  | Arch          | `alsa-lib`          |

  Also install **`pkg-config`** if the linker complains about missing `alsa`.

  **CI:** GitHub Actions installs `libasound2-dev` and `pkg-config` on Ubuntu before `cargo test` (see `.github/workflows/ci.yml`).

- **PipeWire / Pulse:** Often an ALSA compatibility layer is enough; if you have no device, check `pactl list short sinks` / system sound settings.

### Windows

- **Toolchain:** **MSVC** host (default rustup on Windows). Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with “Desktop development with C++” if `link.exe` errors appear.
- **Audio:** WASAPI via cpal; ensure a default playback device exists.

### WSL

- **Real-time audio + interactive TUI** under WSL1/WSL2 is **unreliable** (no native low-latency path like macOS/Linux desktop). Prefer **native Windows**, **native Linux**, or **macOS** for `cargo run`.

## 4. Using the TUI

- **Play / stop:** `Space`
- **Editors:** `Tab` cycles **SEQ** (step sequencer) ↔ **GRAPH** (signal graph)
- **Full keymap:** `?` in the app
- **Quit:** `q` (in some modes) or `Ctrl-C`
- **Save / load project (JSON):** `Ctrl-S` / `Ctrl-O` (default file `project.trem.json` in the current working directory)

Use a **real terminal** (not a minimal pipe); the UI uses **raw mode** and the **alternate screen**.

## 5. Troubleshooting

| Symptom | Things to try |
|---------|----------------|
| `Device not configured` / no audio | Check system output device, volume, exclusive mode; on Linux install `libasound2-dev` and retry `cargo run`. |
| Build errors in `alsa-sys` / `cpal` | Install ALSA dev packages + `pkg-config` (Linux). |
| Blank or garbled UI | Widen terminal; set `TERM=xterm-256color` if needed. |
| High **trem** CPU % in sidebar | Normal under load; number is **this process only**, can exceed **100%** on multi-core. |

## 6. Library-only (no audio device)

Core DSP and offline rendering do not need cpal:

```bash
cargo build -p trem
cargo test -p trem
cargo run -p trem --example offline_render
```

---

More for contributors: [AGENTS.md](../AGENTS.md), testing: [tui-testing.md](./tui-testing.md).

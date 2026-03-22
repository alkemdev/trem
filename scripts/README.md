# scripts

- **`tui-smoke.expect`** — builds `trem-bin`, runs `./target/debug/trem` under `expect`, sends **`q`** after a short delay. Use on a machine with a real TTY/audio. See `docs/tui-testing.md`.

```bash
expect scripts/tui-smoke.expect
```

- **`fetch_sankey_wtc_midi.py`** — downloads John Sankey’s WTC zips from [jsbach.net](http://www.jsbach.net/midi/midi_johnsankey.html) into **`assets/midi/wtc/sankey/`** (full Book I; Book II pairs 1–12 only). Writes **`PERMISSION-JOHN-SANKEY.txt`**. `--dry-run` supported.

```bash
python3 scripts/fetch_sankey_wtc_midi.py
```

- **`fetch_mutopia_wtc_midi.py`** — downloads Mutopia’s Bach WTC-range `.mid` files into **`assets/midi/wtc/mutopia/`** (partial set). `--dry-run` supported.

```bash
python3 scripts/fetch_mutopia_wtc_midi.py
```

- **`fetch_wtc_example_midis.py`** — runs **`fetch_sankey_wtc_midi.py`** then **`fetch_mutopia_wtc_midi.py`** (passes through args like `--dry-run`).

```bash
python3 scripts/fetch_wtc_example_midis.py
```

- **`generate_placeholder_samples.py`** — writes **`assets/samples/sine_440_250ms.wav`** (generated sine; no bundled third-party audio).

```bash
python3 scripts/generate_placeholder_samples.py
```

# trem

A mathematical music engine in Rust.

**trem** is a library-first DAW built on exact arithmetic, xenharmonic pitch
systems, recursive temporal trees, and typed audio graphs. The terminal UI is a
first-class citizen.

## Principles

- **Exact where possible.** Time is rational (integer numerator/denominator
  pairs). Pitch degree is an integer index into an arbitrary scale.
  Floating-point only appears at the DSP boundary.
- **Few assumptions.** No 12-TET default, no 4/4 default, no fixed grid
  resolution. Tuning, meter, and subdivision are all parameters.
- **Composition is a tree.** Patterns are recursive `Tree<Event>` structures.
  Children of `Seq` subdivide the parent's time span evenly. Children of `Par`
  overlap. Triplets, quintuplets, nested polyrhythms — just tree shapes.
- **Sound is a graph.** Audio processing is a DAG of typed processor nodes.
  Each processor declares its own inputs, outputs, and parameters. The graph is
  data: serializable, inspectable, editable at runtime.
- **Library first.** The core `trem` crate has zero I/O dependencies. It
  compiles to WASM. It renders offline to sample buffers. The TUI and audio
  driver are separate crates that depend on it.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  trem (core library, no I/O)                            │
│                                                         │
│  math::Rational ──▶ pitch::Scale ──▶ event::NoteEvent   │
│       │                                     │           │
│       ▼                                     ▼           │
│  time::Span ──▶ tree::Tree ──▶ render ──▶ TimedEvent    │
│                                             │           │
│  grid::Grid ──────────────────────┘         │           │
│                                             ▼           │
│  graph::Graph ◀── dsp::* ◀── euclidean    process()     │
│       │                                                 │
│       ▼                                                 │
│  output_buffer() ──▶ [f32]                              │
└────────────┬────────────────────────────────────────────┘
             │
     ┌───────┴───────┐
     ▼               ▼
┌─────────┐   ┌───────────┐
│trem-cpal│   │  trem-tui │
│         │   │           │
│ cpal    │◀──│ ratatui   │
│ stream  │cmd│ crossterm │
│         │   │           │
└─────────┘   └───────────┘
```

**trem** — Core library. Rational arithmetic, pitch/scale systems, temporal
trees, audio processing graphs, DSP primitives (oscillators, envelopes,
filters, effects), Euclidean rhythm generation, grid sequencer, and offline
rendering. No runtime dependencies beyond `bitflags`.

**trem-cpal** — Real-time audio backend. Drives a `Graph` from a cpal output
stream. Communicates with the UI via a lock-free ring buffer (`rtrb`): the UI
sends `Command`s (play, stop, set parameter), the audio thread sends back
`Notification`s (beat position, meter levels).

**trem-tui** — Terminal interface. Pattern sequencer with per-step note entry,
audio graph viewer with inline parameter editing, transport bar, waveform
scope, and contextual key hints. Built on ratatui + crossterm.

## Quick start

```bash
cargo run
```

This launches the demo project: a 130 BPM, A-minor loop with lead, bass, kick,
snare, and hats routed through an EQ → delay → reverb chain.

Press **Space** to play/stop. Press **Tab** to switch views.

## Keybindings

### Global (all views)

| Key           | Action              |
|---------------|---------------------|
| `Space`       | Play / stop         |
| `Tab`         | Cycle view          |
| `+` / `-`     | BPM up / down       |
| `[` / `]`     | Octave down / up    |
| `Ctrl-C`      | Quit                |

### Pattern view — Navigate mode

| Key           | Action              |
|---------------|---------------------|
| `←` `→`       | Move step cursor    |
| `↑` `↓`       | Move voice cursor   |
| `h` `l` `k` `j` | Vim-style move   |
| `e`           | Enter edit mode     |

### Pattern view — Edit mode

| Key           | Action              |
|---------------|---------------------|
| `z`–`m`       | Enter note (chromatic keyboard layout) |
| `0`–`9`       | Enter note by degree |
| `Del` / `BS`  | Delete note         |
| `w` / `q`     | Velocity up / down  |
| `f`           | Euclidean fill (cycle hit count) |
| `r`           | Randomize voice     |
| `t`           | Reverse voice       |
| `,` / `.`     | Shift voice left / right |
| `Esc`         | Back to navigate    |

### Graph view — Navigate mode

| Key           | Action              |
|---------------|---------------------|
| `←` `→`       | Follow connections  |
| `↑` `↓`       | Move within layer   |
| `e`           | Enter edit mode     |

### Graph view — Edit mode

| Key           | Action              |
|---------------|---------------------|
| `↑` `↓`       | Select parameter    |
| `←` `→`       | Adjust value (1%)   |
| `+` / `-`     | Fine adjust (0.1%)  |
| `Esc`         | Back to navigate    |

## DSP library

All processors implement the `Processor` trait and declare their parameters via
`ParamDescriptor`, enabling automatic UI generation for any frontend.

| Processor        | Description                                    |
|------------------|------------------------------------------------|
| `Oscillator`     | PolyBLEP oscillator (sine, saw, square, triangle) |
| `Adsr`           | Attack-decay-sustain-release envelope          |
| `Gain`           | Mono-to-stereo gain with pan                   |
| `StereoGain`     | Stereo pass-through gain                       |
| `MonoGain`       | Simple mono gain                               |
| `StereoMixer`    | N-input stereo summing bus with level          |
| `BiquadFilter`   | 2nd-order IIR (lowpass, highpass, bandpass)     |
| `Noise`          | White noise (deterministic LCG)                |
| `KickSynth`      | Sine with pitch sweep + amplitude envelope     |
| `SnareSynth`     | Sine body + bandpass-filtered noise burst      |
| `HatSynth`       | Highpass-filtered noise with short envelope    |
| `StereoDelay`    | Stereo delay with feedback and dry/wet mix     |
| `PlateReverb`    | Schroeder plate reverb (4 combs + 2 allpasses) |
| `ParametricEq`   | 3-band stereo parametric EQ                   |

## Building the library only

```bash
cargo build -p trem
```

## Running tests

```bash
cargo test --workspace
```

## Using as a library

```rust
use trem::dsp::{Oscillator, Adsr, Gain, Waveform};
use trem::graph::Graph;
use trem::pitch::Tuning;
use trem::event::NoteEvent;
use trem::math::Rational;

// Build a simple synth graph
let mut graph = Graph::new(512);
let osc = graph.add_node(Box::new(Oscillator::new(Waveform::Saw)));
let env = graph.add_node(Box::new(Adsr::new(0.01, 0.1, 0.3, 0.2)));
let gain = graph.add_node(Box::new(Gain::new(0.5)));
graph.connect(osc, 0, env, 0);
graph.connect(env, 0, gain, 0);

// Render offline
let scale = Tuning::edo12().to_scale();
let tree = trem::tree::Tree::seq(vec![
    trem::tree::Tree::leaf(NoteEvent::simple(0)),
    trem::tree::Tree::rest(),
    trem::tree::Tree::leaf(NoteEvent::simple(4)),
    trem::tree::Tree::rest(),
]);
let audio = trem::render::render_pattern(
    &tree, Rational::integer(4), 120.0, 44100.0,
    &scale, 440.0, &mut graph, gain,
);
// audio[0] = left channel, audio[1] = right channel
```

## License

MIT OR Apache-2.0

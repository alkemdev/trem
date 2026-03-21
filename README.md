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
  Each processor declares its own inputs, outputs, and parameters. Graphs nest
  recursively — a `Graph` is itself a `Processor`, so complex instruments and
  buses are single composable nodes.
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
│       │           registry                              │
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
filters, dynamics, effects), Euclidean rhythm generation, grid sequencer,
processor registry, and offline rendering. No runtime dependencies beyond
`bitflags` and `num-rational`.

**trem-cpal** — Real-time audio backend. Drives a `Graph` from a cpal output
stream. Communicates with the UI via a lock-free ring buffer (`rtrb`): the UI
sends `Command`s (play, stop, set parameter), the audio thread sends back
`Notification`s (beat position, meter levels).

**trem-tui** — Terminal interface. Pattern sequencer with per-step note entry,
audio graph viewer with inline parameter editing, transport bar, waveform
scope, spectrum analyzer, and contextual key hints. Built on ratatui +
crossterm.

## Quick start

```bash
cargo run
```

This launches the demo project: a 130 BPM, A-minor loop with lead, bass, kick,
snare, and hats routed through a nested bus architecture:

```
Lead > ────────┐
                ├── Inst Bus > ──┐
Bass > ────────┘                 │
                                  ├── Main Bus > ── [output]
Kick > ────┐                     │
Snare > ───┼── Drum Bus > ──────┘
Hat > ─────┘
```

Every node marked `>` is a nested graph you can Enter to inspect and edit.

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
| `Enter`       | Dive into nested graph |
| `Esc`         | Back up one level   |
| `e`           | Enter edit mode     |

### Graph view — Edit mode

| Key           | Action              |
|---------------|---------------------|
| `↑` `↓`       | Select parameter    |
| `←` `→`       | Adjust value        |
| `+` / `-`     | Fine adjust         |
| `Esc`         | Back to navigate    |

## DSP library

All processors implement the `Processor` trait and declare their parameters via
`ParamDescriptor`, enabling automatic UI generation for any frontend.

### Sources

| Tag    | Processor        | Description                                    |
|--------|------------------|------------------------------------------------|
| `osc`  | `Oscillator`     | PolyBLEP oscillator (sine, saw, square, triangle) |
| `noi`  | `Noise`          | White noise (deterministic LCG)                |
| `wav`  | `Wavetable`      | Wavetable oscillator with shape crossfade      |
| `kick` | `KickSynth`      | Sine with pitch sweep + amplitude envelope     |
| `snr`  | `SnareSynth`     | Sine body + bandpass-filtered noise burst      |
| `hat`  | `HatSynth`       | Highpass-filtered noise with short envelope    |
| `syn`  | `analog_voice`   | Composite synth graph (2 osc, filter, env, gain) |

### Effects & EQ

| Tag    | Processor        | Description                                    |
|--------|------------------|------------------------------------------------|
| `dly`  | `StereoDelay`    | Stereo delay with feedback and dry/wet mix     |
| `vrb`  | `PlateReverb`    | Schroeder plate reverb (4 combs + 2 allpasses) |
| `peq`  | `ParametricEq`   | 3-band stereo parametric EQ                   |
| `geq`  | `GraphicEq`      | 7-band mono graphic EQ                        |

### Dynamics

| Tag    | Processor        | Description                                    |
|--------|------------------|------------------------------------------------|
| `lim`  | `Limiter`        | Stereo brickwall limiter                       |
| `com`  | `Compressor`     | Stereo downward compressor                     |

### Filters & Modulators

| Tag    | Processor        | Description                                    |
|--------|------------------|------------------------------------------------|
| `lpf`  | `BiquadFilter`   | Low-pass biquad (2nd-order IIR)                |
| `hpf`  | `BiquadFilter`   | High-pass biquad                               |
| `bpf`  | `BiquadFilter`   | Band-pass biquad                               |
| `env`  | `Adsr`           | Attack-decay-sustain-release envelope          |
| `lfo`  | `Lfo`            | Low-frequency oscillator (sine, tri, saw, square) |

### Mixing & Utility

| Tag    | Processor        | Description                                    |
|--------|------------------|------------------------------------------------|
| `vol`  | `StereoGain`     | Stereo pass-through gain                       |
| `gain` | `MonoGain`       | Simple mono gain                               |
| `pan`  | `StereoPan`      | Stereo panning (equal-power)                   |
| `mix`  | `StereoMixer`    | N-input stereo summing bus                     |
| `xfade`| `MonoCrossfade`  | Mono crossfade between two inputs              |

## Processor registry

The `Registry` maps short tags to factory functions, so processors can be
instantiated at runtime without compile-time coupling:

```rust
use trem::registry::Registry;

let reg = Registry::standard();
let delay = reg.create("dly").unwrap();
println!("{}: {} in, {} out", delay.info().name, delay.info().sig.inputs, delay.info().sig.outputs);
```

## Nested graphs

A `Graph` implements `Processor`, so any graph can be a node inside another
graph. The demo project uses this to build self-contained instrument channels
(synth + level/pan in one node) and mix buses (mixer + dynamics + gain):

```rust
use trem::graph::{Graph, Processor, ParamGroup, GroupHint};
use trem::dsp;

let mut ch = Graph::labeled(512, "lead");
let osc = ch.add_node(Box::new(dsp::Oscillator::new(dsp::Waveform::Saw)));
let gain = ch.add_node(Box::new(dsp::Gain::new(0.5)));
ch.connect(osc, 0, gain, 0);
ch.set_output(gain, 2);

// Expose internal params to the parent graph
let g = ch.add_group(ParamGroup { id: 0, name: "Channel", hint: GroupHint::Level });
ch.expose_param_in_group(gain, 0, "Level", g);

// Now `ch` acts as a single stereo-output Processor
assert_eq!(ch.info().sig.outputs, 2);
```

In the TUI, press **Enter** on any nested graph node to dive in and edit its
internal parameters. Press **Esc** to return to the parent level. A breadcrumb
trail shows your current position (e.g. `Graph > Lead > Oscillator`).

## Examples

Runnable examples live in `crates/trem/examples/`:

```bash
cargo run -p trem --example offline_render   # render a pattern to samples
cargo run -p trem --example euclidean_rhythm  # generate and print euclidean patterns
cargo run -p trem --example xenharmonic       # explore tuning systems
cargo run -p trem --example custom_processor  # implement your own Processor
```

## Building the library only

```bash
cargo build -p trem
```

## Running tests

```bash
cargo test --workspace
```

## Benchmarks

```bash
cargo bench -p trem          # core, DSP, and graph benchmarks
cargo bench -p trem-tui      # spectrum analysis benchmarks
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

MIT

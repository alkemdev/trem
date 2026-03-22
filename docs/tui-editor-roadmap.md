# TUI editor roadmap (trem-tui)

The terminal UI is **modal**: one focused editor at a time, switched with **Tab** between **Pattern** and **Graph**. This doc is the plan for growing that system without abandoning that model.

## Principles

- **Cross-mode contract** — see [modes/principles.md](./modes/principles.md) for shared rules (transport, commit, selection-first, help).
- **One surface, one keymap family** — global chords (play, BPM, save, `?`, Tab) stay stable; each editor adds its own layer (like Pattern note keys vs Graph node/param keys).
- **Wire in order** — extend `Editor` in [`input.rs`](../crates/trem-tui/src/input.rs), route in `handle_key`, add a render arm in `App::draw` ([`app.rs`](../crates/trem-tui/src/app.rs)), document bindings in `HelpOverlay` ([`help.rs`](../crates/trem-tui/src/view/help.rs)), and add short sidebar hints in `InfoView` ([`info.rs`](../crates/trem-tui/src/view/info.rs)).
- **Scope / audio** — decide per editor what `ScopeFocus` (`trem-cpal`) and bottom **IN|OUT** behavior should be (Graph today uses dual scope; Pattern uses master only).

## Candidate editors (not implemented)

| Editor | Intent | Notes |
|--------|--------|--------|
| **Piano roll** | Pitch × time, MIDI-like | **Shipped** as SEQ fullscreen mode (`pattern_roll/`). Spec & v2 plan: [modes/pattern-roll.md](./modes/pattern-roll.md). |
| **Sample** | Waveform, regions, trim | Needs buffer model + bridge commands for playback pointer / selection. |
| **Arrange** | Song / clip timeline | Add a new widget + `Editor` when the engine exposes arrange/clip data (previous orphan stub was removed). |
| **Mixer / buses** | Levels, sends, mute/solo | Depends on host graph exposing bus strips or a dedicated snapshot. |

## Related

- Module note: [`crates/trem-tui/src/editor/mod.rs`](../crates/trem-tui/src/editor/mod.rs)

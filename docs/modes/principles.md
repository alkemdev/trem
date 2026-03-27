# Principles — all editing modes

These apply to **pattern roll**, future **sample editor**, **arrange**, **GRAPH** deep edits, etc. They are design constraints, not a promise that every mode implements all of them on day one.

## 1. One clear context

- The user always knows the active **Zone / Mode / Tool** (title, layout, footer hints).
- Normal operation uses a shared shell: top bar, right sidebar, and bottom bar.
- **Global** chords (Tab, `?`, Space, Ctrl+S, …) behave predictably or are explicitly **disabled** with a message when dangerous.

## 2. Transport is shared

- **Play/pause**, **BPM**, and **playhead** reflect **one** engine state.
- Modal editors **do not** fork a second timeline unless we add an explicit “offline / audition” story.
- While a mode is open, the UI should show **where playback is** (playhead) when that mode is time-based.

## 3. Commit vs discard

- Entering a mode **does not** imply saving; leaving must be explicit:
  - **Apply** (write through to parent model + host),
  - **Cancel** (revert working copy), or
  - **Stay** (rare; only if we add true non-modal panels).
- Today **pattern roll**: **Esc** = validate + apply to grid + close. (A future **cancel** path should be documented when added.)

## 4. Selection before mutation

- **Target** comes before **verb**: pick notes/nodes/regions, then move, nudge, delete, quantize, etc.
- **Primary selection** is obvious (highlight); **multi-select** uses a small, learnable set of modifiers (see per-mode spec).

## 5. Spatial consistency

- **Directional keys** mean the same thing within a mode: e.g. time = horizontal, pitch = vertical in piano-roll-like views.
- **Zoom/pan** reuse the same keys across similar modes when possible (`z`/`x`, `h`/`l`, …).

## 6. Discoverability

- A **one-line footer** lists the **most important** actions; **`?`** shows the full map.
- **`Shift+Enter`** toggles a canvas-first fullscreen state for the current view.
- **Sidebar** may stay hidden in fullscreen modes; the mode itself must carry a minimal HUD and keep help reachable.

## 7. Host and preview honesty

- If preview uses a **simplified** path (e.g. no swing, different loop semantics), the mode doc **states** it.
- Round-trip conversions (grid ↔ roll ↔ engine) document **known loss** (pitch class vs scale degree, etc.).

## 8. Testable routing

- Key routing for each mode should be **unit-testable** (action enum or pure handler), same as SEQ/GRAPH today.

## Mode template (for new docs)

When adding `docs/modes/<name>.md`, include:

1. **User story** — who, task, success criteria  
2. **Entry / exit** — how open, how commit/cancel  
3. **Selection model** — single, multi, marquee, time range  
4. **Mutation verbs** — move, resize, duplicate, delete, quantize, …  
5. **Transport** — playhead, follow, loop  
6. **Global chord matrix** — what still works, what is swallowed  
7. **v1 vs v2** — shipped vs planned  

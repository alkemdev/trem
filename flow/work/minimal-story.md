# Minimal end-to-end story (active)

**Status:** work  
**Updated:** 2026-03-22

## Goal

One **credible first session** for someone who just cloned the repo: they get sound,
can change something musical without reading architecture docs, and understand that
**SEQ** and **GRAPH** are two views on the same engine.

## User story

1. **Hear something immediately** — Run `cargo run` (or `trem`). Audio plays the default
   demo pattern and graph; transport shows play state. No config file required.
2. **Change the pattern** — From **SEQ**, move the cursor, **Enter** a step, edit a note
   or gate, **Esc** back; the next loop reflects the change.
3. **Touch the graph** — **Tab** to **GRAPH**, select a node, change one parameter
   (e.g. level or filter), hear the result.
4. **Optional clip path** — `trem clip import file.mid -o out.rung.json` produces Rung JSON;
   `trem clip edit out.rung.json` opens the piano-roll preview (TTY + audio).

## Acceptance criteria (v0)

- [ ] `cargo run` works on a clean clone after `docs/install.md` prerequisites (ALSA on Linux, etc.).
- [x] Default demo is audibly balanced from shared constants — see **`src/demo/levels.rs`** (single source of truth for trims/gains).
- [x] Help overlay (**`?`**) — **GLOBAL** block lists **Tab** (next editor) and **Space** (play/pause) first (`crates/trem-tui/src/view/help.rs`).
- [x] **`flow/done/`** holds archived completion write-ups; **`docs/`** stays reference-only (moved `documentation-and-examples-pass` here).

## Status log

- 2026-03-22: Relocated completed write-ups into `flow/done/`; this work item opened.

## Repo audit — trim & hygiene

### Done this pass

| Item | Action |
|------|--------|
| `docs/documentation-and-examples-pass.md` | Moved to **`flow/done/documentation-and-examples-pass.md`** (archived completion note; not user docs). |
| `flow/prop/trem-dsp-crate-split.md` | Moved to **`flow/done/trem-dsp-crate-split.md`** (work was already shipped). |

### Safe to keep

- **`flow/prop/*.md`** — Horizon ideas; volume is intentional until ideas merge or retire.
- **`docs/design-review-types-dod.md`** — Concrete follow-ups for types/TUI; complements **`docs/graph-architecture.md`**.
- **`docs/tui-editor-roadmap.md`** — Future editors; keep for product alignment.

### Candidates for later (do not block minimal story)

| Area | Suggestion |
|------|------------|
| `crates/trem/src/graph.rs` | Very large; split into `graph/` submodules when next touching topology or `Node` plumbing. |
| TUI `app.rs` | Same pattern: extract transport or graph controller when a second editor lands. |
| Duplicated “registry tag” lists | README vs code: generate table from `register_standard` in doc build (nice-to-have). |
| Benchmark names | Already renamed stale `new_processors` group in trem-dsp benches. |

### Dependencies (sanity)

- **`trem`**: `num-integer` used by `math::Rational` (`lcm`). Optional `serde` / `rung` / `wav` / `midi` are feature-gated.
- **`trem` dev-deps**: `clap` only needed for **`extreme_sidechain`** example; keep.

### Not removed (needs explicit product call)

- Individual **flow/prop** specs — deleting ideas without archiving loses design intent; prefer merging into epics later.

## Lessons learned

*(Fill when this item moves to `flow/done/`.)*

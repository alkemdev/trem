# Flow: structured development workflow

Flow is a lightweight, document-driven workflow for tracking ideas from inception
through completion. It matches the model used in the sibling **ezc** repo (`flow/`
directories and stage rules).

Every non-trivial change moves through these stages.

## Stages

```
prop ──► todo ──► plan ──► work ──► done
 idea    accepted  detailed  active  complete
          commit    design   impl    + lessons
```

## Directory structure

- `flow/prop/` — Proposals and ideas. Not yet accepted.
- `flow/todo/` — Accepted proposals. Committed to doing.
- `flow/plan/` — Detailed implementation plans (diagrams, ordered steps).
- `flow/work/` — Currently in progress. One developer or agent per item.
- `flow/done/` — Completed work items: implementation finished, tests pass, plus **Lessons Learned**.

### Not the same as `docs/`

The top-level **`docs/`** directory is **end-user and contributor reference** (install,
graph architecture, TUI testing, mode specs, etc.). It is **not** a flow stage.

**`flow/done/`** is the **decision log for shipped work items** (the old “move to `docs/` when
done” path referred here — not to `docs/install.md`).

## Document format

Filename: `NNN-slug.md` where `NNN` is a zero-padded sequence number (optional but
recommended for sort order). Many existing files use `slug.md` only; that is fine.

Suggested front matter and sections:

```markdown
# Title

Status: prop | todo | plan | work | done
Created: YYYY-MM-DD
Updated: YYYY-MM-DD

## Problem

## Proposal

## Alternatives

## Plan (at plan / work stage)

## Status log (at work stage)

## Lessons learned (at done stage)
```

## Rules

1. Non-trivial work starts as a proposal in `flow/prop/`.
2. Proposals need explicit acceptance before moving to `flow/todo/`.
3. Write a detailed plan in `flow/plan/` before implementation when the change is
   non-trivial; small fixes may go `todo` → `work` if the todo doc already spells out the steps.
4. Only one item in `flow/work/` per person or agent at a time when possible.
5. Done items in `flow/done/` are project memory — do not delete; refine with append-only notes.
6. Commit flow changes to git; history is the audit trail.

## Stage transitions

- **prop → todo**: Proposal accepted. Move file, update status.
- **todo → plan**: Flesh out implementation steps. Move file to `flow/plan/`, update status.
- **plan → work**: Start coding. Move file to `flow/work/`, add a status log.
- **work → done**: Complete, verified, tests pass. Move file to `flow/done/`, add lessons learned.

## For AI agents

Before starting work:

1. Check `flow/work/` for in-progress items.
2. Check `flow/prop/` and `flow/todo/` (and `flow/plan/`) for related proposals.
3. If nothing matches, create `flow/prop/<slug>.md`.
4. Follow stage transitions; do not skip acceptance before `todo/`.

Project commands and code standards: **`AGENTS.md`** at the repo root.

**Active narrative / UX bar:** [`flow/work/minimal-story.md`](work/minimal-story.md) (first-session story + repo trim notes).

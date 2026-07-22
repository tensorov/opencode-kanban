# Omo Integration Guide

## Architecture Overview

The omo integration follows an adapter pattern:

```
~/.omo/plans/*.md  →  FsPlanReader (implements PlanReader trait)  →  OmoAdapter  →  kanban UI
~/.omo/notepads/*/learnings.md  →  Notepad discovery  →  detail overlay
```

All omo code lives in `src/omo/` module.

## Module Structure

| Module | File | Purpose |
|--------|------|---------|
| `omo::types` | `src/omo/types.rs` | OmoPlan, PlanCard, OmoNotepad, PlanStatus types |
| `omo::reader` | `src/omo/reader.rs` | PlanReader trait definition |
| `omo::parser` | `src/omo/parser.rs` | Markdown plan file parser |
| `omo::fs_reader` | `src/omo/fs_reader.rs` | Filesystem-backed PlanReader |
| `omo::adapter` | `src/omo/adapter.rs` | Bridge between PlanReader and UI (caches + status) |
| `omo::notepad` | `src/omo/notepad.rs` | Notepad discovery and plan-to-notepad mapping |

## Plan Format

Plans use the standard ulw-plan markdown format:
- H1: `# <slug> - Work Plan`
- `## TL;DR` section with key-value pairs
- `## Scope` with `### Must have` / `### Must NOT have`
- `## <Section>` for tasks
- `- [ ] item` for open checklist, `- [x] item` for done

## Notepad Mapping

Notepads are at `~/.omo/notepads/<project>/learnings.md`. The mapping from plan slug to notepad project uses the first two hyphen-separated segments of the slug:
- `fintesla-planishche-feature-x` → project `fintesla-planishche`

## Auto-detect Behavior

At startup, the app checks for `~/.omo/plans/`. If found:
- omo_enabled = true
- Plans are loaded into the adapter
- "PLANS" column appears on the board

If not found:
- omo_enabled = false
- No omo UI elements are rendered

## Start-Work Flow

Pressing `w` in the plan detail overlay:
1. Sets plan status to Active (in-memory)
2. Creates a detached tmux session: `tmux new-session -d -s omo-{slug}`
3. Runs opencode in the working directory
4. Prepends a session marker to the matching notepad's learnings.md
5. Shows notification toast
6. User presses Enter on the card to attach (no auto-attach)

## Feature Flag

The omo module is gated behind `#[cfg(feature = "omo")]`. It is on by default.
Build without it: `cargo build --no-default-features`

# opencode-kanban

<p align="center">
  <img src="assets/kanban.jpg" alt="opencode-kanban board view" width="49%" />
  <img src="assets/detail.jpg" alt="opencode-kanban detail view" width="49%" />
</p>

A Rust terminal kanban board for managing Git worktrees and OpenCode tmux sessions.

## Why this exists
Before creating this tool, I used [Agent of Empires](https://www.agent-of-empires.com/) — which is also a cool project with a similar purpose. However, I found its session management quite barebone as projects grew more complex. I was also inspired by [VibeKanban](https://www.vibekanban.com/). So you can think of this tool as a combination of both - managing your tasks in a kanban without leaving your favorate terminal environment.

What makes this different: I'm building this exclusively for opencode users. This lets me integrate deeply with opencode's API and offer unique features:

1. Stable session running state detection
2. Session TODO list with progress tracking
3. Running subagents and their TODO summaries (when applicable)
4. And more to come 🚀

## Prerequisites

- Unix shell
- `tmux` installed and available on `PATH` (required)
- `opencode` installed and available on `PATH` (recommended for attach/resume workflows)

## Quickstart (2 minutes)
1. Verify runtime tools:

   ```bash
   tmux -V
   opencode --version
   ```

2. Start the app:

   ```bash
   npx @qrafty-ai/opencode-kanban
   ```

3. In the UI:
   - Press `n` to create a task
   - Press `Enter` on a task to attach
   - Press `?` for built-in help
   - Press `q` to quit
4. In the attached tmux session:
   - Press `<prefix>+O` for help overlay
   - Press `<prefix>+K` to return to kanban session

If you start outside tmux, `opencode-kanban` auto-creates or auto-attaches to a tmux session named `opencode-kanban`.

## Installation

### npm

```bash
npm install -g @qrafty-ai/opencode-kanban
```

### AUR (Arch Linux)

```bash
yay -S opencode-kanban
# or
paru -S opencode-kanban
```

### Build from source

```bash
cargo build --release
./target/release/opencode-kanban
```

## First run

- Launch default project:

  ```bash
  opencode-kanban
  ```

- Launch a named project:

  ```bash
  opencode-kanban --project my-project
  ```

- Start with a theme preset:

  ```bash
  opencode-kanban --theme default
  opencode-kanban --theme light
  opencode-kanban --theme high-contrast
  opencode-kanban --theme mono
  opencode-kanban --theme custom
  ```

Each project uses its own SQLite file and board state.

## Core workflows

### Start a new task

1. Press `n` to open the new-task dialog.
2. Pick a repository and enter task details.
3. Press `Enter` to create.
4. Press `Enter` on the task card to attach to its tmux/OpenCode session.

### Organize work on the board

- Move focus with `h`/`l` and select with `j`/`k`.
- Reorder/move task with `H`/`J`/`K`/`L`.
- Archive selected task with `a`.
- Open archive view with `A`.

### Track Task Progress
- Press `v` to toggle between detail/kanban view
- Check detail view for session running state in depth

## Omo Plans Integration

This fork adds support for [oh-my-openagent](https://github.com/oh-my-openagent/oh) (`.omo`) plan files.
It reads your plan files from `~/.omo/plans/*.md`, parses them, and renders a **PLANS** column on the kanban board alongside your regular tasks.

### Quick start (30 seconds)

1. **Create a plan file:**

```bash
mkdir -p ~/.omo/plans ~/.omo/notepads

cat > ~/.omo/plans/my-feature.md << 'EOF'
# my-feature - New Feature Plan

## TL;DR
**What you'll get:** A new feature that does X
**Estimated effort:** 3 days

## Scope
### Must have
- Feature implementation
- Tests

### Must NOT have
- Premature optimization

## Tasks
- [x] Research
- [ ] Implementation
- [ ] Tests
- [ ] Deploy

## Technical approach
Use the existing service layer, add a new handler.
EOF
```

2. **Start the kanban:** (must be inside tmux)

```bash
opencode-kanban
```

3. **Use plans in the UI:**

```
┌──────────┐ ┌───────────┐ ┌─────────────┐
│  TODO    │ │  DOING    │ │   PLANS     │
│          │ │           │ │             │
│ Task 1 ◄─┼─│ Task A    │ │ my-feature  │◀─ focused card
│          │ │           │ │ [1/3]       │
│          │ │           │ │ Drafting    │
└──────────┘ └───────────┘ └─────────────┘
                               ↑
                    ╔══════════╧══════════╗
                    ║  Detail Overlay     ║
                    ║                     ║
                    ║  my-feature —       ║
                    ║  New Feature Plan   ║
                    ║                     ║
                    ║  TL;DR: A new       ║
                    ║  feature that does X║
                    ║                     ║
                    ║  [✓] Research       ║
                    ║  [ ] Implementation ║
                    ║  [ ] Tests          ║
                    ║  [ ] Deploy         ║
                    ║                     ║
                    ║  Notepad: 23 entries║
                    ║                     ║
                    ║  Press w → start    ║
                    ╚═════════════════════╝
   ── PLANS column appears ──
   automatically when         Enter → open detail overlay
   ~/.omo/plans/ exists       w     → create tmux session + opencode
                              Esc   → close overlay
```

### Step-by-step guide

#### Step 1 — Create a plan

Place files at `~/.omo/plans/<slug>.md`. The parser recognizes these sections:

| Section | What gets parsed |
|---------|-----------------|
| `# slug — Title` | slug, title |
| `## TL;DR` | **key:** value metadata pairs |
| `## Scope / ### Must have` | scope items |
| `## Any section` | `- [ ]` / `- [x]` checklist items |

The first line **must** be `# <slug> - <Title>` where slug matches the filename (without `.md`).

#### Step 2 — Open the kanban

The **PLANS** column appears automatically when `~/.omo/plans/` exists on disk.

- **Enter** on a plan card → opens the detail overlay
- **j/k** inside the overlay → scroll content
- **Esc** → close the overlay

#### Step 3 — Start work (`w`)

Press **`w`** inside the detail overlay to start working on a plan:

1. Plan status changes to **Active**
2. A detached tmux session is created: `omo-<slug>`
3. `opencode` launches inside the session
4. A session marker is prepended to the matching `learnings.md` file
5. A toast notification appears at the bottom of the screen

To return from the working session back to the kanban, press `Prefix+K` (standard `switch-client -l`).

#### Step 4 — Notepad linkage

Notepad files live at `~/.omo/notepads/<project>/learnings.md`. The mapping from plan slug to notepad project uses the first two hyphen-separated segments:
- `my-feature-x` → project `my-feature`
- `fintesla-planishche-feature-y` → project `fintesla-planishche`

Notepad excerpts appear in the detail overlay under a **"📓 learnings.md"** heading.

### Plan file format (reference)

```markdown
# <slug> - <Title>

## TL;DR
**Key:** Value

## Scope
### Must have
### Must NOT have

## Any section
- [ ] open task
- [x] completed task
```

### Test drive with a temp plan

```bash
# Create a temporary plan outside ~/.omo
mkdir -p /tmp/demo-omo/plans
cat > /tmp/demo-omo/plans/test-plan.md << 'EOF'
# test-plan - Demo Plan

## TL;DR
**What you'll get:** A demo

## Tasks
- [ ] step one
- [x] step two
EOF

# Symlink so the kanban picks it up
ln -s /tmp/demo-omo ~/.omo

# Launch
opencode-kanban
```

### Status lifecycle

| Action | Effect | Status change |
|--------|--------|---------------|
| Press `w` in detail overlay | Start work session | Drafting → **Active** |
| (automatic) | Complete work | Active → **Completed** |

Status is tracked in-memory in the adapter. Restarting the app resets all plans to Drafting.

### Feature flag

The omo integration is compiled in by default. To build without it:

```bash
cargo build --no-default-features
```

### Keybindings (omo)

| Key | Context | Action |
|-----|---------|--------|
| `j` / `k` | Detail overlay | Scroll content |
| `Enter` | Plan card in PLANS | Open detail overlay |
| `w` | Detail overlay | Start work (tmux + opencode) |
| `Esc` | Detail overlay | Close overlay |

### Architecture

See `docs/omo-integration.md` for the full architecture guide — adapter pattern, module structure (`types`, `reader`, `parser`, `fs_reader`, `adapter`, `notepad`), plan format, and notepad linkage.

## Keybindings cheat sheet

- `Ctrl-p`: switch project
- `n`: new task
- `Enter`: attach selected task
- `h`/`j`/`k`/`l`: navigate board
- `H`/`J`/`K`/`L`: move task
- `a`: archive selected task
- `A`: open archive view
- `?`: help overlay
- `q`: quit

For full, current bindings, use the in-app help overlay (`?`).

## Configuration

- Settings file: `~/.config/opencode-kanban/settings.toml`
- Project databases (Linux default): `~/.local/share/opencode-kanban/*.sqlite`

The app creates config/data files on demand.

Additional top-level notification settings:

- `notification_backend`: `tmux` | `system` | `both` | `none`
- `notification_display_duration_ms`: `500..=30000`
- `completion_sound`: `none` | `beep` (defaults to `none`)
- `completion_sound_volume_percent`: `0..=100` (defaults to `100`; `0` skips audio init and playback)

Example:

```toml
notification_backend = "both"
notification_display_duration_ms = 3000
completion_sound = "beep"
completion_sound_volume_percent = 40
```

### Theme configuration options

Theme values live in `~/.config/opencode-kanban/settings.toml`.

Top-level option:

- `theme`: `default` | `light` | `high-contrast` | `mono` | `custom`

When `theme = "custom"`, configure semantic tokens with these sections:

- `[custom_theme]`
  - `inherit`: `default` | `light` | `high-contrast` | `mono`
- `[custom_theme.base]`
  - `canvas`, `surface`, `text`, `text_muted`, `header`, `accent`, `danger`
- `[custom_theme.interactive]`
  - `focus`, `selected_bg`, `selected_border`, `border`
- `[custom_theme.status]`
  - `running`, `waiting`, `idle`, `dead`, `broken`, `unavailable`
- `[custom_theme.tile]`
  - `repo`, `branch`, `todo`
- `[custom_theme.category]`
  - `primary`, `secondary`, `tertiary`, `success`, `warning`, `danger`
- `[custom_theme.dialog]`
  - `surface`, `input_bg`, `button_bg`, `button_fg`

Example:

```toml
theme = "custom"

[custom_theme]
inherit = "light"

[custom_theme.base]
canvas = "#E2E7EE"
surface = "#ECF1F7"
text = "#222A3A"
text_muted = "#4E596D"
header = "#2F66BF"
accent = "#0E7490"
danger = "#B02E24"

[custom_theme.interactive]
focus = "#2F66BF"
selected_bg = "#D6DFED"
selected_border = "#477ACD"
border = "#A5B2C6"

[custom_theme.status]
running = "#278449"
waiting = "#AB781A"
idle = "#5D687A"
dead = "#B02E24"
broken = "#B02E24"
unavailable = "#B02E24"

[custom_theme.tile]
repo = "#086678"
branch = "#926614"
todo = "#4E596D"

[custom_theme.category]
primary = "#2F66BF"
secondary = "#AB501F"
tertiary = "#6949AB"
success = "#278449"
warning = "#AB781A"
danger = "#B02E24"

[custom_theme.dialog]
surface = "#ECF1F7"
input_bg = "#E0E6EF"
button_bg = "#CDD8E7"
button_fg = "#FFFFFF"
```

- Accepted color format: `#RRGGBB` (hex only).

## Troubleshooting

- `tmux is required but not available`:
  - Install tmux and confirm `tmux -V` works in the same shell.
- `OpenCode binary not found`:
  - Install OpenCode and confirm `opencode --version` works.
- Mouse scroll/click not working well in tmux:
  - Run `tmux set -g mouse on`.

## Local development

```bash
cargo test
cargo clippy -- -D warnings
cargo build --release
```

## Maintainers

Release and publisher setup docs are in `docs/releasing.md`.

## License

MIT

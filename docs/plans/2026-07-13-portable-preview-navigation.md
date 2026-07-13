# Portable Preview Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace terminal-conflicting `Alt+1` through `Alt+5` preview shortcuts with wrapping `Tab` and `Shift+Tab` navigation.

**Architecture:** Keep preview ordering and scroll-reset behavior inside `TuiState`, where preview state already lives. Input handling will translate exact `Tab` and `BackTab` events into next/previous state transitions, while rendering and documentation will advertise the portable bindings.

**Tech Stack:** Rust, Crossterm key events, Ratatui, Cargo test suite

---

### Task 1: Add wrapping preview navigation to the input model

**Files:**
- Modify: `src/tui/input.rs`
- Modify: `src/tui/state.rs`

- [ ] **Step 1: Replace the Alt-number input test with failing Tab navigation tests**

In `src/tui/input.rs`, update the test import to include `PreviewMode` explicitly:

```rust
use crate::tui::state::{PreviewMode, ProjectScope};
```

Replace `alt_number_keys_switch_every_preview_mode` with these tests:

```rust
#[test]
fn tab_cycles_forward_through_preview_modes_and_wraps() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();
    let expected_modes = [
        PreviewMode::Conversation,
        PreviewMode::Tools,
        PreviewMode::Timeline,
        PreviewMode::Audit,
        PreviewMode::Overview,
    ];

    state.scroll_preview_down();
    for expected_mode in expected_modes {
        let action = handle_key(
            &database,
            &mut state,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        )
        .unwrap();

        assert!(matches!(action, TuiAction::Continue));
        assert_eq!(state.preview_mode(), expected_mode);
        assert_eq!(state.preview_scroll(), 0);
    }
}

#[test]
fn shift_tab_cycles_backward_through_preview_modes_and_wraps() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();
    let expected_modes = [
        PreviewMode::Audit,
        PreviewMode::Timeline,
        PreviewMode::Tools,
        PreviewMode::Conversation,
        PreviewMode::Overview,
    ];

    state.scroll_preview_down();
    for expected_mode in expected_modes {
        let action = handle_key(
            &database,
            &mut state,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
        )
        .unwrap();

        assert!(matches!(action, TuiAction::Continue));
        assert_eq!(state.preview_mode(), expected_mode);
        assert_eq!(state.preview_scroll(), 0);
    }
}
```

- [ ] **Step 2: Run the new tests and verify they fail**

Run:

```bash
cargo test tab_cycles_forward_through_preview_modes_and_wraps
cargo test shift_tab_cycles_backward_through_preview_modes_and_wraps
```

Expected: both tests fail because `handle_key` currently ignores `Tab` and `BackTab`.

- [ ] **Step 3: Add preview ordering operations to state**

In `src/tui/state.rs`, add private transitions beside `PreviewMode`:

```rust
impl PreviewMode {
    fn next(self) -> Self {
        match self {
            Self::Overview => Self::Conversation,
            Self::Conversation => Self::Tools,
            Self::Tools => Self::Timeline,
            Self::Timeline => Self::Audit,
            Self::Audit => Self::Overview,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Overview => Self::Audit,
            Self::Conversation => Self::Overview,
            Self::Tools => Self::Conversation,
            Self::Timeline => Self::Tools,
            Self::Audit => Self::Timeline,
        }
    }
}
```

Add these methods beside `set_preview_mode` in `impl TuiState` so all preview changes retain the existing scroll reset:

```rust
pub fn select_next_preview(&mut self) {
    self.set_preview_mode(self.preview_mode.next());
}

pub fn select_previous_preview(&mut self) {
    self.set_preview_mode(self.preview_mode.previous());
}
```

- [ ] **Step 4: Handle exact Tab and Shift+Tab events and remove Alt-number handling**

In `src/tui/input.rs`, remove `PreviewMode` from the production import:

```rust
use super::state::TuiState;
```

Delete the five `KeyCode::Char('1')` through `KeyCode::Char('5')` Alt match arms. Add these arms before printable-character handling:

```rust
KeyCode::Tab if key_event.modifiers == KeyModifiers::NONE => {
    state.select_next_preview();
    Ok(TuiAction::Continue)
}
KeyCode::BackTab if key_event.modifiers == KeyModifiers::SHIFT => {
    state.select_previous_preview();
    Ok(TuiAction::Continue)
}
```

The exact modifier guards ensure modified Tab combinations are not accidentally claimed.

- [ ] **Step 5: Run the input and TUI tests**

Run:

```bash
cargo test tui::input::tests
cargo test --test tui_tests
```

Expected: all input unit tests and all TUI integration tests pass.

- [ ] **Step 6: Commit the behavior change**

```bash
git add src/tui/input.rs src/tui/state.rs
git commit -m "fix: use tab for preview navigation"
```

### Task 2: Update the visible shortcut documentation

**Files:**
- Modify: `src/tui/render.rs`
- Modify: `README.md`

- [ ] **Step 1: Update the in-app help line**

In `src/tui/render.rs`, replace the preview shortcut portion of `render_help`:

```rust
let help = Paragraph::new(
    "Type search | Alt+I audit | Alt+E expand | Alt+R reindex | Tab/Shift+Tab previews | Alt+A all | Alt+P project | Enter resume | Esc quit",
)
.style(secondary_style());
```

- [ ] **Step 2: Update the README shortcut list and audit instructions**

In `README.md`, replace the five Alt-number bullets with:

```markdown
- `Tab` shows the next preview
- `Shift+Tab` shows the previous preview
```

Replace the TUI audit paragraph with:

```markdown
In the TUI, press `Alt+I` to run or refresh an audit for the selected session and open its audit preview.
```

- [ ] **Step 3: Verify stale shortcuts are gone from user-facing code and tests**

Run:

```bash
rg -n 'Alt\+[1-5]|Alt\+1-5' README.md src tests
```

Expected: no matches.

- [ ] **Step 4: Commit the documentation change**

```bash
git add README.md src/tui/render.rs
git commit -m "docs: update preview navigation shortcuts"
```

### Task 3: Run complete verification

**Files:**
- Verify: all changed files

- [ ] **Step 1: Check formatting**

Run:

```bash
cargo fmt --all -- --check
```

Expected: exit code 0 with no formatting differences.

- [ ] **Step 2: Run Clippy with warnings denied**

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: exit code 0 with no warnings.

- [ ] **Step 3: Run the full test suite**

Run:

```bash
cargo test --all-targets --all-features
```

Expected: every test passes.

- [ ] **Step 4: Verify the package and diff**

Run:

```bash
cargo package --allow-dirty --no-verify --offline
git diff --check
git status --short --branch
```

Expected: packaging and diff checks succeed and the feature branch working tree is clean.

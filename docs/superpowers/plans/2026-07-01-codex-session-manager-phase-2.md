# Codex Session Manager Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first usable terminal UI for browsing, searching, previewing, and resuming indexed Codex sessions with `cx`.

**Architecture:** The TUI is a thin layer over the existing SQLite-backed database and Codex command handoff. State management lives in a testable module, while terminal setup, drawing, and event handling stay isolated in the TUI runner.

**Tech Stack:** Rust 2021, `ratatui`, `crossterm`, existing `clap`, `rusqlite`, `anyhow`, `assert_cmd`, `tempfile`.

---

## Scope

This plan implements Phase 2 from the design spec:

- `cx` without a subcommand opens the TUI.
- The TUI lists recent sessions from the existing index.
- Typing filters sessions through the existing search API.
- The selected session preview shows metadata and recent messages.
- `Enter` exits the TUI and runs `codex resume <session-id>`.
- `q` and `Esc` quit.

This phase does not implement tags, archive/delete/fork wrappers, mouse support, saved filters, or semantic search.

## File Structure

- Modify `Cargo.toml`
  - Add `ratatui` and `crossterm`.
- Modify `src/cli.rs`
  - Make the subcommand optional so bare `cx` can start the TUI.
- Modify `src/main.rs`
  - Dispatch `None` to the TUI runner.
- Create `src/tui.rs`
  - Own terminal setup, drawing, key handling, and TUI state.
- Modify `tests/cli_tests.rs`
  - Add a non-interactive assertion that `cx --help` still works with optional commands.
- Create `tests/tui_tests.rs`
  - Test session loading, search filtering, selection movement, preview loading, and resume selection without opening a real terminal.
- Modify `README.md`
  - Document the TUI workflow and keys.

## Implementation Tasks

### Task 1: Make Bare `cx` a Valid CLI Shape

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `tests/cli_tests.rs`

- [ ] **Step 1: Write a failing CLI test**

Add a test to `tests/cli_tests.rs`:

```rust
#[test]
fn help_mentions_default_tui_workflow() {
    Command::cargo_bin("cx")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Run without a command to open the TUI"))
        .stdout(predicate::str::contains("Commands:"));
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cargo test --test cli_tests help_mentions_default_tui_workflow
```

Expected: FAIL because the help text does not describe the default TUI workflow.

- [ ] **Step 3: Implement the minimal CLI shape**

Change `src/cli.rs` so `command` is optional and the help text names the default workflow:

```rust
#[derive(Debug, Parser)]
#[command(name = "cx")]
#[command(about = "Find and resume Codex sessions")]
#[command(after_help = "Run without a command to open the TUI.")]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<PathBuf>,

    #[arg(long, global = true, value_name = "PATH")]
    pub sessions_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}
```

Change `src/main.rs` dispatch to match `Option<Commands>`. For `None`, call a temporary placeholder that bails with a clear message until `src/tui.rs` exists:

```rust
None => {
    anyhow::bail!("TUI is not wired yet");
}
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
cargo test --test cli_tests help_mentions_default_tui_workflow
```

Expected: PASS.

### Task 2: Add Testable TUI State

**Files:**
- Modify: `Cargo.toml`
- Create: `src/tui.rs`
- Modify: `src/main.rs`
- Create: `tests/tui_tests.rs`

- [ ] **Step 1: Add failing state tests**

Create `tests/tui_tests.rs` with tests that index fixture sessions into a temp database, construct `TuiState`, and assert:

- Initial state loads recent sessions.
- Updating the query to `turnstile` filters to the matching fixture.
- Moving down selects the next session when multiple rows exist.
- `selected_session_id()` returns the selected row id.

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test --test tui_tests
```

Expected: FAIL because `src/tui.rs` does not exist.

- [ ] **Step 3: Implement `TuiState`**

Create `src/tui.rs` with:

- `TuiState::load(database: &Database, limit: usize) -> Result<Self>`
- `TuiState::set_query(&mut self, database: &Database, query: String) -> Result<()>`
- `TuiState::move_down(&mut self)`
- `TuiState::move_up(&mut self)`
- `TuiState::selected_summary(&self) -> Option<&SessionSummary>`
- `TuiState::selected_session_id(&self) -> Option<&str>`

Keep all terminal-specific code out of this state type.

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run:

```bash
cargo test --test tui_tests
```

Expected: PASS.

### Task 3: Draw and Run the Terminal UI

**Files:**
- Modify: `src/tui.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add rendering and event code**

Add `ratatui` and `crossterm` dependencies. Implement:

- `run(database: &Database) -> Result<Option<String>>`
- a terminal guard that enables raw mode and alternate screen, restoring both on drop
- list rendering with timestamp, cwd, and title
- preview rendering for the selected session
- key handling:
  - printable chars append to the query
  - Backspace removes one query char
  - Up/Down or `k`/`j` move selection
  - Enter returns `Some(session_id)`
  - Esc or `q` returns `None`

- [ ] **Step 2: Wire bare `cx` to the TUI**

In `src/main.rs`, handle `None` by opening the database, running the TUI, and if it returns `Some(session_id)`, run the existing Codex resume command.

- [ ] **Step 3: Run build and tests**

Run:

```bash
cargo test
cargo build
```

Expected: PASS.

### Task 4: Document and Verify

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update docs**

Document:

```bash
cx
```

Keys:

- type to search
- `Backspace` edits search
- `Up`/`Down` or `k`/`j` moves selection
- `Enter` resumes
- `q` or `Esc` quits

- [ ] **Step 2: Run final verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

Expected: all commands exit 0.

## Plan Self-Review

- Spec coverage: The plan covers the Phase 2 TUI requirements for search, preview, and resume. It intentionally excludes Phase 3 management commands.
- Placeholder scan: No task contains open-ended TODO/TBD placeholders.
- Type consistency: The TUI state API names are stable across tests, implementation, and `main.rs` wiring.

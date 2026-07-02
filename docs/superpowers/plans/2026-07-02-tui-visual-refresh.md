# Codex Explorer TUI Visual Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the `cx` TUI so sessions are easier to recognize by task, project, and useful conversation context.

**Architecture:** Keep the existing Ratatui runner and database APIs. Add testable display helpers in `src/tui.rs`, then use those helpers to render a finder-style header, search bar, two-line list rows, sectioned preview, and accurate footer.

**Tech Stack:** Rust 2021, existing `ratatui`, `crossterm`, `rusqlite`, `assert_cmd`, and `tempfile`.

---

## File Structure

- Modify `src/tui.rs`
  - Add display helper functions for compact timestamps, paths, session IDs, result labels, empty-state text, and meaningful message previews.
  - Update rendering to use a header/search/body/footer layout.
  - Update list row and preview presentation.
- Modify `tests/tui_tests.rs`
  - Add focused tests for display helper behavior and TUI state labels.

## Tasks

### Task 1: Add Display Helper Tests

**Files:**
- Modify: `tests/tui_tests.rs`

- [ ] **Step 1: Add failing tests for result labels and display formatting**

Add tests that assert:

- empty query mode is `Recent`
- active query mode is `Search`
- result label uses `sessions` without a query and `matches` with a query
- session IDs shorten to first 8 and last 4 characters
- project paths prefer the final path segment
- timestamps compact from ISO format to `YYYY-MM-DD HH:MM`
- preview messages skip bootstrap/environment text and keep useful turns

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cargo test --test tui_tests
```

Expected: FAIL because the helper methods and functions do not exist.

### Task 2: Implement Testable Display Helpers

**Files:**
- Modify: `src/tui.rs`

- [ ] **Step 1: Implement helper methods and functions**

Add:

- `TuiState::mode_label(&self) -> &'static str`
- `TuiState::result_label(&self) -> String`
- `short_session_id(session_id: &str) -> String`
- `compact_path(path: &str) -> String`
- `compact_timestamp(timestamp: &str) -> String`
- `empty_results_message(query: &str) -> String`
- `meaningful_preview_messages(messages: &[ParsedMessage], limit: usize) -> Vec<&ParsedMessage>`

- [ ] **Step 2: Run focused tests and verify GREEN**

Run:

```bash
cargo test --test tui_tests
```

Expected: PASS.

### Task 3: Refresh TUI Rendering

**Files:**
- Modify: `src/tui.rs`

- [ ] **Step 1: Update layout and visual hierarchy**

Change rendering to:

- vertical sections for header, search, body, footer
- horizontal body split with 42 percent list and 58 percent preview
- styled header with product name, mode, and result count
- search box with placeholder text when empty
- two-line session rows with title first and metadata second
- sectioned preview with `Task`, `Context`, `Conversation`, and `Resume`
- footer that accurately documents key behavior

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

Expected: all commands exit 0.

## Self-Review

- Spec coverage: The plan covers the visual refresh design without adding tags, semantic summaries, mouse support, or database migrations.
- Placeholder scan: No task depends on unspecified future behavior.
- Type consistency: Helper names in tests and implementation tasks match.

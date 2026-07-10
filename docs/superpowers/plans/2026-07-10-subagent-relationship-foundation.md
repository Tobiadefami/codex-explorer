# Subagent Relationship Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retain Codex subagent sessions in the local index, link them to their parents, keep them out of default top-level results, and expose child counts in the existing TUI.

**Architecture:** Extend the existing parsed session and SQLite projection with relationship metadata. Index every valid Codex thread, apply root-only filtering in browse/search queries, and derive child counts in summary queries. This is the first incremental slice toward the approved modular-monolith architecture.

**Tech Stack:** Rust 2021, rusqlite, SQLite FTS5, serde, Ratatui, fixture-driven integration tests.

---

### Task 1: Characterize Retained Subagent Indexing

**Files:**
- Modify: `tests/index_tests.rs`
- Modify: `src/indexer.rs`

- [ ] **Step 1: Replace the deletion expectation with a failing retention test**

Update the existing subagent indexing test to require two indexed records while keeping only the parent in `list_sessions`:

```rust
assert_eq!(report.scanned_files, 2);
assert_eq!(report.indexed_sessions, 2);
assert_eq!(database.list_sessions(10).unwrap().len(), 1);
assert!(database
    .get_session("33333333-3333-4333-8333-333333333333")
    .unwrap()
    .is_some());
```

- [ ] **Step 2: Run the focused test and verify the expected failure**

Run:

```bash
cargo test --test index_tests reindex_retains_subagent_threads_outside_top_level_sessions
```

Expected: FAIL because the indexer reports one indexed session and deletes the child.

- [ ] **Step 3: Index every parsed session**

Remove the `is_subagent_thread` deletion branch from `indexer::reindex`, allowing the existing upsert path to persist child sessions.

- [ ] **Step 4: Run the focused test again**

Run the command from Step 2.

Expected: the indexed count assertion passes; the top-level list assertion may still fail until Task 3 adds root filtering.

### Task 2: Persist Session Relationships

**Files:**
- Modify: `src/db.rs`
- Modify: `tests/index_tests.rs`

- [ ] **Step 1: Add a failing relationship round-trip assertion**

After indexing the subagent fixture, inspect it and assert:

```rust
let child = database
    .get_session("33333333-3333-4333-8333-333333333333")
    .unwrap()
    .unwrap();
assert_eq!(
    child.summary.parent_thread_id.as_deref(),
    Some("11111111-1111-4111-8111-111111111111")
);
assert_eq!(child.summary.thread_source.as_deref(), Some("subagent"));
```

- [ ] **Step 2: Run the focused test and verify it fails to compile**

Run:

```bash
cargo test --test index_tests reindex_retains_subagent_threads_outside_top_level_sessions
```

Expected: compilation fails because `SessionSummary` does not expose relationship fields.

- [ ] **Step 3: Add relationship columns and summary fields**

Add nullable `parent_thread_id` and `thread_source` columns to `sessions`, add them through `add_column_if_missing` for existing databases, persist them in `upsert_session`, and load them in `summary_from_row`.

- [ ] **Step 4: Run the focused relationship test**

Run the command from Step 2.

Expected: relationship assertions pass.

### Task 3: Apply Root-Only Browse and Search Policy

**Files:**
- Modify: `src/db.rs`
- Modify: `tests/index_tests.rs`

- [ ] **Step 1: Add failing root-only query assertions**

Require `list_sessions`, `list_sessions_for_cwd`, `search_sessions`, and `search_sessions_for_cwd` not to return a child as a top-level result. Direct `get_session(child_id)` must still return it.

- [ ] **Step 2: Run the relevant index tests and verify the child leaks into results**

Run:

```bash
cargo test --test index_tests subagent
```

Expected: at least one root-only query assertion fails.

- [ ] **Step 3: Add root predicates to summary queries**

Use this predicate in top-level session queries:

```sql
COALESCE(thread_source, '') <> 'subagent'
```

Keep `get_session` unfiltered so child details remain addressable.

- [ ] **Step 4: Run all index tests**

Run:

```bash
cargo test --test index_tests
```

Expected: all index tests pass.

### Task 4: Add Child Counts to Parent Summaries

**Files:**
- Modify: `src/db.rs`
- Modify: `tests/index_tests.rs`

- [ ] **Step 1: Add a failing child-count assertion**

Assert the returned parent summary contains one direct child:

```rust
let parent = database.list_sessions(10).unwrap().remove(0);
assert_eq!(parent.child_session_count, 1);
```

- [ ] **Step 2: Run the focused test and verify it fails to compile**

Run:

```bash
cargo test --test index_tests reindex_retains_subagent_threads_outside_top_level_sessions
```

Expected: compilation fails because `child_session_count` is absent.

- [ ] **Step 3: Derive direct child counts in summary queries**

Add a correlated count projection to every `SessionSummary` query:

```sql
(
    SELECT COUNT(*)
    FROM sessions child
    WHERE child.parent_thread_id = sessions.session_id
) AS child_session_count
```

Use the appropriate outer-table alias in joined search queries. Load the count as `usize` in `summary_from_row`.

- [ ] **Step 4: Run all database and index tests**

Run:

```bash
cargo test --test index_tests
cargo test --test tui_tests
```

Expected: both test binaries pass.

### Task 5: Show Agent Counts Without Changing Layout

**Files:**
- Modify: `src/tui/format.rs`
- Modify: `tests/tui_tests.rs`

- [ ] **Step 1: Add a failing formatting test**

Construct a summary with `child_session_count: 2` and assert its expanded metadata contains `2 agents`. Also assert a summary with no children has no agent label.

- [ ] **Step 2: Run the focused TUI tests and verify the failure**

Run:

```bash
cargo test --test tui_tests session_summary
```

Expected: the positive formatting assertion fails because child counts are not rendered.

- [ ] **Step 3: Append the compact agent indicator**

Extend `session_summary_metadata` to append `↳ N agent` or `↳ N agents` only when the count is nonzero.

- [ ] **Step 4: Run all TUI tests**

Run:

```bash
cargo test --test tui_tests
```

Expected: all TUI tests pass.

### Task 6: Verify the Milestone

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document nested agent visibility**

Update the session browser description to state that subagent work is indexed under its parent and represented by an agent count while top-level results remain focused on primary sessions.

- [ ] **Step 2: Run formatting and the complete test suite**

Run:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo package --allow-dirty --no-verify --offline
```

Expected: every command exits successfully with no test failures or Clippy warnings.

- [ ] **Step 3: Review the final diff**

Run:

```bash
git diff --check
git status --short
git diff --stat
```

Expected: no whitespace errors; only the planned documentation, parser/index relationship, storage query, fixture-test, and TUI-formatting files are changed.

# Codex Explorer Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the repository-facing product identity to Codex Explorer while keeping the `cx` command.

**Architecture:** Product text is updated in the CLI help and README. The repository directory is moved from `/home/chief/computer-use` to `/home/chief/codex-explorer`; Rust package, binary, tests, and DB path remain `cx`.

**Tech Stack:** Rust 2021, `clap`, existing CLI tests, shell `mv`.

---

## Tasks

### Task 1: Update Product-Facing Text

**Files:**
- Modify: `tests/cli_tests.rs`
- Modify: `src/cli.rs`
- Modify: `README.md`

- [ ] Write a failing CLI test that expects `Codex Explorer` in `cx --help`.
- [ ] Run `cargo test --test cli_tests help_mentions_codex_explorer`.
- [ ] Update the Clap `about` text to include `Codex Explorer`.
- [ ] Update the README title and opening sentence.
- [ ] Run the focused CLI test and verify it passes.

### Task 2: Rename the Repository Folder

**Filesystem:**
- Move: `/home/chief/computer-use` to `/home/chief/codex-explorer`

- [ ] Confirm `/home/chief/codex-explorer` does not already exist.
- [ ] Move the directory.
- [ ] Continue verification from `/home/chief/codex-explorer`.

### Task 3: Verify and Commit

**Commands:**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
git status --short
git commit -m "chore: rename project to Codex Explorer"
```

Expected: formatting, clippy, tests, and build all exit 0 before commit.

## Self-Review

- Scope matches the approved design.
- No binary/package/database rename is included.
- Historical docs are not rewritten except for the new rename decision.

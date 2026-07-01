# Codex Session Manager Design

## Purpose

Build `cx`, a local session manager for Codex that makes it easy to find, inspect, and resume past sessions without relying on date/time memory. The first version should be a reliable terminal tool over Codex's existing JSONL session files, with a TUI added after the CLI and index are working.

## Current Problem

Codex stores sessions under `~/.codex/sessions/YYYY/MM/DD/*.jsonl`. `codex resume` can resume by session id or session name, but the built-in picker is poor when the user remembers the project, task, topic, prompt, or files involved rather than the exact date and time.

## Goals

- Index local Codex session JSONL files.
- Search sessions by user prompts, assistant replies, working directory, session id, date, file paths, commands, and extracted summaries.
- Preview a session before resuming it.
- Resume a selected session by handing off to `codex resume <session_id>`.
- Provide a scriptable CLI first.
- Add a terminal UI once the storage and search model is stable.
- Keep the architecture simple enough for a Rust learner to understand and maintain.

## Non-Goals

- Replace Codex itself.
- Modify Codex's session files directly.
- Depend on a background daemon in the first version.
- Build a browser dashboard in the first version.
- Add semantic or embedding search before normal full-text search is proven insufficient.

## User Workflow

Initial CLI workflow:

```bash
cx reindex
cx list
cx search "turnstile worker"
cx show <session-id>
cx resume <session-id>
```

Later TUI workflow:

```bash
cx
```

The TUI opens a searchable list of sessions. Selecting a row shows a preview. Pressing `Enter` launches:

```bash
codex resume <session-id>
```

## Technology Choices

- Rust for the implementation language.
- `clap` for CLI parsing.
- `serde` and `serde_json` for JSONL parsing.
- `rusqlite` for SQLite access.
- SQLite FTS5 for full-text search.
- `walkdir` for scanning `~/.codex/sessions`.
- `time` for timestamp handling.
- `dirs` for resolving platform-appropriate data and home directories.
- `anyhow` for early-stage error handling.
- `ratatui` and `crossterm` for the later TUI.

Async Rust is intentionally avoided in the first version. The app is local, filesystem-bound, and easier to learn and test with synchronous code.

## Architecture

The project should be split into small modules:

- `main.rs`: application entrypoint.
- `cli.rs`: command definitions and CLI argument parsing.
- `codex.rs`: Codex JSONL parsing and session metadata extraction.
- `db.rs`: SQLite schema, migrations, inserts, and queries.
- `indexer.rs`: filesystem scanning and reindex logic.
- `search.rs`: search query handling and result ranking.
- `codex_cmd.rs`: handoff to `codex resume`, `codex fork`, `codex archive`, and related commands.
- `tui.rs`: terminal UI added after the CLI is useful.

The core data flow:

```text
~/.codex/sessions/**/*.jsonl
          |
       indexer
          |
~/.local/share/cx/index.sqlite
          |
   CLI commands + TUI
          |
 codex resume <session_id>
```

## Data Model

The SQLite database should store normalized metadata and searchable text.

Suggested tables:

- `sessions`: one row per Codex session file.
- `messages`: user, assistant, and relevant system/developer messages.
- `tool_events`: shell commands and tool calls extracted from response items.
- `session_fts`: FTS5 table containing searchable session text.
- `tags`: user-created labels for sessions.

The first implementation can start with `sessions`, `messages`, and `session_fts`. Tags and tool event extraction can follow once the basic index works.

## Indexing Behavior

`cx reindex` should:

- Scan `~/.codex/sessions`.
- Parse each `.jsonl` file.
- Extract session metadata from `session_meta`.
- Extract readable user and assistant messages from `response_item` records.
- Store the source file path and modified time.
- Skip unchanged files on later runs.
- Rebuild the FTS row for a session when the source file changes.

The indexer should tolerate malformed or partially written session files by reporting the problem and continuing with other files.

## Search Behavior

`cx search <query>` should return ranked session results with:

- session id
- timestamp
- working directory
- title or first user prompt
- short snippet
- source file path

Search should combine metadata filters and FTS. The first version does not need perfect ranking; useful results are more important than clever scoring.

## Resume Behavior

`cx resume <session-id>` should hand off to Codex using:

```bash
codex resume <session-id>
```

The tool should use process replacement where practical so the user's terminal becomes the Codex TUI rather than nesting long-running processes awkwardly. If process replacement is not portable enough, it can spawn Codex and forward exit status.

## TUI Design

The TUI should be added after the CLI is stable. It should provide:

- Search input.
- Session result list.
- Preview pane.
- Filter controls for project path and date.
- Keyboard actions:
  - `Enter`: resume selected session.
  - `/`: focus search.
  - `f`: fork selected session.
  - `a`: archive selected session.
  - `d`: delete selected session after confirmation.
  - `t`: tag selected session.
  - `q`: quit.

The TUI should read from the same database and commands as the CLI. It should not own separate parsing or search logic.

## Error Handling

- Missing `~/.codex/sessions`: show a clear message and exit successfully for list/search with no results.
- Missing `codex` binary when resuming: show the attempted command and explain that Codex must be installed and on `PATH`.
- Malformed JSONL record: skip the record, count it, and include a warning in reindex output.
- SQLite errors: fail with context about the database path and operation.
- Interrupted indexing: leave the previous usable index intact when possible.

## Testing

Use fixture JSONL files under `tests/fixtures`.

Test coverage should include:

- Parsing `session_meta`.
- Extracting user and assistant messages.
- Handling malformed JSONL records.
- Indexing fixture sessions into a temporary SQLite database.
- Searching by prompt text.
- Searching by working directory.
- Producing a resume command for a selected session.

The first implementation should make the parser and indexer testable without invoking the real Codex binary or reading the user's real `~/.codex` directory.

## Phasing

Phase 1: CLI and index.

- Create Rust project.
- Implement parser, database schema, reindex, list, search, show, and resume.
- Add fixture-based tests.

Phase 2: TUI.

- Add Ratatui session browser over the existing search/index APIs.
- Support search, preview, and resume.

Phase 3: Management features.

- Add tags.
- Add archive, unarchive, delete, and fork wrappers.
- Add richer tool event extraction.
- Add better result ranking and saved filters.

## Decisions

- Binary name: `cx`. It does not currently conflict with an existing command on this machine.
- Database path: default to `~/.local/share/cx/index.sqlite`, with `--db <path>` for tests and advanced usage.
- Session source path: default to `~/.codex/sessions`, with `--sessions-dir <path>` for tests and advanced usage.
- Timestamp handling: use the `time` crate.
- Management lifecycle: call Codex's own commands for archive, unarchive, delete, fork, and resume rather than modifying Codex session files directly.

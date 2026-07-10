# Modular Session Foundation Design

## Purpose

Evolve Codex Explorer into one small application with strong internal boundaries. The product remains Codex-only, independently installable, and backed by local SQLite. Codex-specific persistence details must not leak into search, application workflows, or the TUI.

## Principles

- Keep one binary, one repository, and one SQLite database.
- Treat Codex JSONL files as the source of truth and SQLite as a disposable index.
- Use the local Codex source as a behavioral reference, not a dependency.
- Introduce abstractions only at real replacement boundaries.
- Preserve current CLI and TUI behavior while migrating incrementally.
- Hide subagents only as a top-level presentation policy; never discard their activity during indexing.

## Target Architecture

```text
Codex session files
        |
        v
Codex source adapter
        |
        v
Canonical session model
        |
        v
SQLite repositories
        |
        v
Application operations
        |
   +----+----+
   |         |
  CLI       TUI
```

The Codex adapter owns discovery, JSONL decoding, Codex event interpretation, and conversion into canonical sessions. Storage owns migrations, indexing state, normalized projections, search, relationships, and cached audits. Application operations coordinate refresh, browse, search, inspect, audit, and resume. The CLI and TUI are presentation layers over those operations.

## Canonical Model

A session has a native identifier, source identity, working directory, timestamps, optional Git metadata, source file revision, ordered entries, and relationships to parent or child sessions. Entries represent messages, tool calls, tool results, status events, or unknown source events.

The first migration milestone does not replace every existing parsed type. It first establishes session relationships in storage and query behavior, then introduces the full canonical model behind tests in later slices.

## Subagent Presentation

Subagent sessions are indexed as complete sessions and linked to their parent through `parent_thread_id`. Default session lists exclude children so the left pane stays focused on user-created threads. Parent summaries expose an agent count. Parent inspection will later add attributed child activity to overview, tools, timeline, search, and audits.

Subagent metadata should eventually include nickname, role, path, spawn depth, and derived completion status. The first slice persists the relationship and retains the full child session; richer identity follows without another indexing-policy reversal.

## Compatibility and Failure Handling

- Malformed individual JSONL records are counted and skipped while valid records continue.
- A session without a usable native identifier remains a file-level indexing failure.
- Unknown record types remain represented as unknown timeline entries.
- A failed directory traversal must not prune previously indexed sessions.
- Reindexing a changed source replaces its derived rows transactionally.
- Existing databases migrate in place by adding nullable relationship columns.

## Testing

Characterization tests protect current list, search, show, resume, and TUI behavior. Fixture-driven parser and index tests cover parent sessions, child sessions, nesting, malformed records, and unknown records. Each migration slice follows a red-green-refactor cycle and ends with formatting, tests, Clippy, and package verification.

## First Implementation Milestone

1. Persist `parent_thread_id` and `thread_source` on indexed sessions.
2. Stop deleting or skipping subagents during indexing.
3. Keep default list and project-scoped list queries limited to root sessions.
4. Keep default search results limited to root sessions until parent-attributed child matches are designed.
5. Add child-count data to parent summaries.
6. Render the count as a compact agent indicator without changing the two-pane layout.

## Deferred

- Other coding-agent providers
- A generic plugin framework
- Codex app-server integration
- Semantic search
- Background daemons
- Web UI or cloud synchronization
- Editing native session files
- Full canonical tool-call/result migration
- Deep subagent navigation and parent-attributed search, which follow the relationship foundation

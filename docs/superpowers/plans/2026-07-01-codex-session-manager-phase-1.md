# Codex Session Manager Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working `cx` CLI that indexes Codex JSONL sessions, searches them, previews them, and resumes a selected session through `codex resume <session-id>`.

**Architecture:** The app is a synchronous Rust CLI with a small parser, a SQLite-backed index, and commands over that index. The Codex session files stay read-only; lifecycle actions hand off to the installed `codex` binary.

**Tech Stack:** Rust 2021, `clap`, `serde`, `serde_json`, `rusqlite` with bundled SQLite, SQLite FTS5, `walkdir`, `time`, `dirs`, `anyhow`, `assert_cmd`, `predicates`, `tempfile`.

---

## Scope

This plan implements Phase 1 from the design spec. It does not implement the Ratatui TUI, tags, archive/delete/fork wrappers, semantic search, or a background daemon. Those features should be planned after this CLI/index foundation is working.

## File Structure

- Create: `Cargo.toml`
  - Defines the `cx` binary and dependencies.
- Create: `src/main.rs`
  - Entrypoint, command dispatch, path resolution, and output formatting.
- Create: `src/cli.rs`
  - `clap` command definitions.
- Create: `src/codex.rs`
  - Parse Codex JSONL files into `ParsedSession` and `ParsedMessage`.
- Create: `src/db.rs`
  - SQLite schema, upsert logic, list/search/show queries.
- Create: `src/indexer.rs`
  - Scan session directory and index changed `.jsonl` files.
- Create: `src/codex_cmd.rs`
  - Build and run Codex lifecycle commands.
- Create: `tests/fixtures/session-a.jsonl`
  - Valid fixture with metadata and user/assistant messages.
- Create: `tests/fixtures/session-malformed.jsonl`
  - Fixture with one invalid JSONL record and useful surrounding records.
- Create: `tests/parser_tests.rs`
  - Parser unit/integration tests.
- Create: `tests/index_tests.rs`
  - Indexing and search tests with temporary SQLite databases.
- Create: `tests/cli_tests.rs`
  - End-to-end CLI tests using fixture directories.

## Implementation Tasks

### Task 1: Create the Rust Project Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/cli.rs`

- [ ] **Step 1: Create the manifest**

Create `Cargo.toml`:

```toml
[package]
name = "cx"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "cx"
path = "src/main.rs"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
dirs = "6"
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
time = { version = "0.3", features = ["formatting", "parsing", "macros"] }
walkdir = "2"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 2: Add the first CLI definition**

Create `src/cli.rs`:

```rust
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cx")]
#[command(about = "Find and resume Codex sessions")]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<PathBuf>,

    #[arg(long, global = true, value_name = "PATH")]
    pub sessions_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Reindex,
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Show {
        session_id: String,
    },
    Resume {
        session_id: String,
    },
}
```

- [ ] **Step 3: Add a compiling entrypoint**

Create `src/main.rs`:

```rust
mod cli;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Reindex => {
            println!("reindex command is wired");
        }
        Commands::List { limit } => {
            println!("list command is wired with limit {limit}");
        }
        Commands::Search { query, limit } => {
            println!("search command is wired for {query:?} with limit {limit}");
        }
        Commands::Show { session_id } => {
            println!("show command is wired for {session_id}");
        }
        Commands::Resume { session_id } => {
            println!("resume command is wired for {session_id}");
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Verify the skeleton**

Run:

```bash
cargo test
cargo run -- --help
```

Expected: tests compile successfully and help output lists `reindex`, `list`, `search`, `show`, and `resume`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src/cli.rs
git commit -m "feat: scaffold cx CLI"
```

### Task 2: Parse Codex JSONL Sessions

**Files:**
- Create: `src/codex.rs`
- Modify: `src/main.rs`
- Create: `tests/fixtures/session-a.jsonl`
- Create: `tests/fixtures/session-malformed.jsonl`
- Create: `tests/parser_tests.rs`

- [ ] **Step 1: Add parser fixtures**

Create `tests/fixtures/session-a.jsonl`:

```jsonl
{"timestamp":"2026-07-01T10:00:00Z","type":"session_meta","payload":{"session_id":"11111111-1111-4111-8111-111111111111","timestamp":"2026-07-01T10:00:00Z","cwd":"/work/project-a","cli_version":"0.142.5","model_provider":"openai","source":"cli","originator":"codex-tui"}}
{"timestamp":"2026-07-01T10:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"add turnstile to the signup form"}]}}
{"timestamp":"2026-07-01T10:00:03Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"I will inspect the Worker and form code."}]}}
```

Create `tests/fixtures/session-malformed.jsonl`:

```jsonl
{"timestamp":"2026-07-01T11:00:00Z","type":"session_meta","payload":{"session_id":"22222222-2222-4222-8222-222222222222","timestamp":"2026-07-01T11:00:00Z","cwd":"/work/project-b","cli_version":"0.142.5","model_provider":"openai","source":"cli","originator":"codex-tui"}}
this is not json
{"timestamp":"2026-07-01T11:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"debug websocket reconnect loop"}]}}
```

- [ ] **Step 2: Write parser tests**

Create `tests/parser_tests.rs`:

```rust
#[path = "../src/codex.rs"]
mod codex;

use std::path::Path;

#[test]
fn parses_session_metadata_and_messages() {
    let parsed = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();

    assert_eq!(parsed.session_id, "11111111-1111-4111-8111-111111111111");
    assert_eq!(parsed.cwd, "/work/project-a");
    assert_eq!(parsed.started_at, "2026-07-01T10:00:00Z");
    assert_eq!(parsed.title, "add turnstile to the signup form");
    assert_eq!(parsed.messages.len(), 2);
    assert_eq!(parsed.messages[0].role, "user");
    assert_eq!(parsed.messages[1].role, "assistant");
    assert_eq!(parsed.malformed_records, 0);
}

#[test]
fn skips_malformed_records_and_keeps_valid_messages() {
    let parsed = codex::parse_session_file(Path::new("tests/fixtures/session-malformed.jsonl")).unwrap();

    assert_eq!(parsed.session_id, "22222222-2222-4222-8222-222222222222");
    assert_eq!(parsed.title, "debug websocket reconnect loop");
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.malformed_records, 1);
}
```

- [ ] **Step 3: Run parser tests to verify they fail**

Run:

```bash
cargo test --test parser_tests
```

Expected: FAIL because `src/codex.rs` does not exist.

- [ ] **Step 4: Implement the parser**

Create `src/codex.rs`:

```rust
use std::{fs::File, io::{BufRead, BufReader}, path::{Path, PathBuf}};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSession {
    pub session_id: String,
    pub started_at: String,
    pub cwd: String,
    pub cli_version: Option<String>,
    pub model_provider: Option<String>,
    pub source_path: PathBuf,
    pub modified_unix_seconds: i64,
    pub title: String,
    pub messages: Vec<ParsedMessage>,
    pub searchable_text: String,
    pub malformed_records: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMessage {
    pub timestamp: String,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
struct JsonlRecord {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    record_type: String,
    payload: Value,
}

pub fn parse_session_file(path: &Path) -> Result<ParsedSession> {
    let file = File::open(path).with_context(|| format!("open session file {}", path.display()))?;
    let metadata = file.metadata().with_context(|| format!("read metadata {}", path.display()))?;
    let modified_unix_seconds = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);

    let reader = BufReader::new(file);
    let mut session_id = None;
    let mut started_at = None;
    let mut cwd = None;
    let mut cli_version = None;
    let mut model_provider = None;
    let mut messages = Vec::new();
    let mut malformed_records = 0usize;

    for line in reader.lines() {
        let line = line.with_context(|| format!("read line from {}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }

        let record = match serde_json::from_str::<JsonlRecord>(&line) {
            Ok(record) => record,
            Err(_) => {
                malformed_records += 1;
                continue;
            }
        };

        if record.record_type == "session_meta" {
            session_id = string_field(&record.payload, "session_id").or_else(|| string_field(&record.payload, "id"));
            started_at = string_field(&record.payload, "timestamp").or(record.timestamp.clone());
            cwd = string_field(&record.payload, "cwd");
            cli_version = string_field(&record.payload, "cli_version");
            model_provider = string_field(&record.payload, "model_provider");
            continue;
        }

        if record.record_type == "response_item" {
            if let Some(message) = parse_message(record.timestamp.as_deref().unwrap_or_default(), &record.payload) {
                messages.push(message);
            }
        }
    }

    let session_id = session_id.with_context(|| format!("missing session id in {}", path.display()))?;
    let started_at = started_at.unwrap_or_default();
    let cwd = cwd.unwrap_or_default();
    let title = messages
        .iter()
        .find(|message| message.role == "user" && !message.text.trim().is_empty())
        .map(|message| first_line(&message.text))
        .unwrap_or_else(|| session_id.clone());
    let searchable_text = messages
        .iter()
        .map(|message| message.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ParsedSession {
        session_id,
        started_at,
        cwd,
        cli_version,
        model_provider,
        source_path: path.to_path_buf(),
        modified_unix_seconds,
        title,
        messages,
        searchable_text,
        malformed_records,
    })
}

fn parse_message(timestamp: &str, payload: &Value) -> Option<ParsedMessage> {
    if string_field(payload, "type").as_deref() != Some("message") {
        return None;
    }

    let role = string_field(payload, "role")?;
    if !matches!(role.as_str(), "user" | "assistant") {
        return None;
    }

    let text = payload
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| string_field(item, "text"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    if text.trim().is_empty() {
        return None;
    }

    Some(ParsedMessage {
        timestamp: timestamp.to_string(),
        role,
        text,
    })
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).trim().to_string()
}
```

- [ ] **Step 5: Wire the module**

Modify `src/main.rs` so the module list begins:

```rust
mod cli;
mod codex;
```

- [ ] **Step 6: Verify parser tests**

Run:

```bash
cargo test --test parser_tests
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/codex.rs tests/fixtures/session-a.jsonl tests/fixtures/session-malformed.jsonl tests/parser_tests.rs
git commit -m "feat: parse Codex session JSONL"
```

### Task 3: Add SQLite Schema and Repository Queries

**Files:**
- Create: `src/db.rs`
- Modify: `src/main.rs`
- Create: `tests/index_tests.rs`

- [ ] **Step 1: Write database tests**

Create `tests/index_tests.rs`:

```rust
#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/db.rs"]
mod db;

use std::path::Path;

#[test]
fn stores_lists_searches_and_shows_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let parsed = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();

    database.upsert_session(&parsed).unwrap();

    let listed = database.list_sessions(10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, parsed.session_id);
    assert_eq!(listed[0].title, "add turnstile to the signup form");

    let results = database.search_sessions("turnstile", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, parsed.session_id);

    let shown = database.get_session(&parsed.session_id).unwrap().unwrap();
    assert_eq!(shown.messages.len(), 2);
    assert_eq!(shown.messages[0].text, "add turnstile to the signup form");
}
```

- [ ] **Step 2: Run database tests to verify they fail**

Run:

```bash
cargo test --test index_tests
```

Expected: FAIL because `src/db.rs` does not exist.

- [ ] **Step 3: Implement database access**

Create `src/db.rs`:

```rust
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::codex::{ParsedMessage, ParsedSession};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub session_id: String,
    pub started_at: String,
    pub cwd: String,
    pub title: String,
    pub source_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDetail {
    pub summary: SessionSummary,
    pub messages: Vec<ParsedMessage>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create database directory {}", parent.display()))?;
        }

        let conn = Connection::open(path).with_context(|| format!("open database {}", path.display()))?;
        let database = Self { conn };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                started_at TEXT NOT NULL,
                cwd TEXT NOT NULL,
                cli_version TEXT,
                model_provider TEXT,
                source_path TEXT NOT NULL UNIQUE,
                modified_unix_seconds INTEGER NOT NULL,
                title TEXT NOT NULL,
                searchable_text TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                session_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                PRIMARY KEY (session_id, position),
                FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
                session_id UNINDEXED,
                title,
                cwd,
                searchable_text
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_session(&self, session: &ParsedSession) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM messages WHERE session_id = ?1", params![session.session_id])?;
        tx.execute("DELETE FROM session_fts WHERE session_id = ?1", params![session.session_id])?;
        tx.execute(
            r#"
            INSERT INTO sessions (
                session_id, started_at, cwd, cli_version, model_provider,
                source_path, modified_unix_seconds, title, searchable_text
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(session_id) DO UPDATE SET
                started_at = excluded.started_at,
                cwd = excluded.cwd,
                cli_version = excluded.cli_version,
                model_provider = excluded.model_provider,
                source_path = excluded.source_path,
                modified_unix_seconds = excluded.modified_unix_seconds,
                title = excluded.title,
                searchable_text = excluded.searchable_text
            "#,
            params![
                session.session_id,
                session.started_at,
                session.cwd,
                session.cli_version,
                session.model_provider,
                session.source_path.display().to_string(),
                session.modified_unix_seconds,
                session.title,
                session.searchable_text,
            ],
        )?;

        for (position, message) in session.messages.iter().enumerate() {
            tx.execute(
                r#"
                INSERT INTO messages (session_id, position, timestamp, role, text)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![session.session_id, position as i64, message.timestamp, message.role, message.text],
            )?;
        }

        tx.execute(
            r#"
            INSERT INTO session_fts (session_id, title, cwd, searchable_text)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![session.session_id, session.title, session.cwd, session.searchable_text],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>> {
        let mut statement = self.conn.prepare(
            r#"
            SELECT session_id, started_at, cwd, title, source_path
            FROM sessions
            ORDER BY started_at DESC
            LIMIT ?1
            "#,
        )?;
        rows_to_summaries(statement.query_map(params![limit as i64], summary_from_row)?)
    }

    pub fn search_sessions(&self, query: &str, limit: usize) -> Result<Vec<SessionSummary>> {
        let mut statement = self.conn.prepare(
            r#"
            SELECT s.session_id, s.started_at, s.cwd, s.title, s.source_path
            FROM session_fts f
            JOIN sessions s ON s.session_id = f.session_id
            WHERE session_fts MATCH ?1
            ORDER BY rank
            LIMIT ?2
            "#,
        )?;
        rows_to_summaries(statement.query_map(params![query, limit as i64], summary_from_row)?)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let summary = {
            let mut statement = self.conn.prepare(
                r#"
                SELECT session_id, started_at, cwd, title, source_path
                FROM sessions
                WHERE session_id = ?1
                "#,
            )?;
            let mut rows = statement.query(params![session_id])?;
            match rows.next()? {
                Some(row) => Some(summary_from_row(row)?),
                None => None,
            }
        };

        let Some(summary) = summary else {
            return Ok(None);
        };

        let mut statement = self.conn.prepare(
            r#"
            SELECT timestamp, role, text
            FROM messages
            WHERE session_id = ?1
            ORDER BY position ASC
            "#,
        )?;
        let messages = statement
            .query_map(params![session_id], |row| {
                Ok(ParsedMessage {
                    timestamp: row.get(0)?,
                    role: row.get(1)?,
                    text: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(Some(SessionDetail { summary, messages }))
    }
}

fn rows_to_summaries(
    rows: rusqlite::MappedRows<'_, fn(&rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary>>,
) -> Result<Vec<SessionSummary>> {
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        session_id: row.get(0)?,
        started_at: row.get(1)?,
        cwd: row.get(2)?,
        title: row.get(3)?,
        source_path: row.get(4)?,
    })
}
```

- [ ] **Step 4: Wire the module**

Modify `src/main.rs` so the module list begins:

```rust
mod cli;
mod codex;
mod db;
```

- [ ] **Step 5: Verify database tests**

Run:

```bash
cargo test --test index_tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/db.rs tests/index_tests.rs
git commit -m "feat: store and search indexed sessions"
```

### Task 4: Implement Reindexing

**Files:**
- Create: `src/indexer.rs`
- Modify: `src/main.rs`
- Modify: `tests/index_tests.rs`

- [ ] **Step 1: Add an indexer test**

Append this test to `tests/index_tests.rs`:

```rust
#[path = "../src/indexer.rs"]
mod indexer;

#[test]
fn reindexes_all_jsonl_files_in_a_sessions_directory() {
    let temp = tempfile::tempdir().unwrap();
    let sessions_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::copy("tests/fixtures/session-a.jsonl", sessions_dir.join("session-a.jsonl")).unwrap();
    std::fs::copy("tests/fixtures/session-malformed.jsonl", sessions_dir.join("session-malformed.jsonl")).unwrap();

    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let report = indexer::reindex(&database, &sessions_dir).unwrap();

    assert_eq!(report.scanned_files, 2);
    assert_eq!(report.indexed_sessions, 2);
    assert_eq!(report.malformed_records, 1);
    assert_eq!(database.list_sessions(10).unwrap().len(), 2);
}
```

- [ ] **Step 2: Run indexer test to verify it fails**

Run:

```bash
cargo test --test index_tests reindexes_all_jsonl_files_in_a_sessions_directory
```

Expected: FAIL because `src/indexer.rs` does not exist.

- [ ] **Step 3: Implement the indexer**

Create `src/indexer.rs`:

```rust
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::{codex, db::Database};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReindexReport {
    pub scanned_files: usize,
    pub indexed_sessions: usize,
    pub malformed_records: usize,
    pub failed_files: Vec<String>,
}

pub fn reindex(database: &Database, sessions_dir: &Path) -> Result<ReindexReport> {
    let mut report = ReindexReport::default();

    if !sessions_dir.exists() {
        return Ok(report);
    }

    for entry in WalkDir::new(sessions_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }

        report.scanned_files += 1;

        match codex::parse_session_file(path) {
            Ok(session) => {
                report.malformed_records += session.malformed_records;
                database.upsert_session(&session)?;
                report.indexed_sessions += 1;
            }
            Err(error) => {
                report.failed_files.push(format!("{}: {error:#}", path.display()));
            }
        }
    }

    Ok(report)
}
```

- [ ] **Step 4: Wire the module**

Modify `src/main.rs` so the module list begins:

```rust
mod cli;
mod codex;
mod db;
mod indexer;
```

- [ ] **Step 5: Verify indexer test**

Run:

```bash
cargo test --test index_tests reindexes_all_jsonl_files_in_a_sessions_directory
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/indexer.rs tests/index_tests.rs
git commit -m "feat: reindex Codex session directory"
```

### Task 5: Implement CLI Commands Over the Index

**Files:**
- Modify: `src/main.rs`
- Create: `tests/cli_tests.rs`

- [ ] **Step 1: Write CLI behavior tests**

Create `tests/cli_tests.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn fixture_sessions_dir() -> &'static str {
    "tests/fixtures"
}

#[test]
fn reindex_and_search_from_cli() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");

    Command::cargo_bin("cx")
        .unwrap()
        .args(["--db", db_path.to_str().unwrap(), "--sessions-dir", fixture_sessions_dir(), "reindex"])
        .assert()
        .success()
        .stdout(predicate::str::contains("indexed 2 sessions"));

    Command::cargo_bin("cx")
        .unwrap()
        .args(["--db", db_path.to_str().unwrap(), "search", "turnstile"])
        .assert()
        .success()
        .stdout(predicate::str::contains("11111111-1111-4111-8111-111111111111"))
        .stdout(predicate::str::contains("add turnstile to the signup form"));
}

#[test]
fn show_displays_session_preview() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");

    Command::cargo_bin("cx")
        .unwrap()
        .args(["--db", db_path.to_str().unwrap(), "--sessions-dir", fixture_sessions_dir(), "reindex"])
        .assert()
        .success();

    Command::cargo_bin("cx")
        .unwrap()
        .args(["--db", db_path.to_str().unwrap(), "show", "11111111-1111-4111-8111-111111111111"])
        .assert()
        .success()
        .stdout(predicate::str::contains("/work/project-a"))
        .stdout(predicate::str::contains("user: add turnstile to the signup form"))
        .stdout(predicate::str::contains("assistant: I will inspect the Worker and form code."));
}
```

- [ ] **Step 2: Run CLI tests to verify they fail**

Run:

```bash
cargo test --test cli_tests
```

Expected: FAIL because `src/main.rs` still prints the initial scaffold output instead of reading from SQLite.

- [ ] **Step 3: Implement path resolution and command dispatch**

Replace `src/main.rs` with:

```rust
mod cli;
mod codex;
mod db;
mod indexer;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::db::{Database, SessionSummary};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = resolve_db_path(cli.db)?;
    let database = Database::open(&db_path)?;

    match cli.command {
        Commands::Reindex => {
            let sessions_dir = resolve_sessions_dir(cli.sessions_dir)?;
            let report = indexer::reindex(&database, &sessions_dir)?;
            println!(
                "scanned {} files, indexed {} sessions, skipped {} malformed records",
                report.scanned_files, report.indexed_sessions, report.malformed_records
            );
            if !report.failed_files.is_empty() {
                eprintln!("failed files:");
                for failure in report.failed_files {
                    eprintln!("  {failure}");
                }
            }
        }
        Commands::List { limit } => {
            print_summaries(database.list_sessions(limit)?);
        }
        Commands::Search { query, limit } => {
            print_summaries(database.search_sessions(&query, limit)?);
        }
        Commands::Show { session_id } => {
            match database.get_session(&session_id)? {
                Some(detail) => {
                    println!("{}  {}", detail.summary.session_id, detail.summary.title);
                    println!("cwd: {}", detail.summary.cwd);
                    println!("started: {}", detail.summary.started_at);
                    println!("source: {}", detail.summary.source_path);
                    println!();
                    for message in detail.messages {
                        println!("{}: {}", message.role, message.text.replace('\n', " "));
                    }
                }
                None => {
                    anyhow::bail!("session not found: {session_id}");
                }
            }
        }
        Commands::Resume { session_id } => {
            println!("codex resume {session_id}");
        }
    }

    Ok(())
}

fn resolve_db_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    let data_dir = dirs::data_dir().context("could not resolve local data directory")?;
    Ok(data_dir.join("cx").join("index.sqlite"))
}

fn resolve_sessions_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    let home = dirs::home_dir().context("could not resolve home directory")?;
    Ok(home.join(".codex").join("sessions"))
}

fn print_summaries(summaries: Vec<SessionSummary>) {
    for summary in summaries {
        println!(
            "{}  {}  {}  {}",
            summary.started_at, summary.session_id, summary.cwd, summary.title
        );
    }
}
```

- [ ] **Step 4: Verify CLI tests**

Run:

```bash
cargo test --test cli_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs tests/cli_tests.rs
git commit -m "feat: add index-backed CLI commands"
```

### Task 6: Add Codex Resume Handoff

**Files:**
- Create: `src/codex_cmd.rs`
- Modify: `src/main.rs`
- Modify: `tests/cli_tests.rs`

- [ ] **Step 1: Add command construction test**

Append this test to `tests/cli_tests.rs`:

```rust
#[path = "../src/codex_cmd.rs"]
mod codex_cmd;

#[test]
fn builds_codex_resume_command() {
    let command = codex_cmd::resume_command("11111111-1111-4111-8111-111111111111");

    assert_eq!(command.program, "codex");
    assert_eq!(command.args, vec!["resume", "11111111-1111-4111-8111-111111111111"]);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --test cli_tests builds_codex_resume_command
```

Expected: FAIL because `src/codex_cmd.rs` does not exist.

- [ ] **Step 3: Implement Codex command handoff**

Create `src/codex_cmd.rs`:

```rust
use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn resume_command(session_id: &str) -> ExternalCommand {
    ExternalCommand {
        program: "codex".to_string(),
        args: vec!["resume".to_string(), session_id.to_string()],
    }
}

pub fn run(command: ExternalCommand) -> Result<i32> {
    let status = std::process::Command::new(&command.program)
        .args(&command.args)
        .status()
        .with_context(|| format!("run {} {}", command.program, command.args.join(" ")))?;

    Ok(status.code().unwrap_or(1))
}
```

- [ ] **Step 4: Wire resume command**

Modify `src/main.rs` so the module list includes:

```rust
mod codex_cmd;
```

Replace the `Commands::Resume` arm with:

```rust
        Commands::Resume { session_id } => {
            let command = codex_cmd::resume_command(&session_id);
            let code = codex_cmd::run(command)?;
            std::process::exit(code);
        }
```

- [ ] **Step 5: Verify resume command tests**

Run:

```bash
cargo test --test cli_tests builds_codex_resume_command
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/codex_cmd.rs tests/cli_tests.rs
git commit -m "feat: hand off resume to Codex"
```

### Task 7: Add User-Facing Polish and Documentation

**Files:**
- Create: `README.md`
- Modify: `src/main.rs`

- [ ] **Step 1: Add README**

Create `README.md`:

```markdown
# cx

`cx` is a local Codex session manager. It indexes `~/.codex/sessions/**/*.jsonl` so you can search, preview, and resume old Codex sessions without remembering the exact date.

## Commands

```bash
cx reindex
cx list
cx search "turnstile worker"
cx show <session-id>
cx resume <session-id>
```

## Development

Run tests:

```bash
cargo test
```

Run against fixture data:

```bash
cargo run -- --db /tmp/cx.sqlite --sessions-dir tests/fixtures reindex
cargo run -- --db /tmp/cx.sqlite search turnstile
```
```

- [ ] **Step 2: Improve missing result output**

In `src/main.rs`, update `print_summaries`:

```rust
fn print_summaries(summaries: Vec<SessionSummary>) {
    if summaries.is_empty() {
        println!("no sessions found");
        return;
    }

    for summary in summaries {
        println!(
            "{}  {}  {}  {}",
            summary.started_at, summary.session_id, summary.cwd, summary.title
        );
    }
}
```

- [ ] **Step 3: Verify all tests and formatting**

Run:

```bash
cargo fmt --check
cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add README.md src/main.rs
git commit -m "docs: document cx CLI usage"
```

### Task 8: Manual Verification Against Real Codex Sessions

**Files:**
- No source files should be changed unless verification exposes a defect.

- [ ] **Step 1: Build the binary**

Run:

```bash
cargo build
```

Expected: PASS.

- [ ] **Step 2: Index real sessions into a disposable database**

Run:

```bash
cargo run -- --db /tmp/cx-real.sqlite reindex
```

Expected: command succeeds and prints a count of scanned files and indexed sessions.

- [ ] **Step 3: Search for known text from this project**

Run:

```bash
cargo run -- --db /tmp/cx-real.sqlite search "session manager"
```

Expected: output includes at least one recent session from `/home/chief/computer-use`.

- [ ] **Step 4: Show one returned session**

Run:

```bash
cargo run -- --db /tmp/cx-real.sqlite show <session-id-from-search-output>
```

Expected: output includes cwd, started time, source path, and readable user/assistant messages.

- [ ] **Step 5: Do not run resume during automated verification**

Reason: `cx resume <session-id>` intentionally launches the interactive Codex TUI. Verify command construction through tests and leave interactive resume for a deliberate manual check.

- [ ] **Step 6: Commit any verification fixes**

If source changes were needed:

```bash
git add <changed-files>
git commit -m "fix: address real session indexing issue"
```

If no source changes were needed, do not create a commit.

## Self-Review

- Spec coverage: Phase 1 covers indexing local JSONL files, searching by prompt/message/cwd through FTS, previewing with `show`, and handing resume to Codex. TUI, tags, archive/delete/fork wrappers, and richer tool extraction are explicitly outside this Phase 1 plan.
- Quality scan: No unspecified implementation steps remain.
- Type consistency: `ParsedSession`, `ParsedMessage`, `Database`, `SessionSummary`, and `ExternalCommand` are defined before subsequent tasks use them.

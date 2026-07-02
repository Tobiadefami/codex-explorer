use std::{collections::HashSet, path::Path};

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

        let conn =
            Connection::open(path).with_context(|| format!("open database {}", path.display()))?;
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
        let source_path = session.source_path.display().to_string();
        let tx = self.conn.unchecked_transaction()?;
        let existing_session_ids = {
            let mut statement = tx.prepare(
                r#"
                SELECT session_id
                FROM sessions
                WHERE session_id = ?1 OR source_path = ?2
                "#,
            )?;
            let session_ids = statement
                .query_map(params![session.session_id, source_path], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()?;
            session_ids
        };

        for existing_session_id in &existing_session_ids {
            tx.execute(
                "DELETE FROM messages WHERE session_id = ?1",
                params![existing_session_id],
            )?;
            tx.execute(
                "DELETE FROM session_fts WHERE session_id = ?1",
                params![existing_session_id],
            )?;
        }

        tx.execute(
            "DELETE FROM sessions WHERE source_path = ?1 AND session_id <> ?2",
            params![source_path, session.session_id],
        )?;
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
                source_path,
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
                params![
                    session.session_id,
                    position as i64,
                    message.timestamp,
                    message.role,
                    message.text
                ],
            )?;
        }

        tx.execute(
            r#"
            INSERT INTO session_fts (session_id, title, cwd, searchable_text)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                session.session_id,
                session.title,
                session.cwd,
                session.searchable_text
            ],
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
        let rows = statement.query_map(params![limit as i64], summary_from_row)?;
        rows_to_summaries(rows)
    }

    pub fn search_sessions(&self, query: &str, limit: usize) -> Result<Vec<SessionSummary>> {
        let Some(fts_query) = plain_text_fts_query(query) else {
            return Ok(Vec::new());
        };

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
        let rows = statement.query_map(params![fts_query, limit as i64], summary_from_row)?;
        rows_to_summaries(rows)
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

    pub fn delete_source_path(&self, source_path: &Path) -> Result<usize> {
        let source_path = source_path.display().to_string();
        let session_ids = {
            let mut statement = self
                .conn
                .prepare("SELECT session_id FROM sessions WHERE source_path = ?1")?;
            let session_ids = statement
                .query_map(params![source_path], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()?;
            session_ids
        };

        if session_ids.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.unchecked_transaction()?;
        for session_id in &session_ids {
            tx.execute(
                "DELETE FROM session_fts WHERE session_id = ?1",
                params![session_id],
            )?;
            tx.execute(
                "DELETE FROM sessions WHERE session_id = ?1",
                params![session_id],
            )?;
        }
        tx.commit()?;

        Ok(session_ids.len())
    }

    pub fn prune_missing_source_paths(
        &self,
        source_root: &Path,
        seen_source_paths: &HashSet<String>,
    ) -> Result<usize> {
        let mut statement = self
            .conn
            .prepare("SELECT session_id, source_path FROM sessions")?;
        let stored_sessions = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let session_ids_to_prune = stored_sessions
            .into_iter()
            .filter(|(_, source_path)| {
                Path::new(source_path).starts_with(source_root)
                    && !seen_source_paths.contains(source_path)
            })
            .map(|(session_id, _)| session_id)
            .collect::<Vec<_>>();

        if session_ids_to_prune.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.unchecked_transaction()?;
        for session_id in &session_ids_to_prune {
            tx.execute(
                "DELETE FROM session_fts WHERE session_id = ?1",
                params![session_id],
            )?;
            tx.execute(
                "DELETE FROM sessions WHERE session_id = ?1",
                params![session_id],
            )?;
        }
        tx.commit()?;

        Ok(session_ids_to_prune.len())
    }
}

fn rows_to_summaries<I>(rows: I) -> Result<Vec<SessionSummary>>
where
    I: Iterator<Item = rusqlite::Result<SessionSummary>>,
{
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn plain_text_fts_query(query: &str) -> Option<String> {
    let terms = query
        .split_whitespace()
        .map(|term| term.replace('"', "\"\""))
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
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

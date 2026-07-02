use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

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
    pub parent_thread_id: Option<String>,
    pub thread_source: Option<String>,
    pub source_path: PathBuf,
    pub modified_unix_seconds: i64,
    pub title: String,
    pub messages: Vec<ParsedMessage>,
    pub searchable_text: String,
    pub malformed_records: usize,
}

impl ParsedSession {
    pub fn is_subagent_thread(&self) -> bool {
        self.thread_source.as_deref() == Some("subagent")
    }
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
    let metadata = file
        .metadata()
        .with_context(|| format!("read metadata {}", path.display()))?;
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
    let mut parent_thread_id = None;
    let mut thread_source = None;
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
            session_id = string_field(&record.payload, "id")
                .or_else(|| string_field(&record.payload, "session_id"));
            started_at = string_field(&record.payload, "timestamp").or(record.timestamp.clone());
            cwd = string_field(&record.payload, "cwd");
            cli_version = string_field(&record.payload, "cli_version");
            model_provider = string_field(&record.payload, "model_provider");
            parent_thread_id = string_field(&record.payload, "parent_thread_id");
            thread_source = string_field(&record.payload, "thread_source");
            continue;
        }

        if record.record_type == "response_item" {
            if let Some(message) = parse_message(
                record.timestamp.as_deref().unwrap_or_default(),
                &record.payload,
            ) {
                messages.push(message);
            }
        }
    }

    let session_id =
        session_id.with_context(|| format!("missing session id in {}", path.display()))?;
    let started_at = started_at.unwrap_or_default();
    let cwd = cwd.unwrap_or_default();
    let title = messages
        .iter()
        .filter_map(title_from_message)
        .next()
        .unwrap_or_else(|| session_id.clone());
    let searchable_text = messages
        .iter()
        .filter_map(searchable_text_from_message)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ParsedSession {
        session_id,
        started_at,
        cwd,
        cli_version,
        model_provider,
        parent_thread_id,
        thread_source,
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
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn title_from_message(message: &ParsedMessage) -> Option<String> {
    if message.role != "user" {
        return None;
    }

    let text = message.text.trim_start();
    if text.is_empty() || is_bootstrap_message(text) {
        return None;
    }

    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !is_noise_line(line))
        .map(ToOwned::to_owned)
}

fn searchable_text_from_message(message: &ParsedMessage) -> Option<String> {
    let text = message.text.trim_start();
    if text.is_empty() || is_bootstrap_message(text) {
        return None;
    }

    let searchable_text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_noise_line(line))
        .collect::<Vec<_>>()
        .join("\n");

    if searchable_text.is_empty() {
        None
    } else {
        Some(searchable_text)
    }
}

fn is_bootstrap_message(text: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "<environment_context",
        "<permissions instructions>",
        "<apps_instructions>",
        "<skills_instructions>",
        "<plugins_instructions>",
        "<collaboration_mode>",
        "# AGENTS.md instructions",
        "# CLAUDE.md instructions",
        "# GEMINI.md instructions",
    ];

    PREFIXES.iter().any(|prefix| text.starts_with(prefix))
}

fn is_noise_line(line: &str) -> bool {
    line.starts_with("<image ")
}

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSession {
    pub session_id: String,
    pub started_at: String,
    pub cwd: String,
    pub cli_version: Option<String>,
    pub model_provider: Option<String>,
    pub git_branch: Option<String>,
    pub parent_thread_id: Option<String>,
    pub thread_source: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub agent_path: Option<String>,
    pub source_path: PathBuf,
    pub modified_unix_seconds: i64,
    pub title: String,
    pub last_activity_at: String,
    pub latest_user_message: Option<String>,
    pub latest_assistant_message: Option<String>,
    pub items: Vec<ParsedSessionItem>,
    pub messages: Vec<ParsedMessage>,
    pub tool_events: Vec<ParsedToolEvent>,
    pub searchable_text: String,
    pub malformed_records: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedMessage {
    pub timestamp: String,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedSessionItem {
    pub timestamp: String,
    pub kind: ParsedSessionItemKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParsedSessionItemKind {
    SessionMeta(ParsedSessionMeta),
    Message(ParsedMessage),
    ToolEvent(ParsedToolEvent),
    Unknown {
        record_type: String,
        payload_type: Option<String>,
        payload: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedSessionMeta {
    pub session_id: Option<String>,
    pub started_at: Option<String>,
    pub cwd: Option<String>,
    pub cli_version: Option<String>,
    pub model_provider: Option<String>,
    pub git_branch: Option<String>,
    pub parent_thread_id: Option<String>,
    pub thread_source: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub agent_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedToolEvent {
    pub timestamp: String,
    pub kind: String,
    pub name: String,
    pub summary: String,
    pub status: Option<String>,
    pub call_id: Option<String>,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<i64>,
    pub cwd: Option<String>,
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
    let mut items = Vec::new();
    let mut malformed_records = 0usize;

    for line in reader.lines() {
        let line = line.with_context(|| format!("read line from {}", path.display()))?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<JsonlRecord>(&line) {
            Ok(record) => items.push(parse_session_item(record)),
            Err(_) => malformed_records += 1,
        }
    }

    derive_session_from_items(path, modified_unix_seconds, items, malformed_records)
}

fn derive_session_from_items(
    path: &Path,
    modified_unix_seconds: i64,
    items: Vec<ParsedSessionItem>,
    malformed_records: usize,
) -> Result<ParsedSession> {
    let mut session_id = None;
    let mut started_at = None;
    let mut cwd = None;
    let mut cli_version = None;
    let mut model_provider = None;
    let mut git_branch = None;
    let mut parent_thread_id = None;
    let mut thread_source = None;
    let mut agent_nickname = None;
    let mut agent_role = None;
    let mut agent_path = None;
    let mut messages = Vec::new();
    let mut tool_events = Vec::new();
    let mut last_activity_at = String::new();
    let mut latest_user_message = None;
    let mut latest_assistant_message = None;

    for item in &items {
        match &item.kind {
            ParsedSessionItemKind::SessionMeta(meta) => {
                session_id = meta.session_id.clone();
                started_at = meta.started_at.clone();
                cwd = meta.cwd.clone();
                cli_version = meta.cli_version.clone();
                model_provider = meta.model_provider.clone();
                git_branch = meta.git_branch.clone();
                parent_thread_id = meta.parent_thread_id.clone();
                thread_source = meta.thread_source.clone();
                agent_nickname = meta.agent_nickname.clone();
                agent_role = meta.agent_role.clone();
                agent_path = meta.agent_path.clone();
            }
            ParsedSessionItemKind::ToolEvent(tool_event) => {
                update_last_activity(&mut last_activity_at, &tool_event.timestamp);
                tool_events.push(tool_event.clone());
            }
            ParsedSessionItemKind::Message(message) => {
                update_last_activity(&mut last_activity_at, &message.timestamp);
                if is_meaningful_text(&message.text) {
                    match message.role.as_str() {
                        "user" => latest_user_message = Some(preview_message_text(&message.text)),
                        "assistant" => {
                            latest_assistant_message = Some(preview_message_text(&message.text));
                        }
                        _ => {}
                    }
                }
                messages.push(message.clone());
            }
            ParsedSessionItemKind::Unknown { .. } => {}
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
    let mut searchable_parts = Vec::new();
    if let Some(branch) = &git_branch {
        searchable_parts.push(branch.clone());
    }
    searchable_parts.extend(messages.iter().filter_map(searchable_text_from_message));
    searchable_parts.extend(
        tool_events
            .iter()
            .filter_map(searchable_text_from_tool_event),
    );
    let searchable_text = searchable_parts.join("\n");
    if last_activity_at.is_empty() {
        last_activity_at = started_at.clone();
    }

    Ok(ParsedSession {
        session_id,
        started_at,
        cwd,
        cli_version,
        model_provider,
        git_branch,
        parent_thread_id,
        thread_source,
        agent_nickname,
        agent_role,
        agent_path,
        source_path: path.to_path_buf(),
        modified_unix_seconds,
        title,
        last_activity_at,
        latest_user_message,
        latest_assistant_message,
        items,
        messages,
        tool_events,
        searchable_text,
        malformed_records,
    })
}

fn parse_session_item(record: JsonlRecord) -> ParsedSessionItem {
    let timestamp = record.timestamp.unwrap_or_default();
    let kind = match record.record_type.as_str() {
        "session_meta" => {
            ParsedSessionItemKind::SessionMeta(parse_session_meta(&timestamp, &record.payload))
        }
        "response_item" => parse_tool_event(&timestamp, &record.payload)
            .map(ParsedSessionItemKind::ToolEvent)
            .or_else(|| {
                parse_message(&timestamp, &record.payload).map(ParsedSessionItemKind::Message)
            })
            .unwrap_or_else(|| unknown_session_item(record.record_type, record.payload)),
        "event_msg" => parse_event_message(&timestamp, &record.payload)
            .map(ParsedSessionItemKind::ToolEvent)
            .unwrap_or_else(|| unknown_session_item(record.record_type, record.payload)),
        _ => unknown_session_item(record.record_type, record.payload),
    };

    ParsedSessionItem { timestamp, kind }
}

fn parse_session_meta(timestamp: &str, payload: &Value) -> ParsedSessionMeta {
    ParsedSessionMeta {
        session_id: string_field(payload, "id").or_else(|| string_field(payload, "session_id")),
        started_at: string_field(payload, "timestamp").or_else(|| {
            if timestamp.is_empty() {
                None
            } else {
                Some(timestamp.to_string())
            }
        }),
        cwd: string_field(payload, "cwd"),
        cli_version: string_field(payload, "cli_version"),
        model_provider: string_field(payload, "model_provider"),
        git_branch: payload
            .get("git")
            .and_then(|git| string_field(git, "branch")),
        parent_thread_id: string_field(payload, "parent_thread_id"),
        thread_source: string_field(payload, "thread_source"),
        agent_nickname: string_field(payload, "agent_nickname"),
        agent_role: string_field(payload, "agent_role"),
        agent_path: string_field(payload, "agent_path"),
    }
}

fn unknown_session_item(record_type: String, payload: Value) -> ParsedSessionItemKind {
    ParsedSessionItemKind::Unknown {
        payload_type: string_field(&payload, "type"),
        record_type,
        payload,
    }
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

fn parse_tool_event(timestamp: &str, payload: &Value) -> Option<ParsedToolEvent> {
    let payload_type = string_field(payload, "type")?;
    match payload_type.as_str() {
        "function_call" => parse_function_call(timestamp, payload),
        "function_call_output" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "function_call_output".to_string(),
            name: "function_call_output".to_string(),
            summary: string_field(payload, "output").unwrap_or_default(),
            status: command_status_from_exit_code(
                string_field(payload, "output")
                    .as_deref()
                    .and_then(exit_code_from_command_output),
            ),
            call_id: string_field(payload, "call_id"),
            exit_code: string_field(payload, "output")
                .as_deref()
                .and_then(exit_code_from_command_output),
            duration_ms: None,
            cwd: None,
        }),
        "custom_tool_call" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "custom_tool_call".to_string(),
            name: string_field(payload, "name").unwrap_or_else(|| "custom_tool_call".to_string()),
            summary: payload.get("input").map(value_summary).unwrap_or_default(),
            status: string_field(payload, "status"),
            call_id: string_field(payload, "call_id"),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        }),
        "custom_tool_call_output" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "custom_tool_call_output".to_string(),
            name: string_field(payload, "name")
                .unwrap_or_else(|| "custom_tool_call_output".to_string()),
            summary: string_field(payload, "output").unwrap_or_default(),
            status: None,
            call_id: string_field(payload, "call_id"),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        }),
        "web_search_call" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "web_search_call".to_string(),
            name: "web_search".to_string(),
            summary: payload
                .get("action")
                .map(Value::to_string)
                .unwrap_or_default(),
            status: string_field(payload, "status"),
            call_id: string_field(payload, "call_id"),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        }),
        _ => None,
    }
}

fn parse_function_call(timestamp: &str, payload: &Value) -> Option<ParsedToolEvent> {
    let name = string_field(payload, "name")?;
    let summary = if name == "exec_command" {
        function_call_arguments(payload)
            .and_then(|arguments| string_field(&arguments, "cmd"))
            .unwrap_or_default()
    } else {
        string_field(payload, "arguments").unwrap_or_default()
    };

    Some(ParsedToolEvent {
        timestamp: timestamp.to_string(),
        kind: "function_call".to_string(),
        name,
        summary,
        status: None,
        call_id: string_field(payload, "call_id"),
        exit_code: None,
        duration_ms: None,
        cwd: None,
    })
}

fn parse_event_message(timestamp: &str, payload: &Value) -> Option<ParsedToolEvent> {
    let payload_type = string_field(payload, "type")?;
    match payload_type.as_str() {
        "task_complete" => Some(ParsedToolEvent {
            timestamp: string_field(payload, "completed_at")
                .unwrap_or_else(|| timestamp.to_string()),
            kind: "task_complete".to_string(),
            name: "task_complete".to_string(),
            summary: string_field(payload, "last_agent_message").unwrap_or_default(),
            status: Some("completed".to_string()),
            call_id: None,
            exit_code: None,
            duration_ms: i64_field(payload, "duration_ms"),
            cwd: None,
        }),
        "turn_aborted" => Some(ParsedToolEvent {
            timestamp: string_field(payload, "completed_at")
                .unwrap_or_else(|| timestamp.to_string()),
            kind: "turn_aborted".to_string(),
            name: "turn_aborted".to_string(),
            summary: string_field(payload, "reason").unwrap_or_default(),
            status: Some("aborted".to_string()),
            call_id: None,
            exit_code: None,
            duration_ms: i64_field(payload, "duration_ms"),
            cwd: None,
        }),
        "exec_command_end" => {
            let exit_code = i64_field(payload, "exit_code");
            Some(ParsedToolEvent {
                timestamp: timestamp.to_string(),
                kind: "exec_command_end".to_string(),
                name: "exec_command".to_string(),
                summary: command_summary(payload),
                status: command_status_from_exit_code(exit_code)
                    .or_else(|| string_field(payload, "status")),
                call_id: string_field(payload, "call_id"),
                exit_code,
                duration_ms: duration_ms_from_payload(payload),
                cwd: string_field(payload, "cwd"),
            })
        }
        "mcp_tool_call_end" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "mcp_tool_call_end".to_string(),
            name: mcp_tool_name(payload),
            summary: payload
                .get("invocation")
                .map(Value::to_string)
                .unwrap_or_default(),
            status: string_field(payload, "status"),
            call_id: string_field(payload, "call_id"),
            exit_code: None,
            duration_ms: duration_ms_from_payload(payload),
            cwd: None,
        }),
        "patch_apply_end" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "patch_apply_end".to_string(),
            name: "apply_patch".to_string(),
            summary: patch_summary(payload),
            status: string_field(payload, "status").or_else(|| {
                payload
                    .get("success")
                    .and_then(Value::as_bool)
                    .map(|success| if success { "success" } else { "failed" }.to_string())
            }),
            call_id: string_field(payload, "call_id"),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        }),
        "web_search_end" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "web_search_end".to_string(),
            name: "web_search".to_string(),
            summary: string_field(payload, "query")
                .or_else(|| payload.get("action").map(Value::to_string))
                .unwrap_or_default(),
            status: None,
            call_id: string_field(payload, "call_id"),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        }),
        _ => None,
    }
}

fn function_call_arguments(payload: &Value) -> Option<Value> {
    let arguments = string_field(payload, "arguments")?;
    serde_json::from_str(&arguments).ok()
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn command_summary(payload: &Value) -> String {
    payload
        .get("command")
        .and_then(Value::as_array)
        .and_then(|command| command.last())
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| string_field(payload, "aggregated_output"))
        .unwrap_or_default()
}

fn mcp_tool_name(payload: &Value) -> String {
    payload
        .get("invocation")
        .and_then(|invocation| string_field(invocation, "tool"))
        .unwrap_or_else(|| "mcp_tool_call".to_string())
}

fn command_status_from_exit_code(exit_code: Option<i64>) -> Option<String> {
    exit_code.map(|code| {
        if code == 0 {
            "completed".to_string()
        } else {
            "failed".to_string()
        }
    })
}

fn exit_code_from_command_output(output: &str) -> Option<i64> {
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Process exited with code ")
            .and_then(|code| code.parse::<i64>().ok())
    })
}

fn duration_ms_from_payload(payload: &Value) -> Option<i64> {
    if let Some(duration_ms) = i64_field(payload, "duration_ms") {
        return Some(duration_ms);
    }

    let duration = payload.get("duration")?;
    let seconds = i64_field(duration, "secs")?;
    let nanos = i64_field(duration, "nanos").unwrap_or(0);
    Some(seconds.saturating_mul(1000) + nanos / 1_000_000)
}

fn patch_summary(payload: &Value) -> String {
    payload
        .get("changes")
        .and_then(Value::as_object)
        .map(|changes| changes.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default()
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn i64_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}

fn update_last_activity(last_activity_at: &mut String, timestamp: &str) {
    if timestamp.is_empty() {
        return;
    }
    if last_activity_at.is_empty() || timestamp > last_activity_at.as_str() {
        *last_activity_at = timestamp.to_string();
    }
}

fn title_from_message(message: &ParsedMessage) -> Option<String> {
    if message.role != "user" {
        return None;
    }

    let text = message.text.trim_start();
    if text.is_empty() || is_contextual_user_message(text) {
        return None;
    }

    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !is_noise_line(line))
        .map(ToOwned::to_owned)
}

fn preview_message_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_noise_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn searchable_text_from_message(message: &ParsedMessage) -> Option<String> {
    let text = message.text.trim_start();
    if text.is_empty() || is_contextual_user_message(text) {
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

fn searchable_text_from_tool_event(event: &ParsedToolEvent) -> Option<String> {
    if matches!(
        event.kind.as_str(),
        "function_call_output" | "custom_tool_call_output"
    ) {
        return None;
    }

    let searchable_text = [
        event.kind.as_str(),
        event.name.as_str(),
        event.summary.as_str(),
        event.status.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n");

    if searchable_text.is_empty() {
        None
    } else {
        Some(searchable_text)
    }
}

fn is_meaningful_text(text: &str) -> bool {
    let text = text.trim_start();
    !text.is_empty() && !is_contextual_user_message(text)
}

pub(crate) fn is_contextual_user_message(text: &str) -> bool {
    const CONTEXTUAL_USER_MESSAGE_PREFIXES: &[&str] = &[
        "<environment_context",
        "<skills_instructions>",
        "<external_",
        "<user_shell_command>",
        "<turn_aborted>",
        "<subagent_notification>",
        "<codex_internal_context",
        "<recommended_plugins>",
        "<hook_prompt",
        "# AGENTS.md instructions",
    ];

    CONTEXTUAL_USER_MESSAGE_PREFIXES
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

fn is_noise_line(line: &str) -> bool {
    line.starts_with("<image ")
}

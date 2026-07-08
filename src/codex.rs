use std::{
    collections::HashSet,
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
    pub last_activity_at: String,
    pub latest_user_message: Option<String>,
    pub latest_assistant_message: Option<String>,
    pub messages: Vec<ParsedMessage>,
    pub tool_events: Vec<ParsedToolEvent>,
    pub skill_evidence: Vec<ParsedSkillEvidence>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedToolEvent {
    pub timestamp: String,
    pub kind: String,
    pub name: String,
    pub summary: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSkillEvidence {
    pub timestamp: String,
    pub skill_name: String,
    pub evidence_type: String,
    pub confidence: String,
    pub detail: String,
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
    let mut tool_events = Vec::new();
    let mut skill_evidence = Vec::new();
    let mut seen_skill_evidence = HashSet::new();
    let mut last_activity_at = String::new();
    let mut latest_user_message = None;
    let mut latest_assistant_message = None;
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
            if let Some(tool_event) = parse_tool_event(
                record.timestamp.as_deref().unwrap_or_default(),
                &record.payload,
            ) {
                if tool_event.name == "exec_command" {
                    for evidence in skill_file_read_evidence(&tool_event) {
                        push_unique_skill_evidence(
                            &mut skill_evidence,
                            &mut seen_skill_evidence,
                            evidence,
                        );
                    }
                }
                update_last_activity(&mut last_activity_at, &tool_event.timestamp);
                tool_events.push(tool_event);
            }

            if let Some(message) = parse_message(
                record.timestamp.as_deref().unwrap_or_default(),
                &record.payload,
            ) {
                update_last_activity(&mut last_activity_at, &message.timestamp);
                if is_meaningful_text(&message.text) {
                    match message.role.as_str() {
                        "user" => latest_user_message = Some(preview_message_text(&message.text)),
                        "assistant" => {
                            latest_assistant_message = Some(preview_message_text(&message.text));
                            for evidence in assistant_skill_announcements(&message) {
                                push_unique_skill_evidence(
                                    &mut skill_evidence,
                                    &mut seen_skill_evidence,
                                    evidence,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                messages.push(message);
            }
            continue;
        }

        if record.record_type == "event_msg" {
            if let Some(tool_event) = parse_event_message(
                record.timestamp.as_deref().unwrap_or_default(),
                &record.payload,
            ) {
                update_last_activity(&mut last_activity_at, &tool_event.timestamp);
                tool_events.push(tool_event);
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
        .chain(
            tool_events
                .iter()
                .filter_map(searchable_text_from_tool_event),
        )
        .chain(
            skill_evidence
                .iter()
                .map(searchable_text_from_skill_evidence),
        )
        .collect::<Vec<_>>()
        .join("\n");
    if last_activity_at.is_empty() {
        last_activity_at = started_at.clone();
    }

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
        last_activity_at,
        latest_user_message,
        latest_assistant_message,
        messages,
        tool_events,
        skill_evidence,
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

fn parse_tool_event(timestamp: &str, payload: &Value) -> Option<ParsedToolEvent> {
    let payload_type = string_field(payload, "type")?;
    match payload_type.as_str() {
        "function_call" => parse_function_call(timestamp, payload),
        "function_call_output" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "function_call_output".to_string(),
            name: "function_call_output".to_string(),
            summary: string_field(payload, "output").unwrap_or_default(),
            status: None,
        }),
        "custom_tool_call" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "custom_tool_call".to_string(),
            name: string_field(payload, "name").unwrap_or_else(|| "custom_tool_call".to_string()),
            summary: payload.get("input").map(value_summary).unwrap_or_default(),
            status: string_field(payload, "status"),
        }),
        "custom_tool_call_output" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "custom_tool_call_output".to_string(),
            name: "custom_tool_call_output".to_string(),
            summary: string_field(payload, "output").unwrap_or_default(),
            status: None,
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
        }),
        "web_search_end" => Some(ParsedToolEvent {
            timestamp: timestamp.to_string(),
            kind: "web_search_end".to_string(),
            name: "web_search".to_string(),
            summary: string_field(payload, "query")
                .or_else(|| payload.get("action").map(Value::to_string))
                .unwrap_or_default(),
            status: None,
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

fn patch_summary(payload: &Value) -> String {
    payload
        .get("changes")
        .and_then(Value::as_object)
        .map(|changes| changes.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default()
}

fn skill_file_read_evidence(tool_event: &ParsedToolEvent) -> Vec<ParsedSkillEvidence> {
    skill_names_from_command(&tool_event.summary)
        .into_iter()
        .map(|skill_name| ParsedSkillEvidence {
            timestamp: tool_event.timestamp.clone(),
            skill_name,
            evidence_type: "skill_file_read".to_string(),
            confidence: "high".to_string(),
            detail: tool_event.summary.clone(),
        })
        .collect()
}

fn assistant_skill_announcements(message: &ParsedMessage) -> Vec<ParsedSkillEvidence> {
    announced_skill_names(&message.text)
        .into_iter()
        .map(|skill_name| ParsedSkillEvidence {
            timestamp: message.timestamp.clone(),
            skill_name,
            evidence_type: "assistant_announcement".to_string(),
            confidence: "medium".to_string(),
            detail: preview_message_text(&message.text),
        })
        .collect()
}

fn push_unique_skill_evidence(
    evidence: &mut Vec<ParsedSkillEvidence>,
    seen: &mut HashSet<(String, String, String)>,
    item: ParsedSkillEvidence,
) {
    let key = (
        item.timestamp.clone(),
        item.skill_name.clone(),
        item.evidence_type.clone(),
    );
    if seen.insert(key) {
        evidence.push(item);
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
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
    if text.is_empty() || is_bootstrap_message(text) {
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

fn searchable_text_from_skill_evidence(evidence: &ParsedSkillEvidence) -> String {
    [
        evidence.skill_name.as_str(),
        evidence.evidence_type.as_str(),
        evidence.confidence.as_str(),
        evidence.detail.as_str(),
    ]
    .into_iter()
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn is_meaningful_text(text: &str) -> bool {
    let text = text.trim_start();
    !text.is_empty() && !is_bootstrap_message(text)
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

fn announced_skill_names(text: &str) -> Vec<String> {
    let mut skill_names = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("Using `") {
        let after_start = &remaining[start + "Using `".len()..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let skill_name = &after_start[..end];
        if looks_like_skill_name(skill_name) {
            skill_names.push(skill_name.to_string());
        }
        remaining = &after_start[end + 1..];
    }

    skill_names
}

fn skill_names_from_command(command: &str) -> Vec<String> {
    let mut skill_names = Vec::new();
    let mut search_start = 0usize;

    while let Some(relative_end) = command[search_start..].find("SKILL.md") {
        let skill_file_end = search_start + relative_end + "SKILL.md".len();
        let skill_file_start = command[..skill_file_end]
            .rfind(|character: char| {
                character.is_whitespace() || character == '\'' || character == '"'
            })
            .map(|index| index + 1)
            .unwrap_or(0);
        let skill_path = &command[skill_file_start..skill_file_end];
        if let Some(skill_name) = skill_name_from_path(skill_path) {
            skill_names.push(skill_name);
        }
        search_start = skill_file_end;
    }

    skill_names.sort();
    skill_names.dedup();
    skill_names
}

fn skill_name_from_path(path: &str) -> Option<String> {
    let normalized = path.trim_matches(|character| character == '\'' || character == '"');
    if !normalized.ends_with("/SKILL.md") {
        return None;
    }

    let skill_name = Path::new(normalized)
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())?;
    if skill_name.is_empty() {
        return None;
    }

    if normalized.contains("/.codex/superpowers/skills/") {
        Some(format!("superpowers:{skill_name}"))
    } else {
        Some(skill_name.to_string())
    }
}

fn looks_like_skill_name(skill_name: &str) -> bool {
    !skill_name.is_empty()
        && skill_name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
        })
}

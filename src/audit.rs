use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::codex_cmd;
use crate::db::{Database, SessionDetail};

pub const AUDIT_INPUT_VERSION: i64 = 1;
pub const AUDIT_PROMPT_VERSION: i64 = 1;
pub const DEFAULT_MODEL: &str = "gpt-5.6-luna";
pub const DEFAULT_REASONING_EFFORT: &str = "low";

pub const AUDIT_PROMPT: &str = r#"You are auditing a Codex CLI session transcript.

Your job is to classify the session outcome and identify the most useful next action for the user.

Use only the transcript provided in stdin. Do not assume unstated success. Do not reward effort; evaluate observable progress and ending state.

Return JSON matching the provided schema.

Definitions:
- finished: the user's apparent goal was completed and verified or clearly delivered.
- unfinished: meaningful progress was made, but completion is not shown.
- blocked: the session got stuck on an unresolved error, missing information, failing command, permission issue, or repeated failed attempt.
- exploratory: the session was mainly investigation, brainstorming, planning, or discovery.
- unclear: the transcript does not contain enough signal to classify confidently.

Rules:
- `gist` must be one sentence under 180 characters.
- `hinge` must name the decisive reason for the status.
- `next` must be one concrete action the user can take.
- `signals` must contain 1 to 3 specific observations from the transcript.
- Do not provide a productivity score.
- Do not give generic coaching advice unless the transcript directly shows the issue.
"#;

pub const AUDIT_SCHEMA_JSON: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["status", "gist", "hinge", "next", "signals"],
  "properties": {
    "status": {
      "type": "string",
      "enum": ["finished", "unfinished", "blocked", "exploratory", "unclear"]
    },
    "gist": { "type": "string" },
    "hinge": { "type": "string" },
    "next": { "type": "string" },
    "signals": {
      "type": "array",
      "minItems": 1,
      "maxItems": 3,
      "items": { "type": "string" }
    }
  }
}"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditStatus {
    Finished,
    Unfinished,
    Blocked,
    Exploratory,
    Unclear,
}

impl AuditStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Unfinished => "unfinished",
            Self::Blocked => "blocked",
            Self::Exploratory => "exploratory",
            Self::Unclear => "unclear",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAuditResult {
    pub status: AuditStatus,
    pub gist: String,
    pub hinge: String,
    pub next: String,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAuditRecord {
    pub session_id: String,
    pub source_modified_unix_seconds: i64,
    pub audit_input_version: i64,
    pub prompt_version: i64,
    pub model: String,
    pub reasoning_effort: String,
    pub result: SessionAuditResult,
    pub created_at: String,
}

pub fn run_codex_audit(
    detail: &SessionDetail,
    model: &str,
    reasoning_effort: &str,
) -> Result<SessionAuditRecord> {
    let audit_input = format_audit_input(detail);
    let schema_path = write_schema_file()?;
    let command = codex_cmd::audit_command(
        &schema_path.display().to_string(),
        model,
        reasoning_effort,
        AUDIT_PROMPT,
    );
    let output = codex_cmd::run_with_stdin(command, &audit_input);
    let _ = fs::remove_file(&schema_path);
    let result = parse_audit_result(&output?)?;

    Ok(SessionAuditRecord {
        session_id: detail.summary.session_id.clone(),
        source_modified_unix_seconds: source_modified_unix_seconds(detail),
        audit_input_version: AUDIT_INPUT_VERSION,
        prompt_version: AUDIT_PROMPT_VERSION,
        model: model.to_string(),
        reasoning_effort: reasoning_effort.to_string(),
        result,
        created_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .context("format audit timestamp")?,
    })
}

pub fn audit_session(
    database: &Database,
    detail: &SessionDetail,
    refresh: bool,
    model: &str,
    reasoning_effort: &str,
) -> Result<SessionAuditRecord> {
    let source_modified_unix_seconds = source_modified_unix_seconds(detail);
    let cached_audit = if refresh {
        None
    } else {
        database.get_fresh_session_audit(
            &detail.summary.session_id,
            source_modified_unix_seconds,
            AUDIT_INPUT_VERSION,
            AUDIT_PROMPT_VERSION,
            model,
            reasoning_effort,
        )?
    };

    let audit = match cached_audit {
        Some(audit) => audit,
        None => {
            let audit = run_codex_audit(detail, model, reasoning_effort)?;
            database.upsert_session_audit(&audit)?;
            audit
        }
    };
    Ok(audit)
}

pub fn parse_audit_result(output: &str) -> Result<SessionAuditResult> {
    serde_json::from_str(output.trim()).context("parse Codex audit JSON")
}

pub fn format_audit_input(detail: &SessionDetail) -> String {
    let mut lines = vec![
        format!("AUDIT_INPUT_VERSION: {AUDIT_INPUT_VERSION}"),
        String::new(),
        "SESSION".to_string(),
        format!("id: {}", detail.summary.session_id),
        format!("title: {}", detail.summary.title),
        format!("cwd: {}", detail.summary.cwd),
        format!(
            "branch: {}",
            detail.summary.git_branch.as_deref().unwrap_or("unknown")
        ),
        format!("started: {}", detail.summary.started_at),
        format!("last_activity: {}", detail.summary.last_activity_at),
        String::new(),
        "CONVERSATION".to_string(),
    ];

    let message_omissions = append_messages(&mut lines, detail);

    lines.push(String::new());
    lines.push("TOOLS".to_string());
    let tool_omissions = append_tools(&mut lines, detail);

    append_failures(&mut lines, detail);

    lines.push(String::new());
    lines.push("END STATE".to_string());
    lines.push(format!(
        "last_user: {}",
        detail
            .summary
            .latest_user_message
            .as_deref()
            .map(compact_text)
            .unwrap_or_else(|| "none".to_string())
    ));
    lines.push(format!(
        "last_assistant: {}",
        detail
            .summary
            .latest_assistant_message
            .as_deref()
            .map(compact_text)
            .unwrap_or_else(|| "none".to_string())
    ));

    lines.push(String::new());
    lines.push("OMISSIONS".to_string());
    lines.push(
        "- raw JSONL omitted; this input is built from normalized cx session tables.".to_string(),
    );
    lines.push(format!(
        "- {message_omissions} middle conversation messages omitted."
    ));
    lines.push(format!(
        "- {tool_omissions} long successful tool summaries truncated or omitted."
    ));

    lines.join("\n")
}

fn append_messages(lines: &mut Vec<String>, detail: &SessionDetail) -> usize {
    const WINDOW: usize = 6;
    let messages = &detail.messages;
    if messages.len() <= WINDOW * 2 {
        for message in messages {
            lines.push(format!(
                "[{}] {}",
                message.role,
                limit_text(&message.text, 1_200)
            ));
        }
        return 0;
    }

    for message in messages.iter().take(WINDOW) {
        lines.push(format!(
            "[{}] {}",
            message.role,
            limit_text(&message.text, 1_200)
        ));
    }

    let omitted_count = messages.len().saturating_sub(WINDOW * 2);
    lines.push(format!("[omitted] {omitted_count} middle messages"));

    for message in messages.iter().skip(messages.len() - WINDOW) {
        lines.push(format!(
            "[{}] {}",
            message.role,
            limit_text(&message.text, 1_200)
        ));
    }

    omitted_count
}

fn append_tools(lines: &mut Vec<String>, detail: &SessionDetail) -> usize {
    if detail.tool_events.is_empty() {
        lines.push("- none".to_string());
        return 0;
    }

    let mut omitted_or_truncated = 0usize;
    for event in &detail.tool_events {
        let status = event.status.as_deref().unwrap_or("unknown");
        let exit = event
            .exit_code
            .map(|code| format!(", exit {code}"))
            .unwrap_or_default();
        let summary = if status == "completed" && event.summary.len() > 500 {
            omitted_or_truncated += 1;
            limit_text(&event.summary, 500)
        } else {
            compact_text(&event.summary)
        };
        if matches!(event.kind.as_str(), "task_complete" | "turn_aborted") {
            lines.push(format!("- {}: {status} - {summary}{exit}", event.name));
        } else {
            lines.push(format!("- {}: {} ({status}{exit})", event.name, summary));
        }
    }

    omitted_or_truncated
}

fn append_failures(lines: &mut Vec<String>, detail: &SessionDetail) {
    let failures = detail
        .tool_events
        .iter()
        .filter(|event| {
            event.status.as_deref() == Some("failed")
                || event.exit_code.is_some_and(|exit_code| exit_code != 0)
        })
        .collect::<Vec<_>>();

    if failures.is_empty() {
        return;
    }

    lines.push(String::new());
    lines.push("FAILURES".to_string());
    for failure in failures {
        let exit = failure
            .exit_code
            .map(|code| format!(" exit {code}"))
            .unwrap_or_default();
        lines.push(format!(
            "- {}:{} {}",
            failure.name,
            exit,
            limit_text(&failure.summary, 800)
        ));
    }
}

fn compact_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn limit_text(text: &str, max_chars: usize) -> String {
    let compacted = compact_text(text);
    if compacted.chars().count() <= max_chars {
        return compacted;
    }

    let mut shortened = compacted.chars().take(max_chars).collect::<String>();
    shortened.push_str("...");
    shortened
}

fn write_schema_file() -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "cx-audit-schema-{}-{}.json",
        std::process::id(),
        OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    fs::write(&path, AUDIT_SCHEMA_JSON)
        .with_context(|| format!("write audit schema {}", path.display()))?;
    Ok(path)
}

pub fn source_modified_unix_seconds(detail: &SessionDetail) -> i64 {
    Path::new(&detail.summary.source_path)
        .metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

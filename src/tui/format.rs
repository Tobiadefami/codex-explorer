use std::{collections::HashSet, path::Path};

use crate::{
    codex::{ParsedMessage, ParsedToolEvent},
    db::SessionSummary,
};

use super::refresh::RefreshStatus;

pub struct ConversationWindows<'a> {
    pub opening: Vec<&'a ParsedMessage>,
    pub omitted_count: usize,
    pub recent: Vec<&'a ParsedMessage>,
}

pub struct ToolEventGroups<'a> {
    pub commands: Vec<&'a ParsedToolEvent>,
    pub file_changes: Vec<&'a ParsedToolEvent>,
    pub web_searches: Vec<&'a ParsedToolEvent>,
    pub failures: Vec<&'a ParsedToolEvent>,
    pub other_events: Vec<&'a ParsedToolEvent>,
    pub failure_count: usize,
}

pub struct VisibleToolEvents<'a> {
    pub events: Vec<&'a ParsedToolEvent>,
    pub omitted_count: usize,
}

pub struct OverviewDetailRow {
    pub label: &'static str,
    pub value: String,
}

pub const TOOL_SECTION_ITEM_LIMIT: usize = 5;
const ICON_BRANCH: &str = "";
const ICON_PROJECT: &str = "⌁";
const ICON_UPDATED: &str = "◷";

pub fn session_summary_text_lines(summary: &SessionSummary, expanded: bool) -> Vec<String> {
    let mut lines = vec![summary.title.clone()];
    if !expanded {
        return lines;
    }

    lines.push(session_summary_metadata(summary));
    lines.push(format!(
        "User: {}",
        summary
            .latest_user_message
            .as_deref()
            .map(preview_text)
            .unwrap_or_else(|| "No message found".to_string())
    ));
    lines.push(format!(
        "Assistant: {}",
        summary
            .latest_assistant_message
            .as_deref()
            .map(preview_text)
            .unwrap_or_else(|| "No message found".to_string())
    ));
    lines
}

pub fn session_overview_detail_rows(summary: &SessionSummary) -> Vec<OverviewDetailRow> {
    let mut rows = vec![
        OverviewDetailRow {
            label: "Project",
            value: compact_path(&summary.cwd),
        },
        OverviewDetailRow {
            label: "Path",
            value: summary.cwd.clone(),
        },
        OverviewDetailRow {
            label: "Started",
            value: compact_timestamp(&summary.started_at),
        },
        OverviewDetailRow {
            label: "Last active",
            value: compact_timestamp(&summary.last_activity_at),
        },
    ];

    if let Some(branch) = summary.git_branch.as_deref() {
        rows.insert(
            1,
            OverviewDetailRow {
                label: "Branch",
                value: branch.to_string(),
            },
        );
    }

    rows
}

pub fn agent_activity_text_lines(children: &[SessionSummary]) -> Vec<String> {
    children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let name = child
                .agent_nickname
                .clone()
                .unwrap_or_else(|| format!("Agent {}", index + 1));
            let identity = match child.agent_role.as_deref() {
                Some(role) if !role.trim().is_empty() => format!("{name} · {role}"),
                _ => name,
            };
            format!("{identity}: {}", preview_text(&child.title))
        })
        .collect()
}

pub fn short_session_id(session_id: &str) -> String {
    if session_id.len() <= 16 {
        return session_id.to_string();
    }

    let prefix = &session_id[..8];
    let suffix = &session_id[session_id.len() - 4..];
    format!("{prefix}...{suffix}")
}

pub fn compact_path(path: &str) -> String {
    if path == "/" {
        return path.to_string();
    }

    Path::new(path)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .filter(|file_name| !file_name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string())
}

pub fn compact_timestamp(timestamp: &str) -> String {
    if timestamp.len() >= 16 && timestamp.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &timestamp[..10], &timestamp[11..16])
    } else {
        timestamp.to_string()
    }
}

pub fn empty_results_message(query: &str) -> String {
    if query.trim().is_empty() {
        "No sessions indexed yet".to_string()
    } else {
        format!("No sessions match {:?}", query)
    }
}

pub fn empty_state_message(query: &str, refresh_status: &RefreshStatus) -> String {
    if matches!(refresh_status, RefreshStatus::Running { .. }) {
        if query.trim().is_empty() {
            "Refreshing sessions...".to_string()
        } else {
            format!("Refreshing matches for {:?}...", query)
        }
    } else {
        empty_results_message(query)
    }
}

pub fn conversation_windows(
    messages: &[ParsedMessage],
    window_size: usize,
) -> ConversationWindows<'_> {
    let meaningful_messages = messages
        .iter()
        .filter(|message| is_meaningful_message(&message.text))
        .collect::<Vec<_>>();

    if meaningful_messages.len() <= window_size * 2 {
        return ConversationWindows {
            opening: meaningful_messages,
            omitted_count: 0,
            recent: Vec::new(),
        };
    }

    let opening = meaningful_messages
        .iter()
        .take(window_size)
        .copied()
        .collect::<Vec<_>>();
    let recent_start = meaningful_messages.len() - window_size;
    let recent = meaningful_messages
        .iter()
        .skip(recent_start)
        .copied()
        .collect::<Vec<_>>();

    ConversationWindows {
        opening,
        omitted_count: recent_start - window_size,
        recent,
    }
}

pub fn group_tool_events(events: &[ParsedToolEvent]) -> ToolEventGroups<'_> {
    let completed_command_call_ids = events
        .iter()
        .filter(|event| event.kind == "exec_command_end")
        .filter_map(|event| event.call_id.as_deref())
        .collect::<HashSet<_>>();
    let mut failure_keys = HashSet::new();
    let mut groups = ToolEventGroups {
        commands: Vec::new(),
        file_changes: Vec::new(),
        web_searches: Vec::new(),
        failures: Vec::new(),
        other_events: Vec::new(),
        failure_count: 0,
    };

    for event in events {
        if is_failure(event) && failure_keys.insert(tool_event_key(event)) {
            groups.failures.push(event);
            groups.failure_count += 1;
        }

        if is_superseded_command_start(event, &completed_command_call_ids) {
            continue;
        }

        if event.name == "exec_command" {
            groups.commands.push(event);
        } else if event.name == "apply_patch" || event.kind == "patch_apply_end" {
            groups.file_changes.push(event);
        } else if event.name == "web_search" {
            groups.web_searches.push(event);
        } else if !is_low_signal_tool_event(event) {
            groups.other_events.push(event);
        }
    }

    groups
}

pub fn visible_tool_events<'a>(events: &[&'a ParsedToolEvent]) -> VisibleToolEvents<'a> {
    VisibleToolEvents {
        events: events
            .iter()
            .take(TOOL_SECTION_ITEM_LIMIT)
            .copied()
            .collect(),
        omitted_count: events.len().saturating_sub(TOOL_SECTION_ITEM_LIMIT),
    }
}

pub(super) fn preview_text(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    trim_to_chars(&normalized, 180)
}

pub(super) fn trim_to_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut trimmed = text
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    trimmed.push_str("...");
    trimmed
}

fn is_meaningful_message(text: &str) -> bool {
    let text = text.trim_start();
    if text.is_empty() {
        return false;
    }

    const BOOTSTRAP_PREFIXES: &[&str] = &[
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

    !BOOTSTRAP_PREFIXES
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

fn is_failure(event: &ParsedToolEvent) -> bool {
    event
        .status
        .as_deref()
        .map(|status| {
            let status = status.to_ascii_lowercase();
            status.contains("fail") || status.contains("error")
        })
        .unwrap_or(false)
}

fn is_superseded_command_start(
    event: &ParsedToolEvent,
    completed_call_ids: &HashSet<&str>,
) -> bool {
    event.kind == "function_call"
        && event.name == "exec_command"
        && event
            .call_id
            .as_deref()
            .is_some_and(|call_id| completed_call_ids.contains(call_id))
}

fn session_summary_metadata(summary: &SessionSummary) -> String {
    let mut parts = vec![format!("{ICON_PROJECT} {}", compact_path(&summary.cwd))];
    if let Some(branch) = summary.git_branch.as_deref() {
        parts.push(format!("{ICON_BRANCH} {branch}"));
    }
    parts.push(format!(
        "{ICON_UPDATED} {}",
        compact_timestamp(&summary.last_activity_at)
    ));
    if summary.child_session_count > 0 {
        let noun = if summary.child_session_count == 1 {
            "agent"
        } else {
            "agents"
        };
        parts.push(format!("↳ {} {noun}", summary.child_session_count));
    }
    parts.join("  ")
}

fn tool_event_key(event: &ParsedToolEvent) -> String {
    event.call_id.as_ref().cloned().unwrap_or_else(|| {
        format!(
            "{}:{}:{}:{}",
            event.timestamp, event.kind, event.name, event.summary
        )
    })
}

fn is_low_signal_tool_event(event: &ParsedToolEvent) -> bool {
    matches!(
        event.kind.as_str(),
        "function_call_output" | "custom_tool_call_output" | "task_complete"
    )
}

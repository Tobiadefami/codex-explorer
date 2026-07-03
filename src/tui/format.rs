use std::path::Path;

use crate::codex::ParsedMessage;

use super::refresh::RefreshStatus;

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

pub fn meaningful_preview_messages(
    messages: &[ParsedMessage],
    limit: usize,
) -> Vec<&ParsedMessage> {
    messages
        .iter()
        .filter(|message| is_meaningful_message(&message.text))
        .take(limit)
        .collect()
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

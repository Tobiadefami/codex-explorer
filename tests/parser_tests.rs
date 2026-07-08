#[path = "../src/codex.rs"]
mod codex;

use std::path::Path;

use codex::{ParsedSessionItemKind, ParsedToolEvent};

#[test]
fn parses_session_metadata_and_messages() {
    let parsed = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();

    assert_eq!(parsed.session_id, "11111111-1111-4111-8111-111111111111");
    assert_eq!(parsed.cwd, "/work/project-a");
    assert_eq!(parsed.started_at, "2026-07-01T10:00:00Z");
    assert_eq!(parsed.title, "add turnstile to the signup form");
    assert_eq!(parsed.cli_version.as_deref(), Some("0.142.5"));
    assert_eq!(parsed.model_provider.as_deref(), Some("openai"));
    assert_eq!(
        parsed.source_path,
        Path::new("tests/fixtures/session-a.jsonl")
    );
    assert!(parsed.modified_unix_seconds >= 0);
    assert_eq!(parsed.messages.len(), 2);
    assert_eq!(parsed.messages[0].role, "user");
    assert_eq!(parsed.messages[0].text, "add turnstile to the signup form");
    assert_eq!(parsed.messages[1].role, "assistant");
    assert_eq!(
        parsed.messages[1].text,
        "I will inspect the Worker and form code."
    );
    assert_eq!(
        parsed.searchable_text,
        "add turnstile to the signup form\nI will inspect the Worker and form code."
    );
    assert_eq!(parsed.malformed_records, 0);
}

#[test]
fn preserves_ordered_session_items_for_known_records() {
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-overview-ui.jsonl")).unwrap();

    assert_eq!(parsed.items.len(), 15);

    assert!(matches!(
        parsed.items[0].kind,
        ParsedSessionItemKind::SessionMeta(_)
    ));
    assert_message_item(
        &parsed.items[1].kind,
        "user",
        "add turnstile to the signup form",
    );
    assert_message_item(
        &parsed.items[2].kind,
        "assistant",
        "I will inspect the Worker and form code.",
    );
    assert_tool_item(&parsed.items[3].kind, "function_call", "exec_command");
    assert_tool_item(
        &parsed.items[4].kind,
        "function_call_output",
        "function_call_output",
    );
    assert_tool_item(&parsed.items[14].kind, "task_complete", "task_complete");
}

#[test]
fn preserves_unknown_records_in_session_items() {
    let temp_dir = tempfile::tempdir().unwrap();
    let session_path = temp_dir.path().join("unknown-record.jsonl");
    std::fs::write(
        &session_path,
        concat!(
            "{\"timestamp\":\"2026-07-03T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"66666666-6666-4666-8666-666666666666\",\"timestamp\":\"2026-07-03T10:00:00Z\",\"cwd\":\"/work/project-f\"}}\n",
            "{\"timestamp\":\"2026-07-03T10:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"context_compaction\",\"message\":\"compacted previous history\"}}\n",
        ),
    )
    .unwrap();

    let parsed = codex::parse_session_file(&session_path).unwrap();

    assert_eq!(parsed.items.len(), 2);
    match &parsed.items[1].kind {
        ParsedSessionItemKind::Unknown {
            record_type,
            payload_type,
            ..
        } => {
            assert_eq!(record_type, "response_item");
            assert_eq!(payload_type.as_deref(), Some("context_compaction"));
        }
        other => panic!("expected unknown session item, got {other:?}"),
    }
}

#[test]
fn skips_malformed_records_and_keeps_valid_messages() {
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-malformed.jsonl")).unwrap();

    assert_eq!(parsed.session_id, "22222222-2222-4222-8222-222222222222");
    assert_eq!(parsed.title, "debug websocket reconnect loop");
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.malformed_records, 1);
}

#[test]
fn title_skips_environment_context_messages() {
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-environment-title.jsonl"))
            .unwrap();

    assert_eq!(parsed.title, "build a codex session manager");
    assert_eq!(parsed.messages.len(), 3);
    assert!(!parsed.searchable_text.contains("<environment_context>"));
}

#[test]
fn title_skips_image_placeholder_lines() {
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-image-title.jsonl")).unwrap();

    assert_eq!(parsed.title, "explain what is broken in this screenshot");
    assert_eq!(parsed.messages.len(), 2);
}

#[test]
fn parses_thread_source_for_subagent_sessions() {
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-subagent.jsonl")).unwrap();

    assert_eq!(parsed.session_id, "33333333-3333-4333-8333-333333333333");
    assert_eq!(
        parsed.parent_thread_id.as_deref(),
        Some("11111111-1111-4111-8111-111111111111")
    );
    assert_eq!(parsed.thread_source.as_deref(), Some("subagent"));
    assert!(parsed.is_subagent_thread());
}

#[test]
fn extracts_overview_activity_and_tool_events() {
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-overview-ui.jsonl")).unwrap();

    assert_eq!(parsed.title, "add turnstile to the signup form");
    assert_eq!(parsed.last_activity_at, "2026-07-02T09:06:04Z");
    assert_eq!(
        parsed.latest_user_message.as_deref(),
        Some("now audit how tools are used")
    );
    assert_eq!(
        parsed.latest_assistant_message.as_deref(),
        Some("I found command and task events in the local session logs.")
    );

    assert_eq!(parsed.tool_events.len(), 3);
    assert_eq!(parsed.tool_events[0].kind, "function_call");
    assert_eq!(parsed.tool_events[0].name, "exec_command");
    assert_eq!(
        parsed.tool_events[0].call_id.as_deref(),
        Some("call_read_source")
    );
    assert!(parsed.tool_events[0].summary.contains("src/main.rs"));
    assert_eq!(parsed.tool_events[1].kind, "function_call_output");
    assert_eq!(
        parsed.tool_events[1].call_id.as_deref(),
        Some("call_read_source")
    );
    assert_eq!(parsed.tool_events[2].kind, "task_complete");
    assert_eq!(parsed.tool_events[2].status.as_deref(), Some("completed"));
    assert_eq!(parsed.tool_events[2].duration_ms, Some(364000));
}

#[test]
fn parses_command_end_metadata_and_failures() {
    let temp_dir = tempfile::tempdir().unwrap();
    let session_path = temp_dir.path().join("command-failure.jsonl");
    std::fs::write(
        &session_path,
        concat!(
            "{\"timestamp\":\"2026-07-03T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"66666666-6666-4666-8666-666666666666\",\"timestamp\":\"2026-07-03T10:00:00Z\",\"cwd\":\"/work/project-f\"}}\n",
            "{\"timestamp\":\"2026-07-03T10:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"arguments\":\"{\\\"cmd\\\":\\\"cargo test\\\",\\\"workdir\\\":\\\"/work/project-f\\\"}\",\"call_id\":\"call_cmd\"}}\n",
            "{\"timestamp\":\"2026-07-03T10:00:02Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"exec_command_end\",\"call_id\":\"call_cmd\",\"command\":[\"/usr/bin/zsh\",\"-lc\",\"cargo test\"],\"cwd\":\"/work/project-f\",\"stdout\":\"\",\"stderr\":\"failed\",\"aggregated_output\":\"failed\",\"exit_code\":101,\"duration\":{\"secs\":2,\"nanos\":500000000},\"status\":\"completed\"}}\n",
            "{\"timestamp\":\"2026-07-03T10:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"call_id\":\"call_other\",\"output\":\"Process exited with code 2\\nOutput:\\nfailed\"}}\n",
        ),
    )
    .unwrap();

    let parsed = codex::parse_session_file(&session_path).unwrap();

    assert_eq!(parsed.tool_events.len(), 3);
    assert_eq!(parsed.tool_events[1].kind, "exec_command_end");
    assert_eq!(parsed.tool_events[1].name, "exec_command");
    assert_eq!(parsed.tool_events[1].call_id.as_deref(), Some("call_cmd"));
    assert_eq!(parsed.tool_events[1].summary, "cargo test");
    assert_eq!(parsed.tool_events[1].status.as_deref(), Some("failed"));
    assert_eq!(parsed.tool_events[1].exit_code, Some(101));
    assert_eq!(parsed.tool_events[1].duration_ms, Some(2500));
    assert_eq!(
        parsed.tool_events[1].cwd.as_deref(),
        Some("/work/project-f")
    );
    assert_eq!(parsed.tool_events[2].status.as_deref(), Some("failed"));
    assert_eq!(parsed.tool_events[2].exit_code, Some(2));
}

fn assert_message_item(kind: &ParsedSessionItemKind, role: &str, text: &str) {
    match kind {
        ParsedSessionItemKind::Message(message) => {
            assert_eq!(message.role, role);
            assert_eq!(message.text, text);
        }
        other => panic!("expected message item, got {other:?}"),
    }
}

fn assert_tool_item(kind: &ParsedSessionItemKind, expected_kind: &str, name: &str) {
    match kind {
        ParsedSessionItemKind::ToolEvent(ParsedToolEvent {
            kind,
            name: actual_name,
            ..
        }) => {
            assert_eq!(kind, expected_kind);
            assert_eq!(actual_name, name);
        }
        other => panic!("expected tool event item, got {other:?}"),
    }
}

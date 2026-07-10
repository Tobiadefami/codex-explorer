#[allow(dead_code)]
#[path = "../src/audit.rs"]
mod audit;
#[path = "../src/codex.rs"]
mod codex;
#[allow(dead_code)]
#[path = "../src/codex_cmd.rs"]
mod codex_cmd;
#[allow(dead_code)]
#[path = "../src/db.rs"]
mod db;
#[path = "../src/indexer.rs"]
mod indexer;
#[allow(dead_code)]
#[path = "../src/tui/mod.rs"]
mod tui;

use db::Database;
use tui::format::{
    compact_path, compact_timestamp, conversation_windows, empty_results_message,
    empty_state_message, group_tool_events, session_overview_detail_rows,
    session_summary_text_lines, short_session_id, visible_tool_events,
};
use tui::refresh::RefreshStatus;
use tui::state::{PreviewMode, ProjectScope, TuiState};

fn fixture_sessions_dir(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let sessions_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::copy(
        "tests/fixtures/session-a.jsonl",
        sessions_dir.join("session-a.jsonl"),
    )
    .unwrap();
    std::fs::copy(
        "tests/fixtures/session-malformed.jsonl",
        sessions_dir.join("session-malformed.jsonl"),
    )
    .unwrap();
    sessions_dir
}

fn indexed_database(temp: &tempfile::TempDir) -> Database {
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let sessions_dir = fixture_sessions_dir(temp);
    indexer::reindex(&database, &sessions_dir).unwrap();
    database
}

fn scoped_database(temp: &tempfile::TempDir) -> Database {
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut first =
        codex::parse_session_file(std::path::Path::new("tests/fixtures/session-a.jsonl")).unwrap();
    first.cwd = "/work/project-a".to_string();
    database.upsert_session(&first).unwrap();

    let mut second = codex::parse_session_file(std::path::Path::new(
        "tests/fixtures/session-malformed.jsonl",
    ))
    .unwrap();
    second.cwd = "/work/project-b".to_string();
    database.upsert_session(&second).unwrap();

    database
}

#[test]
fn initial_state_loads_recent_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);

    let state = TuiState::load(&database, 20).unwrap();

    assert_eq!(state.query(), "");
    assert_eq!(state.summaries().len(), 2);
    assert_eq!(
        state.selected_session_id(),
        Some("22222222-2222-4222-8222-222222222222")
    );
}

#[test]
fn query_filters_sessions_through_search_index() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    state.set_query(&database, "turnstile".to_string()).unwrap();

    assert_eq!(state.summaries().len(), 1);
    assert_eq!(
        state.selected_session_id(),
        Some("11111111-1111-4111-8111-111111111111")
    );
}

#[test]
fn selection_moves_within_loaded_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    state.move_down();
    assert_eq!(
        state.selected_session_id(),
        Some("11111111-1111-4111-8111-111111111111")
    );

    state.move_down();
    assert_eq!(
        state.selected_session_id(),
        Some("11111111-1111-4111-8111-111111111111")
    );

    state.move_up();
    assert_eq!(
        state.selected_session_id(),
        Some("22222222-2222-4222-8222-222222222222")
    );
}

#[test]
fn empty_search_clears_selection() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    state
        .set_query(&database, "not-present".to_string())
        .unwrap();

    assert!(state.summaries().is_empty());
    assert_eq!(state.selected_session_id(), None);
}

#[test]
fn state_reports_recent_and_search_result_labels() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    assert_eq!(state.mode_label(), "All projects");
    assert_eq!(state.result_label(), "2 sessions");

    state.set_query(&database, "turnstile".to_string()).unwrap();

    assert_eq!(state.mode_label(), "Search");
    assert_eq!(state.result_label(), "1 match");
}

#[test]
fn state_tracks_preview_modes() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    assert_eq!(state.preview_mode(), PreviewMode::Overview);
    assert_eq!(state.preview_mode_label(), "Overview");

    state.set_preview_mode(PreviewMode::Conversation);
    assert_eq!(state.preview_mode(), PreviewMode::Conversation);
    assert_eq!(state.preview_mode_label(), "Conversation");

    state.scroll_preview_down();
    state.set_preview_mode(PreviewMode::Tools);
    assert_eq!(state.preview_mode(), PreviewMode::Tools);
    assert_eq!(state.preview_scroll(), 0);

    state.set_preview_mode(PreviewMode::Timeline);
    assert_eq!(state.preview_mode(), PreviewMode::Timeline);
    assert_eq!(state.preview_mode_label(), "Timeline");

    state.set_preview_mode(PreviewMode::Audit);
    assert_eq!(state.preview_mode(), PreviewMode::Audit);
    assert_eq!(state.preview_mode_label(), "Audit");
}

#[test]
fn state_toggles_expansion_for_selected_session() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    assert_eq!(state.expanded_session_id(), None);

    let first_session_id = state.selected_session_id().unwrap().to_string();
    state.toggle_selected_expansion();
    assert_eq!(state.expanded_session_id(), Some(first_session_id.as_str()));
    assert!(state.is_summary_expanded(state.selected_summary().unwrap()));

    state.move_down();
    assert_eq!(state.expanded_session_id(), Some(first_session_id.as_str()));
    assert!(!state.is_summary_expanded(state.selected_summary().unwrap()));

    let second_session_id = state.selected_session_id().unwrap().to_string();
    state.toggle_selected_expansion();
    assert_eq!(
        state.expanded_session_id(),
        Some(second_session_id.as_str())
    );

    state.toggle_selected_expansion();
    assert_eq!(state.expanded_session_id(), None);
}

#[test]
fn reload_clears_stale_expanded_session() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    state.toggle_selected_expansion();
    assert!(state.expanded_session_id().is_some());

    state
        .set_query(&database, "not-present".to_string())
        .unwrap();

    assert!(state.summaries().is_empty());
    assert_eq!(state.expanded_session_id(), None);
}

#[test]
fn expanded_session_summary_lines_focus_on_branch_activity_and_messages() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let summary = database
        .search_sessions("feature/session-browser", 10)
        .unwrap()
        .remove(0);

    let lines = session_summary_text_lines(&summary, true);

    assert_eq!(lines[0], "add turnstile to the signup form");
    assert!(lines[1].contains("⌁ project-a"));
    assert!(lines[1].contains(" feature/session-browser"));
    assert!(lines[1].contains("◷ 2026-07-01 10:00"));
    assert!(lines[2].contains("User: add turnstile to the signup form"));
    assert!(lines[3].contains("Assistant: I will inspect the Worker and form code."));
    assert_eq!(lines.len(), 4);
}

#[test]
fn expanded_parent_summary_includes_agent_count() {
    let temp = tempfile::tempdir().unwrap();
    let sessions_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::copy(
        "tests/fixtures/session-a.jsonl",
        sessions_dir.join("session-a.jsonl"),
    )
    .unwrap();
    std::fs::copy(
        "tests/fixtures/session-subagent.jsonl",
        sessions_dir.join("session-subagent.jsonl"),
    )
    .unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    indexer::reindex(&database, &sessions_dir).unwrap();
    let parent = database.list_sessions(10).unwrap().remove(0);

    let lines = session_summary_text_lines(&parent, true);

    assert!(lines[1].contains("↳ 1 agent"));
}

#[test]
fn overview_detail_rows_include_git_branch_when_present() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let summary = database
        .search_sessions("feature/session-browser", 10)
        .unwrap()
        .remove(0);

    let rows = session_overview_detail_rows(&summary);

    assert!(rows
        .iter()
        .any(|row| row.label == "Branch" && row.value == "feature/session-browser"));
}

#[test]
fn display_helpers_create_compact_session_metadata() {
    assert_eq!(
        short_session_id("11111111-1111-4111-8111-111111111111"),
        "11111111...1111"
    );
    assert_eq!(short_session_id("short-id"), "short-id");
    assert_eq!(compact_path("/home/chief/codex-explorer"), "codex-explorer");
    assert_eq!(compact_path("/"), "/");
    assert_eq!(
        compact_timestamp("2026-07-01T10:00:00Z"),
        "2026-07-01 10:00"
    );
    assert_eq!(compact_timestamp("not-a-timestamp"), "not-a-timestamp");
    assert_eq!(
        empty_results_message("turnstile"),
        "No sessions match \"turnstile\""
    );
    assert_eq!(
        empty_state_message("", &RefreshStatus::running()),
        "Refreshing sessions..."
    );
    assert_eq!(
        empty_state_message("turnstile", &RefreshStatus::running()),
        "Refreshing matches for \"turnstile\"..."
    );
}

#[test]
fn preview_messages_skip_bootstrap_context_and_limit_results() {
    let messages = vec![
        codex::ParsedMessage {
            timestamp: "2026-07-01T10:00:00Z".to_string(),
            role: "user".to_string(),
            text: "<environment_context>\n  <cwd>/work/project</cwd>\n</environment_context>"
                .to_string(),
        },
        codex::ParsedMessage {
            timestamp: "2026-07-01T10:00:01Z".to_string(),
            role: "user".to_string(),
            text: "build a better session browser".to_string(),
        },
        codex::ParsedMessage {
            timestamp: "2026-07-01T10:00:02Z".to_string(),
            role: "assistant".to_string(),
            text: "I will improve the TUI layout.".to_string(),
        },
    ];

    let windows = conversation_windows(&messages, 1);

    assert_eq!(windows.opening.len(), 2);
    assert_eq!(windows.opening[0].text, "build a better session browser");
    assert_eq!(windows.opening[1].text, "I will improve the TUI layout.");
    assert_eq!(windows.omitted_count, 0);
    assert!(windows.recent.is_empty());
}

#[test]
fn conversation_windows_return_opening_and_recent_messages_without_overlap() {
    let messages = (0..12)
        .map(|index| codex::ParsedMessage {
            timestamp: format!("2026-07-01T10:{index:02}:00Z"),
            role: if index % 2 == 0 { "user" } else { "assistant" }.to_string(),
            text: format!("message {index}"),
        })
        .collect::<Vec<_>>();

    let windows = conversation_windows(&messages, 5);

    assert_eq!(windows.opening.len(), 5);
    assert_eq!(windows.opening[0].text, "message 0");
    assert_eq!(windows.opening[4].text, "message 4");
    assert_eq!(windows.omitted_count, 2);
    assert_eq!(windows.recent.len(), 5);
    assert_eq!(windows.recent[0].text, "message 7");
    assert_eq!(windows.recent[4].text, "message 11");
}

#[test]
fn tool_event_groups_collect_scannable_categories() {
    let events = vec![
        codex::ParsedToolEvent {
            timestamp: "2026-07-01T10:00:00Z".to_string(),
            kind: "function_call".to_string(),
            name: "exec_command".to_string(),
            summary: "cargo test".to_string(),
            status: None,
            call_id: Some("call_cargo".to_string()),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        },
        codex::ParsedToolEvent {
            timestamp: "2026-07-01T10:00:01Z".to_string(),
            kind: "exec_command_end".to_string(),
            name: "exec_command".to_string(),
            summary: "cargo test".to_string(),
            status: Some("failed".to_string()),
            call_id: Some("call_cargo".to_string()),
            exit_code: Some(101),
            duration_ms: Some(1200),
            cwd: Some("/work/project-a".to_string()),
        },
        codex::ParsedToolEvent {
            timestamp: "2026-07-01T10:01:00Z".to_string(),
            kind: "patch_apply_end".to_string(),
            name: "apply_patch".to_string(),
            summary: "src/tui/render.rs".to_string(),
            status: Some("completed".to_string()),
            call_id: Some("call_patch".to_string()),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        },
        codex::ParsedToolEvent {
            timestamp: "2026-07-01T10:02:00Z".to_string(),
            kind: "web_search_end".to_string(),
            name: "web_search".to_string(),
            summary: "Codex hooks".to_string(),
            status: None,
            call_id: Some("call_web".to_string()),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        },
        codex::ParsedToolEvent {
            timestamp: "2026-07-01T10:03:00Z".to_string(),
            kind: "function_call_output".to_string(),
            name: "function_call_output".to_string(),
            summary: "test failed".to_string(),
            status: Some("failed".to_string()),
            call_id: Some("call_output".to_string()),
            exit_code: Some(2),
            duration_ms: None,
            cwd: None,
        },
    ];

    let groups = group_tool_events(&events);

    assert_eq!(groups.commands.len(), 1);
    assert_eq!(groups.commands[0].kind, "exec_command_end");
    assert_eq!(groups.file_changes.len(), 1);
    assert_eq!(groups.web_searches.len(), 1);
    assert_eq!(groups.failures.len(), 2);
    assert_eq!(groups.failure_count, 2);
}

#[test]
fn visible_tool_events_limits_each_section_to_five_items() {
    let events = (0..8)
        .map(|index| codex::ParsedToolEvent {
            timestamp: format!("2026-07-01T10:0{index}:00Z"),
            kind: "function_call".to_string(),
            name: "exec_command".to_string(),
            summary: format!("command {index}"),
            status: None,
            call_id: Some(format!("call_{index}")),
            exit_code: None,
            duration_ms: None,
            cwd: None,
        })
        .collect::<Vec<_>>();
    let refs = events.iter().collect::<Vec<_>>();

    let visible = visible_tool_events(&refs);

    assert_eq!(visible.events.len(), 5);
    assert_eq!(visible.omitted_count, 3);
}

#[test]
fn refresh_status_reports_progress_and_outcomes() {
    let mut status = RefreshStatus::running();

    assert_eq!(status.label(), "Refreshing |");

    status.tick();
    assert_eq!(status.label(), "Refreshing /");

    let complete = RefreshStatus::complete(4, 3);
    assert_eq!(complete.label(), "Refreshed 3 sessions from 4 files");

    let failed = RefreshStatus::failed("database is locked");
    assert_eq!(failed.label(), "Refresh failed: database is locked");
}

#[test]
fn reload_keeps_current_query_after_background_refresh() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = Database::open(&db_path).unwrap();
    let sessions_dir = fixture_sessions_dir(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    state.set_query(&database, "turnstile".to_string()).unwrap();
    assert!(state.summaries().is_empty());

    indexer::reindex(&database, &sessions_dir).unwrap();
    state.reload(&database).unwrap();

    assert_eq!(state.query(), "turnstile");
    assert_eq!(state.summaries().len(), 1);
    assert_eq!(
        state.selected_session_id(),
        Some("11111111-1111-4111-8111-111111111111")
    );
}

#[test]
fn scoped_state_defaults_to_current_directory_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let database = scoped_database(&temp);

    let state = TuiState::load_scoped(&database, "/work/project-a".into(), 20).unwrap();

    assert_eq!(state.scope(), &ProjectScope::CurrentDirectory);
    assert_eq!(state.scope_label(), "Current directory");
    assert_eq!(state.summaries().len(), 1);
    assert_eq!(
        state.selected_session_id(),
        Some("11111111-1111-4111-8111-111111111111")
    );
}

#[test]
fn scoped_state_can_toggle_between_current_directory_and_all_projects() {
    let temp = tempfile::tempdir().unwrap();
    let database = scoped_database(&temp);
    let mut state = TuiState::load_scoped(&database, "/work/project-a".into(), 20).unwrap();

    state.show_all_projects(&database).unwrap();
    assert_eq!(state.scope(), &ProjectScope::AllProjects);
    assert_eq!(state.scope_label(), "All projects");
    assert_eq!(state.summaries().len(), 2);

    state.show_current_directory(&database).unwrap();
    assert_eq!(state.scope(), &ProjectScope::CurrentDirectory);
    assert_eq!(state.summaries().len(), 1);
}

#[test]
fn scoped_state_falls_back_to_all_projects_when_current_directory_has_no_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let database = scoped_database(&temp);

    let state = TuiState::load_scoped(&database, "/work/missing".into(), 20).unwrap();

    assert_eq!(state.scope(), &ProjectScope::AllProjects);
    assert_eq!(state.scope_label(), "All projects");
    assert_eq!(
        state.scope_note(),
        Some("No sessions for current directory")
    );
    assert_eq!(state.summaries().len(), 2);
}

#[test]
fn preview_scroll_moves_and_resets_on_selection_change() {
    let temp = tempfile::tempdir().unwrap();
    let database = indexed_database(&temp);
    let mut state = TuiState::load(&database, 20).unwrap();

    state.scroll_preview_down();
    state.scroll_preview_down();
    assert_eq!(state.preview_scroll(), 2);

    state.move_down();
    assert_eq!(state.preview_scroll(), 0);

    state.scroll_preview_down();
    state.scroll_preview_up();
    assert_eq!(state.preview_scroll(), 0);
}

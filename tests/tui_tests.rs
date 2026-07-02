#[path = "../src/codex.rs"]
mod codex;
#[allow(dead_code)]
#[path = "../src/db.rs"]
mod db;
#[path = "../src/indexer.rs"]
mod indexer;
#[allow(dead_code)]
#[path = "../src/tui.rs"]
mod tui;

use db::Database;
use tui::{
    compact_path, compact_timestamp, empty_results_message, empty_state_message,
    meaningful_preview_messages, short_session_id, RefreshStatus, TuiState,
};

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

    assert_eq!(state.mode_label(), "Recent");
    assert_eq!(state.result_label(), "2 sessions");

    state.set_query(&database, "turnstile".to_string()).unwrap();

    assert_eq!(state.mode_label(), "Search");
    assert_eq!(state.result_label(), "1 match");
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

    let preview_messages = meaningful_preview_messages(&messages, 1);

    assert_eq!(preview_messages.len(), 1);
    assert_eq!(preview_messages[0].text, "build a better session browser");
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

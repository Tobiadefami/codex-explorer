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
use tui::TuiState;

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

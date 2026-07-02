#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/db.rs"]
mod db;
#[path = "../src/indexer.rs"]
mod indexer;

use std::path::Path;

#[test]
fn stores_lists_searches_and_shows_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let parsed = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();

    database.upsert_session(&parsed).unwrap();

    let listed = database.list_sessions(10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, parsed.session_id);
    assert_eq!(listed[0].title, "add turnstile to the signup form");

    let results = database.search_sessions("turnstile", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, parsed.session_id);

    let shown = database.get_session(&parsed.session_id).unwrap().unwrap();
    assert_eq!(shown.messages.len(), 2);
    assert_eq!(shown.messages[0].text, "add turnstile to the signup form");
}

#[test]
fn upsert_replaces_existing_source_path_and_cleans_stale_fts() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let original = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();

    database.upsert_session(&original).unwrap();

    let mut replacement = original.clone();
    replacement.session_id = "33333333-3333-4333-8333-333333333333".to_string();
    replacement.title = "debug durable object alarm".to_string();
    replacement.searchable_text = "debug durable object alarm".to_string();
    replacement.messages = vec![codex::ParsedMessage {
        timestamp: "2026-07-01T12:00:00Z".to_string(),
        role: "user".to_string(),
        text: "debug durable object alarm".to_string(),
    }];

    database.upsert_session(&replacement).unwrap();

    assert!(database
        .get_session(&original.session_id)
        .unwrap()
        .is_none());
    let shown = database
        .get_session(&replacement.session_id)
        .unwrap()
        .unwrap();
    assert_eq!(shown.messages.len(), 1);
    assert_eq!(shown.messages[0].text, "debug durable object alarm");

    assert!(database
        .search_sessions("turnstile", 10)
        .unwrap()
        .is_empty());
    let results = database.search_sessions("durable", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, replacement.session_id);
}

#[test]
fn search_treats_user_input_as_plain_text() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let parsed = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();
    database.upsert_session(&parsed).unwrap();

    assert!(database
        .search_sessions("not-present", 10)
        .unwrap()
        .is_empty());
    assert!(database.search_sessions("\"turnstile", 10).is_ok());
    assert!(database.search_sessions("title:turnstile", 10).is_ok());
    assert!(database.search_sessions("", 10).unwrap().is_empty());
}

#[test]
fn reindexes_all_jsonl_files_in_a_sessions_directory() {
    let temp = tempfile::tempdir().unwrap();
    let sessions_dir = temp.path().join("sessions");
    let nested_dir = sessions_dir.join("nested");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::create_dir_all(&nested_dir).unwrap();
    std::fs::copy(
        "tests/fixtures/session-a.jsonl",
        sessions_dir.join("session-a.jsonl"),
    )
    .unwrap();
    std::fs::copy(
        "tests/fixtures/session-malformed.jsonl",
        nested_dir.join("session-malformed.jsonl"),
    )
    .unwrap();
    std::fs::write(sessions_dir.join("notes.txt"), "not a session").unwrap();

    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let report = indexer::reindex(&database, &sessions_dir).unwrap();

    assert_eq!(report.scanned_files, 2);
    assert_eq!(report.indexed_sessions, 2);
    assert_eq!(report.malformed_records, 1);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].path.contains("session-malformed.jsonl"));
    assert_eq!(report.warnings[0].malformed_records, 1);
    assert_eq!(database.list_sessions(10).unwrap().len(), 2);
    assert!(report.failed_files.is_empty());
}

#[test]
fn reindex_missing_sessions_directory_returns_empty_report() {
    let temp = tempfile::tempdir().unwrap();
    let database = db::Database::open(&temp.path().join("index.sqlite")).unwrap();
    let report = indexer::reindex(&database, &temp.path().join("missing")).unwrap();

    assert_eq!(report.scanned_files, 0);
    assert_eq!(report.indexed_sessions, 0);
    assert_eq!(report.malformed_records, 0);
    assert!(report.failed_files.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn reindex_reports_parse_failures_and_continues() {
    let temp = tempfile::tempdir().unwrap();
    let sessions_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::copy(
        "tests/fixtures/session-a.jsonl",
        sessions_dir.join("session-a.jsonl"),
    )
    .unwrap();
    std::fs::write(
        sessions_dir.join("missing-meta.jsonl"),
        "{\"timestamp\":\"2026-07-01T12:00:00Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"no metadata here\"}]}}\n",
    )
    .unwrap();

    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let report = indexer::reindex(&database, &sessions_dir).unwrap();

    assert_eq!(report.scanned_files, 2);
    assert_eq!(report.indexed_sessions, 1);
    assert_eq!(database.list_sessions(10).unwrap().len(), 1);
    assert_eq!(report.failed_files.len(), 1);
    assert!(report.failed_files[0].contains("missing-meta.jsonl"));
    assert!(report.failed_files[0].contains("missing session id"));
}

#[test]
fn reindex_prunes_sessions_when_source_files_are_removed() {
    let temp = tempfile::tempdir().unwrap();
    let sessions_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let session_path = sessions_dir.join("session-a.jsonl");
    std::fs::copy("tests/fixtures/session-a.jsonl", &session_path).unwrap();

    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();

    let first_report = indexer::reindex(&database, &sessions_dir).unwrap();
    assert_eq!(first_report.indexed_sessions, 1);
    assert_eq!(database.list_sessions(10).unwrap().len(), 1);

    std::fs::remove_file(session_path).unwrap();

    let second_report = indexer::reindex(&database, &sessions_dir).unwrap();
    assert_eq!(second_report.scanned_files, 0);
    assert_eq!(second_report.indexed_sessions, 0);
    assert!(database.list_sessions(10).unwrap().is_empty());
}

#[test]
fn reindex_excludes_subagent_threads_from_top_level_sessions() {
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

    let database = db::Database::open(&temp.path().join("index.sqlite")).unwrap();
    let report = indexer::reindex(&database, &sessions_dir).unwrap();

    assert_eq!(report.scanned_files, 2);
    assert_eq!(report.indexed_sessions, 1);

    let sessions = database.list_sessions(10).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].session_id,
        "11111111-1111-4111-8111-111111111111"
    );
    assert!(database
        .search_sessions("review implementation", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn reindex_removes_previously_indexed_subagent_threads() {
    let temp = tempfile::tempdir().unwrap();
    let sessions_dir = temp.path().join("sessions");
    let subagent_path = sessions_dir.join("session-subagent.jsonl");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::copy("tests/fixtures/session-subagent.jsonl", &subagent_path).unwrap();

    let database = db::Database::open(&temp.path().join("index.sqlite")).unwrap();
    let parsed_subagent = codex::parse_session_file(&subagent_path).unwrap();
    database.upsert_session(&parsed_subagent).unwrap();
    assert_eq!(database.list_sessions(10).unwrap().len(), 1);

    let report = indexer::reindex(&database, &sessions_dir).unwrap();

    assert_eq!(report.scanned_files, 1);
    assert_eq!(report.indexed_sessions, 0);
    assert!(database.list_sessions(10).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn reindex_does_not_prune_after_traversal_errors() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let sessions_dir = temp.path().join("sessions");
    let nested_dir = sessions_dir.join("nested");
    std::fs::create_dir_all(&nested_dir).unwrap();
    std::fs::copy(
        "tests/fixtures/session-a.jsonl",
        nested_dir.join("session-a.jsonl"),
    )
    .unwrap();

    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let first_report = indexer::reindex(&database, &sessions_dir).unwrap();
    assert_eq!(first_report.indexed_sessions, 1);
    assert_eq!(database.list_sessions(10).unwrap().len(), 1);

    std::fs::set_permissions(&nested_dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    let second_report = indexer::reindex(&database, &sessions_dir);
    std::fs::set_permissions(&nested_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    let second_report = second_report.unwrap();
    assert!(!second_report.failed_files.is_empty());
    assert_eq!(database.list_sessions(10).unwrap().len(), 1);
}

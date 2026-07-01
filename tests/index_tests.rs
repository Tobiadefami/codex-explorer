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

    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let report = indexer::reindex(&database, &sessions_dir).unwrap();

    assert_eq!(report.scanned_files, 2);
    assert_eq!(report.indexed_sessions, 2);
    assert_eq!(report.malformed_records, 1);
    assert_eq!(database.list_sessions(10).unwrap().len(), 2);
}

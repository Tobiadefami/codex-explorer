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

use std::{path::Path, time::Duration};

use codex::ParsedSessionItemKind;

#[test]
fn reads_committed_sessions_while_an_exclusive_write_is_open() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let parsed = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();
    database.upsert_session(&parsed).unwrap();

    let writer = rusqlite::Connection::open(&db_path).unwrap();
    writer.busy_timeout(Duration::ZERO).unwrap();
    writer
        .execute_batch(
            "BEGIN EXCLUSIVE;
             UPDATE sessions SET title = 'uncommitted title' WHERE session_id =
                 '11111111-1111-4111-8111-111111111111';",
        )
        .unwrap();

    let sessions = database.list_sessions(10).unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "add turnstile to the signup form");
    writer.execute_batch("ROLLBACK;").unwrap();
}

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
    assert_eq!(
        listed[0].git_branch.as_deref(),
        Some("feature/session-browser")
    );

    let results = database.search_sessions("turnstile", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, parsed.session_id);

    let branch_results = database
        .search_sessions("feature/session-browser", 10)
        .unwrap();
    assert_eq!(branch_results.len(), 1);
    assert_eq!(branch_results[0].session_id, parsed.session_id);

    let shown = database.get_session(&parsed.session_id).unwrap().unwrap();
    assert_eq!(
        shown.summary.git_branch.as_deref(),
        Some("feature/session-browser")
    );
    assert_eq!(shown.messages.len(), 2);
    assert_eq!(shown.messages[0].text, "add turnstile to the signup form");
}

#[test]
fn stores_overview_activity_and_tool_events() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-overview-ui.jsonl")).unwrap();

    database.upsert_session(&parsed).unwrap();

    let listed = database.list_sessions(10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].last_activity_at, "2026-07-02T09:06:04Z");
    assert_eq!(
        listed[0].latest_user_message.as_deref(),
        Some("now audit how tools are used")
    );
    let shown = database.get_session(&parsed.session_id).unwrap().unwrap();
    assert_eq!(shown.messages.len(), 11);
    assert_eq!(shown.tool_events.len(), 3);
    assert_eq!(shown.tool_events[0].name, "exec_command");
    assert_eq!(
        shown.tool_events[0].call_id.as_deref(),
        Some("call_read_source")
    );
    assert_eq!(shown.tool_events[2].duration_ms, Some(364000));
}

#[test]
fn stores_tool_event_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let mut parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-overview-ui.jsonl")).unwrap();
    parsed.tool_events = vec![codex::ParsedToolEvent {
        timestamp: "2026-07-02T09:01:01Z".to_string(),
        kind: "exec_command_end".to_string(),
        name: "exec_command".to_string(),
        summary: "cargo test".to_string(),
        status: Some("failed".to_string()),
        call_id: Some("call_cmd".to_string()),
        exit_code: Some(101),
        duration_ms: Some(2500),
        cwd: Some("/work/project-e".to_string()),
    }];

    database.upsert_session(&parsed).unwrap();

    let shown = database.get_session(&parsed.session_id).unwrap().unwrap();
    assert_eq!(shown.tool_events.len(), 1);
    assert_eq!(shown.tool_events[0].call_id.as_deref(), Some("call_cmd"));
    assert_eq!(shown.tool_events[0].exit_code, Some(101));
    assert_eq!(shown.tool_events[0].duration_ms, Some(2500));
    assert_eq!(shown.tool_events[0].cwd.as_deref(), Some("/work/project-e"));
}

#[test]
fn stores_ordered_session_items_for_detail_views() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-overview-ui.jsonl")).unwrap();

    database.upsert_session(&parsed).unwrap();

    let shown = database.get_session(&parsed.session_id).unwrap().unwrap();
    assert_eq!(shown.items.len(), parsed.items.len());
    assert!(matches!(
        shown.items[0].kind,
        ParsedSessionItemKind::SessionMeta(_)
    ));
    assert!(matches!(
        shown.items[1].kind,
        ParsedSessionItemKind::Message(_)
    ));
    assert!(matches!(
        shown.items[3].kind,
        ParsedSessionItemKind::ToolEvent(_)
    ));
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
fn lists_and_searches_sessions_for_exact_cwd() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");
    let database = db::Database::open(&db_path).unwrap();
    let mut first = codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();
    first.cwd = "/work/project-a".to_string();
    database.upsert_session(&first).unwrap();

    let mut second =
        codex::parse_session_file(Path::new("tests/fixtures/session-malformed.jsonl")).unwrap();
    second.cwd = "/work/project-b".to_string();
    database.upsert_session(&second).unwrap();

    let project_a_sessions = database
        .list_sessions_for_cwd("/work/project-a", 10)
        .unwrap();
    assert_eq!(project_a_sessions.len(), 1);
    assert_eq!(project_a_sessions[0].session_id, first.session_id);

    let project_a_matches = database
        .search_sessions_for_cwd("/work/project-a", "turnstile", 10)
        .unwrap();
    assert_eq!(project_a_matches.len(), 1);
    assert_eq!(project_a_matches[0].session_id, first.session_id);

    assert!(database
        .search_sessions_for_cwd("/work/project-b", "turnstile", 10)
        .unwrap()
        .is_empty());
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
fn reindex_retains_subagent_threads_outside_top_level_sessions() {
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
    assert_eq!(report.indexed_sessions, 2);

    let sessions = database.list_sessions(10).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].session_id,
        "11111111-1111-4111-8111-111111111111"
    );
    assert_eq!(sessions[0].child_session_count, 1);
    let child = database
        .get_session("33333333-3333-4333-8333-333333333333")
        .unwrap()
        .unwrap();
    assert_eq!(
        child.summary.parent_thread_id.as_deref(),
        Some("11111111-1111-4111-8111-111111111111")
    );
    assert_eq!(child.summary.thread_source.as_deref(), Some("subagent"));
    assert_eq!(child.summary.agent_nickname.as_deref(), Some("Reviewer"));
    assert_eq!(child.summary.agent_role.as_deref(), Some("default"));
    let parent = database
        .get_session("11111111-1111-4111-8111-111111111111")
        .unwrap()
        .unwrap();
    assert_eq!(parent.child_sessions.len(), 1);
    assert_eq!(
        parent.child_sessions[0].session_id,
        "33333333-3333-4333-8333-333333333333"
    );

    assert_eq!(
        database
            .list_sessions_for_cwd("/work/project-a", 10)
            .unwrap()
            .len(),
        1
    );
    assert!(database
        .search_sessions("review implementation", 10)
        .unwrap()
        .is_empty());
    assert!(database
        .search_sessions_for_cwd("/work/project-a", "review implementation", 10)
        .unwrap()
        .is_empty());
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

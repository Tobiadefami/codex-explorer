#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/db.rs"]
mod db;

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

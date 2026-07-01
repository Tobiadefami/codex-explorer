#[path = "../src/codex.rs"]
mod codex;

use std::path::Path;

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
fn skips_malformed_records_and_keeps_valid_messages() {
    let parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-malformed.jsonl")).unwrap();

    assert_eq!(parsed.session_id, "22222222-2222-4222-8222-222222222222");
    assert_eq!(parsed.title, "debug websocket reconnect loop");
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.malformed_records, 1);
}

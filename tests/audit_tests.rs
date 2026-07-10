#[allow(dead_code)]
#[path = "../src/audit.rs"]
mod audit;
#[allow(dead_code)]
#[path = "../src/codex.rs"]
mod codex;
#[allow(dead_code)]
#[path = "../src/codex_cmd.rs"]
mod codex_cmd;
#[allow(dead_code)]
#[path = "../src/db.rs"]
mod db;

use std::path::Path;

use audit::{format_audit_input, AUDIT_INPUT_VERSION, AUDIT_PROMPT_VERSION};
use db::Database;

#[test]
fn formats_compact_audit_input_from_indexed_session_detail() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-overview-ui.jsonl")).unwrap();
    parsed.source_path = temp.path().join("session-overview-ui.jsonl");
    database.upsert_session(&parsed).unwrap();
    let detail = database.get_session(&parsed.session_id).unwrap().unwrap();

    let input = format_audit_input(&detail);

    assert!(input.contains("AUDIT_INPUT_VERSION: 1"));
    assert!(input.contains("SESSION"));
    assert!(input.contains("id: 55555555-5555-4555-8555-555555555555"));
    assert!(input.contains("cwd: /work/project-e"));
    assert!(input.contains("branch: unknown"));
    assert!(input.contains("CONVERSATION"));
    assert!(input.contains("[user] add turnstile to the signup form"));
    assert!(input.contains("[assistant] I will inspect the Worker and form code."));
    assert!(input.contains("TOOLS"));
    assert!(input.contains("- exec_command: sed -n '1,220p' src/main.rs"));
    assert!(input.contains("- task_complete: completed"));
    assert!(input.contains("OMISSIONS"));
    assert!(input.contains("raw JSONL omitted"));
    assert!(!input.contains("\"payload\""));
}

#[test]
fn stores_and_reads_fresh_audit_results() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut parsed =
        codex::parse_session_file(Path::new("tests/fixtures/session-a.jsonl")).unwrap();
    parsed.source_path = temp.path().join("session-a.jsonl");
    database.upsert_session(&parsed).unwrap();

    let result = audit::SessionAuditResult {
        status: audit::AuditStatus::Unfinished,
        gist: "Turnstile signup work began but no completion is shown.".to_string(),
        hinge: "The transcript ends after inspection without edits or verification.".to_string(),
        next: "Resume the session and implement the signup form changes.".to_string(),
        signals: vec![
            "The user asked to add Turnstile to signup.".to_string(),
            "The assistant only inspected source code.".to_string(),
        ],
    };
    let record = audit::SessionAuditRecord {
        session_id: parsed.session_id.clone(),
        source_modified_unix_seconds: parsed.modified_unix_seconds,
        audit_input_version: AUDIT_INPUT_VERSION,
        prompt_version: AUDIT_PROMPT_VERSION,
        model: "gpt-5.6-luna".to_string(),
        reasoning_effort: "low".to_string(),
        result,
        created_at: "2026-07-09T12:00:00Z".to_string(),
    };

    database.upsert_session_audit(&record).unwrap();
    let stored = database
        .get_fresh_session_audit(
            &parsed.session_id,
            parsed.modified_unix_seconds,
            AUDIT_INPUT_VERSION,
            AUDIT_PROMPT_VERSION,
            "gpt-5.6-luna",
            "low",
        )
        .unwrap()
        .unwrap();

    assert_eq!(stored, record);
    assert!(database
        .get_fresh_session_audit(
            &parsed.session_id,
            parsed.modified_unix_seconds + 1,
            AUDIT_INPUT_VERSION,
            AUDIT_PROMPT_VERSION,
            "gpt-5.6-luna",
            "low",
        )
        .unwrap()
        .is_none());
}

#[test]
fn parses_codex_audit_json_result() {
    let result = audit::parse_audit_result(
        r#"{
          "status": "blocked",
          "gist": "Tests failed after the implementation attempt.",
          "hinge": "The final cargo test command exited with code 101.",
          "next": "Inspect the failing Rust test and rerun cargo test.",
          "signals": [
            "The session edited Rust source files.",
            "cargo test failed with exit code 101."
          ]
        }"#,
    )
    .unwrap();

    assert_eq!(result.status, audit::AuditStatus::Blocked);
    assert_eq!(result.signals.len(), 2);
}

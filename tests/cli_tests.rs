use assert_cmd::Command;
use predicates::prelude::*;

fn fixture_sessions_dir() -> &'static str {
    "tests/fixtures"
}

#[test]
fn reindex_and_search_from_cli() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");

    Command::cargo_bin("cx")
        .unwrap()
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--sessions-dir",
            fixture_sessions_dir(),
            "reindex",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("indexed 2 sessions"))
        .stdout(predicate::str::contains("skipped 1 malformed records"))
        .stderr(predicate::str::contains("session-malformed.jsonl"));

    Command::cargo_bin("cx")
        .unwrap()
        .args(["--db", db_path.to_str().unwrap(), "search", "turnstile"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "11111111-1111-4111-8111-111111111111",
        ))
        .stdout(predicate::str::contains("add turnstile to the signup form"));
}

#[test]
fn show_displays_session_preview() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("index.sqlite");

    Command::cargo_bin("cx")
        .unwrap()
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "--sessions-dir",
            fixture_sessions_dir(),
            "reindex",
        ])
        .assert()
        .success();

    Command::cargo_bin("cx")
        .unwrap()
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "show",
            "11111111-1111-4111-8111-111111111111",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("/work/project-a"))
        .stdout(predicate::str::contains(
            "user: add turnstile to the signup form",
        ))
        .stdout(predicate::str::contains(
            "assistant: I will inspect the Worker and form code.",
        ));
}

# Strict TUI Keybindings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every unmodified printable key enter search text and require `Alt` for all printable-character TUI commands.

**Architecture:** Keep command dispatch in `src/tui/input.rs` and tighten each printable command arm to require exactly `KeyModifiers::ALT`; remove the empty-query condition entirely. Preserve the existing state/action APIs, non-printable navigation, `Esc`, and `Ctrl+C`, then update the two user-facing key references.

**Tech Stack:** Rust 2021, crossterm key events, ratatui, built-in Rust tests, Cargo fmt/test.

---

## File Map

- `src/tui/input.rs`: Owns key-event dispatch and its focused unit tests. Modify command guards and replace tests that encode the old empty-query behavior.
- `src/tui/render.rs`: Owns the persistent TUI help bar. Prefix printable commands with `Alt+`.
- `README.md`: Owns the complete user-facing key reference. Document the new bindings and immediate-search guarantee.

### Task 1: Lock Down Search Input with Failing Tests

**Files:**
- Modify: `src/tui/input.rs:120-226`

- [ ] **Step 1: Replace the old plain-key tests with search regression tests**

Replace the `r_requests_refresh_when_search_is_empty`, `number_keys_switch_preview_modes`, and `five_switches_to_audit_preview_mode` tests with:

```rust
fn press_character(
    database: &Database,
    state: &mut TuiState,
    character: char,
    modifiers: KeyModifiers,
) -> TuiAction {
    handle_key(
        database,
        state,
        KeyEvent::new(KeyCode::Char(character), modifiers),
    )
    .unwrap()
}

#[test]
fn plain_printable_keys_always_append_to_search() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();

    for character in "apple".chars() {
        assert!(matches!(
            press_character(&database, &mut state, character, KeyModifiers::NONE),
            TuiAction::Continue
        ));
    }

    assert_eq!(state.query(), "apple");
}

#[test]
fn plain_digit_starts_a_search_instead_of_switching_preview() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();

    let action = press_character(&database, &mut state, '4', KeyModifiers::NONE);

    assert!(matches!(action, TuiAction::Continue));
    assert_eq!(state.query(), "4");
    assert_eq!(state.preview_mode(), PreviewMode::Overview);
}

#[test]
fn plain_q_is_search_input_instead_of_quit() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();

    let action = press_character(&database, &mut state, 'q', KeyModifiers::NONE);

    assert!(matches!(action, TuiAction::Continue));
    assert_eq!(state.query(), "q");
}
```

- [ ] **Step 2: Run the focused tests and verify the old behavior fails**

Run:

```bash
cargo test plain_printable_keys_always_append_to_search
cargo test plain_digit_starts_a_search_instead_of_switching_preview
cargo test plain_q_is_search_input_instead_of_quit
```

Expected: all three tests FAIL because `a`, `4`, and `q` still invoke commands when the query is empty.

- [ ] **Step 3: Commit the failing regression tests**

```bash
git add src/tui/input.rs
git commit -m "test: cover printable TUI search input"
```

### Task 2: Move Printable Commands Behind Alt

**Files:**
- Modify: `src/tui/input.rs:53-116`
- Modify: `src/tui/input.rs:120-226`

- [ ] **Step 1: Add Alt-command tests before changing dispatch**

Update the existing expansion and audit tests to pass `KeyModifiers::ALT`, rename them to `alt_e_toggles_selected_row_expansion` and `alt_i_requests_audit_for_selected_session`, and add:

```rust
#[test]
fn alt_r_requests_refresh_even_when_search_is_not_empty() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();
    state.set_query(&database, "active search".to_string()).unwrap();

    let action = press_character(&database, &mut state, 'r', KeyModifiers::ALT);

    assert!(matches!(action, TuiAction::Refresh));
    assert_eq!(state.query(), "active search");
}

#[test]
fn alt_number_keys_switch_every_preview_mode() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();
    let cases = [
        ('1', PreviewMode::Overview),
        ('2', PreviewMode::Conversation),
        ('3', PreviewMode::Tools),
        ('4', PreviewMode::Timeline),
        ('5', PreviewMode::Audit),
    ];

    for (key, expected_mode) in cases {
        let action = press_character(&database, &mut state, key, KeyModifiers::ALT);
        assert!(matches!(action, TuiAction::Continue));
        assert_eq!(state.preview_mode(), expected_mode);
    }
}

#[test]
fn alt_navigation_commands_move_selection_and_preview() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();

    press_character(&database, &mut state, 'd', KeyModifiers::ALT);
    assert_eq!(state.preview_scroll(), 1);
    press_character(&database, &mut state, 'u', KeyModifiers::ALT);
    assert_eq!(state.preview_scroll(), 0);

    assert!(matches!(
        press_character(&database, &mut state, 'j', KeyModifiers::ALT),
        TuiAction::Continue
    ));
    assert!(matches!(
        press_character(&database, &mut state, 'k', KeyModifiers::ALT),
        TuiAction::Continue
    ));
}

#[test]
fn alt_q_quits() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();

    let action = press_character(&database, &mut state, 'q', KeyModifiers::ALT);

    assert!(matches!(action, TuiAction::Quit));
}

#[test]
fn alt_a_and_alt_p_switch_project_scope() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut parsed = crate::codex::parse_session_file(std::path::Path::new(
        "tests/fixtures/session-a.jsonl",
    ))
    .unwrap();
    parsed.cwd = temp.path().display().to_string();
    parsed.source_path = temp.path().join("session-a.jsonl");
    database.upsert_session(&parsed).unwrap();
    let mut state = TuiState::load_scoped(&database, temp.path().to_path_buf(), 20).unwrap();

    press_character(&database, &mut state, 'a', KeyModifiers::ALT);
    assert_eq!(state.scope(), &super::super::state::ProjectScope::AllProjects);

    press_character(&database, &mut state, 'p', KeyModifiers::ALT);
    assert_eq!(
        state.scope(),
        &super::super::state::ProjectScope::CurrentDirectory
    );
}

#[test]
fn non_alt_quit_shortcuts_remain_available() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
    let mut state = TuiState::load(&database, 20).unwrap();

    let escape = handle_key(
        &database,
        &mut state,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
    )
    .unwrap();
    let control_c = press_character(&database, &mut state, 'c', KeyModifiers::CONTROL);

    assert!(matches!(escape, TuiAction::Quit));
    assert!(matches!(control_c, TuiAction::Quit));
}
```

In `alt_e_toggles_selected_row_expansion`, change the key event to:

```rust
KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
```

In `alt_i_requests_audit_for_selected_session`, change the key event to:

```rust
KeyEvent::new(KeyCode::Char('i'), KeyModifiers::ALT)
```

- [ ] **Step 2: Run the Alt tests and verify they fail**

Run:

```bash
cargo test alt_
```

Expected: FAIL because the current handler ignores Alt-modified application commands.

- [ ] **Step 3: Require Alt for every printable application command**

In `handle_key`, keep the non-printable arms unchanged and replace the character-command block from `Ctrl+C` through the fallback character arm with:

```rust
KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
    Ok(TuiAction::Quit)
}
KeyCode::Char('q') if key_event.modifiers == KeyModifiers::ALT => Ok(TuiAction::Quit),
KeyCode::Char('a') if key_event.modifiers == KeyModifiers::ALT => {
    state.show_all_projects(database)?;
    Ok(TuiAction::Continue)
}
KeyCode::Char('p') if key_event.modifiers == KeyModifiers::ALT => {
    state.show_current_directory(database)?;
    Ok(TuiAction::Continue)
}
KeyCode::Char('r') if key_event.modifiers == KeyModifiers::ALT => Ok(TuiAction::Refresh),
KeyCode::Char('i') if key_event.modifiers == KeyModifiers::ALT => Ok(state
    .selected_session_id()
    .map(|session_id| TuiAction::Audit(session_id.to_string()))
    .unwrap_or(TuiAction::Continue)),
KeyCode::Char('e') if key_event.modifiers == KeyModifiers::ALT => {
    state.toggle_selected_expansion();
    Ok(TuiAction::Continue)
}
KeyCode::Char('1') if key_event.modifiers == KeyModifiers::ALT => {
    state.set_preview_mode(PreviewMode::Overview);
    Ok(TuiAction::Continue)
}
KeyCode::Char('2') if key_event.modifiers == KeyModifiers::ALT => {
    state.set_preview_mode(PreviewMode::Conversation);
    Ok(TuiAction::Continue)
}
KeyCode::Char('3') if key_event.modifiers == KeyModifiers::ALT => {
    state.set_preview_mode(PreviewMode::Tools);
    Ok(TuiAction::Continue)
}
KeyCode::Char('4') if key_event.modifiers == KeyModifiers::ALT => {
    state.set_preview_mode(PreviewMode::Timeline);
    Ok(TuiAction::Continue)
}
KeyCode::Char('5') if key_event.modifiers == KeyModifiers::ALT => {
    state.set_preview_mode(PreviewMode::Audit);
    Ok(TuiAction::Continue)
}
KeyCode::Char('d') if key_event.modifiers == KeyModifiers::ALT => {
    state.scroll_preview_down();
    Ok(TuiAction::Continue)
}
KeyCode::Char('u') if key_event.modifiers == KeyModifiers::ALT => {
    state.scroll_preview_up();
    Ok(TuiAction::Continue)
}
KeyCode::Char('j') if key_event.modifiers == KeyModifiers::ALT => {
    state.move_down();
    Ok(TuiAction::Continue)
}
KeyCode::Char('k') if key_event.modifiers == KeyModifiers::ALT => {
    state.move_up();
    Ok(TuiAction::Continue)
}
KeyCode::Char(character) if key_event.modifiers == KeyModifiers::NONE => {
    let mut query = state.query().to_string();
    query.push(character);
    state.set_query(database, query)?;
    Ok(TuiAction::Continue)
}
_ => Ok(TuiAction::Continue),
```

Exact equality makes the bindings strict: extra modifiers do not accidentally trigger a command.

- [ ] **Step 4: Run focused input tests**

Run:

```bash
cargo test tui::input::tests
```

Expected: all input tests PASS.

- [ ] **Step 5: Commit the input behavior**

```bash
git add src/tui/input.rs
git commit -m "fix: require Alt for TUI commands"
```

### Task 3: Update the Visible Key References

**Files:**
- Modify: `src/tui/render.rs:217-223`
- Modify: `README.md:57-75`

- [ ] **Step 1: Update the persistent help bar**

Replace the help string in `render_help` with:

```rust
let help = Paragraph::new(
    "Type search | Alt+I audit | Alt+E expand | Alt+R reindex | Alt+1-5 previews | Alt+A all | Alt+P project | Enter resume | Esc quit",
)
.style(secondary_style());
```

- [ ] **Step 2: Update the README key list**

Replace the TUI key bullets with:

```markdown
- unmodified printable keys type into search, including when the search box is empty
- `Backspace` edits search
- `Up`/`Down` moves selection
- `Alt+A` shows all projects
- `Alt+P` returns to the current directory
- `Alt+R` refreshes the index
- `Alt+E` expands or collapses the selected row
- `Alt+I` runs or refreshes an audit for the selected session
- `Alt+1` shows the overview preview
- `Alt+2` shows the conversation preview
- `Alt+3` shows tool activity
- `Alt+4` shows the timeline
- `Alt+5` shows the audit preview
- `Alt+J`/`Alt+K` also moves selection
- `Alt+D`/`Alt+U` scrolls the preview one line
- `PgUp`/`PgDn` scrolls the preview by a page
- `Enter` resumes the selected session
- `Esc`, `Ctrl+C`, or `Alt+Q` quits
```

- [ ] **Step 3: Check formatting and stale plain-key documentation**

Run:

```bash
cargo fmt --check
rg -n '(^|[^+`])(a|p|r|e|i|[1-5]|q) (shows|returns|refreshes|runs|quits)|Type search \| i run audit' README.md src/tui/render.rs
```

Expected: formatting succeeds and `rg` returns no matches.

- [ ] **Step 4: Run the full test suite**

Run:

```bash
cargo test
```

Expected: all unit and integration tests PASS with zero failures.

- [ ] **Step 5: Inspect the final diff**

Run:

```bash
git diff --check
git diff --stat 13f045a
```

Expected: `git diff --check` produces no output; aside from this tracked plan, the stat includes only `src/tui/input.rs`, `src/tui/render.rs`, and `README.md`.

- [ ] **Step 6: Commit the documentation updates**

```bash
git add README.md src/tui/render.rs
git commit -m "docs: update TUI Alt shortcuts"
```

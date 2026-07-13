use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::db::Database;

use super::state::TuiState;

pub(super) enum TuiAction {
    Continue,
    Quit,
    Refresh,
    Audit(String),
    Resume(String),
}

pub(super) fn handle_key(
    database: &Database,
    state: &mut TuiState,
    key_event: KeyEvent,
) -> Result<TuiAction> {
    match key_event.code {
        KeyCode::Esc => Ok(TuiAction::Quit),
        KeyCode::Enter => Ok(state
            .selected_session_id()
            .map(|session_id| TuiAction::Resume(session_id.to_string()))
            .unwrap_or(TuiAction::Continue)),
        KeyCode::Up => {
            state.move_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Down => {
            state.move_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::PageDown => {
            state.scroll_preview_page_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::PageUp => {
            state.scroll_preview_page_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Home => {
            state.scroll_preview_top();
            Ok(TuiAction::Continue)
        }
        KeyCode::Tab if key_event.modifiers == KeyModifiers::NONE => {
            state.select_next_preview();
            Ok(TuiAction::Continue)
        }
        KeyCode::BackTab if key_event.modifiers == KeyModifiers::SHIFT => {
            state.select_previous_preview();
            Ok(TuiAction::Continue)
        }
        KeyCode::Backspace => {
            let mut query = state.query().to_string();
            query.pop();
            state.set_query(database, query)?;
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => Ok(TuiAction::Quit),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::{PreviewMode, ProjectScope};

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

    #[test]
    fn alt_e_toggles_selected_row_expansion() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut parsed = crate::codex::parse_session_file(std::path::Path::new(
            "tests/fixtures/session-a.jsonl",
        ))
        .unwrap();
        parsed.source_path = temp.path().join("session-a.jsonl");
        database.upsert_session(&parsed).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();

        let action = press_character(&database, &mut state, 'e', KeyModifiers::ALT);

        assert!(matches!(action, TuiAction::Continue));
        assert_eq!(
            state.expanded_session_id(),
            Some(parsed.session_id.as_str())
        );
    }

    #[test]
    fn alt_i_requests_audit_for_selected_session() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut parsed = crate::codex::parse_session_file(std::path::Path::new(
            "tests/fixtures/session-a.jsonl",
        ))
        .unwrap();
        parsed.source_path = temp.path().join("session-a.jsonl");
        database.upsert_session(&parsed).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();

        let action = press_character(&database, &mut state, 'i', KeyModifiers::ALT);

        assert!(matches!(
            action,
            TuiAction::Audit(ref session_id)
                if session_id == "11111111-1111-4111-8111-111111111111"
        ));
    }

    #[test]
    fn alt_r_requests_refresh_even_when_search_is_not_empty() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();
        state
            .set_query(&database, "active search".to_string())
            .unwrap();

        let action = press_character(&database, &mut state, 'r', KeyModifiers::ALT);

        assert!(matches!(action, TuiAction::Refresh));
        assert_eq!(state.query(), "active search");
    }

    #[test]
    fn tab_cycles_forward_through_preview_modes_and_resets_scroll() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();
        let expected_modes = [
            PreviewMode::Conversation,
            PreviewMode::Tools,
            PreviewMode::Timeline,
            PreviewMode::Audit,
            PreviewMode::Overview,
        ];

        for expected_mode in expected_modes {
            state.scroll_preview_down();
            let action = handle_key(
                &database,
                &mut state,
                KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            )
            .unwrap();

            assert!(matches!(action, TuiAction::Continue));
            assert_eq!(state.preview_mode(), expected_mode);
            assert_eq!(state.preview_scroll(), 0);
        }
    }

    #[test]
    fn backtab_cycles_backward_through_preview_modes_and_resets_scroll() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();
        let expected_modes = [
            PreviewMode::Audit,
            PreviewMode::Timeline,
            PreviewMode::Tools,
            PreviewMode::Conversation,
            PreviewMode::Overview,
        ];

        for expected_mode in expected_modes {
            state.scroll_preview_down();
            let action = handle_key(
                &database,
                &mut state,
                KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            )
            .unwrap();

            assert!(matches!(action, TuiAction::Continue));
            assert_eq!(state.preview_mode(), expected_mode);
            assert_eq!(state.preview_scroll(), 0);
        }
    }

    #[test]
    fn alt_navigation_commands_move_selection_and_preview() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut first_session = crate::codex::parse_session_file(std::path::Path::new(
            "tests/fixtures/session-a.jsonl",
        ))
        .unwrap();
        first_session.source_path = temp.path().join("session-a.jsonl");
        database.upsert_session(&first_session).unwrap();
        let mut second_session = crate::codex::parse_session_file(std::path::Path::new(
            "tests/fixtures/session-overview-ui.jsonl",
        ))
        .unwrap();
        second_session.source_path = temp.path().join("session-overview-ui.jsonl");
        database.upsert_session(&second_session).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();

        let down_action = press_character(&database, &mut state, 'd', KeyModifiers::ALT);
        assert!(matches!(down_action, TuiAction::Continue));
        assert_eq!(state.preview_scroll(), 1);

        let up_action = press_character(&database, &mut state, 'u', KeyModifiers::ALT);
        assert!(matches!(up_action, TuiAction::Continue));
        assert_eq!(state.preview_scroll(), 0);

        assert_eq!(state.selected_index(), Some(0));
        let next_session_action = press_character(&database, &mut state, 'j', KeyModifiers::ALT);
        assert!(matches!(next_session_action, TuiAction::Continue));
        assert_eq!(state.selected_index(), Some(1));

        let previous_session_action =
            press_character(&database, &mut state, 'k', KeyModifiers::ALT);
        assert!(matches!(previous_session_action, TuiAction::Continue));
        assert_eq!(state.selected_index(), Some(0));
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

        let all_projects_action = press_character(&database, &mut state, 'a', KeyModifiers::ALT);
        assert!(matches!(all_projects_action, TuiAction::Continue));
        assert_eq!(state.scope(), &ProjectScope::AllProjects);

        let current_directory_action =
            press_character(&database, &mut state, 'p', KeyModifiers::ALT);
        assert!(matches!(current_directory_action, TuiAction::Continue));
        assert_eq!(state.scope(), &ProjectScope::CurrentDirectory);
    }

    #[test]
    fn non_alt_quit_shortcuts_remain_available() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();

        let escape_action = handle_key(
            &database,
            &mut state,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        )
        .unwrap();
        assert!(matches!(escape_action, TuiAction::Quit));

        let control_c_action = press_character(&database, &mut state, 'c', KeyModifiers::CONTROL);
        assert!(matches!(control_c_action, TuiAction::Quit));
    }
}

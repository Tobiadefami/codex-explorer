use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::db::Database;

use super::state::{PreviewMode, TuiState};

pub(super) enum TuiAction {
    Continue,
    Quit,
    Refresh,
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
        KeyCode::Backspace => {
            let mut query = state.query().to_string();
            query.pop();
            state.set_query(database, query)?;
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            Ok(TuiAction::Quit)
        }
        KeyCode::Char('q') if state.query().is_empty() => Ok(TuiAction::Quit),
        KeyCode::Char('a') if state.query().is_empty() => {
            state.show_all_projects(database)?;
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('p') if state.query().is_empty() => {
            state.show_current_directory(database)?;
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('r') if state.query().is_empty() => Ok(TuiAction::Refresh),
        KeyCode::Char('e') if state.query().is_empty() => {
            state.toggle_selected_expansion();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('1') if state.query().is_empty() => {
            state.set_preview_mode(PreviewMode::Overview);
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('2') if state.query().is_empty() => {
            state.set_preview_mode(PreviewMode::Conversation);
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('3') if state.query().is_empty() => {
            state.set_preview_mode(PreviewMode::Tools);
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('4') if state.query().is_empty() => {
            state.set_preview_mode(PreviewMode::Timeline);
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('d') if state.query().is_empty() => {
            state.scroll_preview_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('u') if state.query().is_empty() => {
            state.scroll_preview_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('j') if state.query().is_empty() => {
            state.move_down();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char('k') if state.query().is_empty() => {
            state.move_up();
            Ok(TuiAction::Continue)
        }
        KeyCode::Char(character) if key_event.modifiers.is_empty() => {
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

    #[test]
    fn r_requests_refresh_when_search_is_empty() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();

        let action = handle_key(
            &database,
            &mut state,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        )
        .unwrap();

        assert!(matches!(action, TuiAction::Refresh));
        assert_eq!(state.query(), "");
    }

    #[test]
    fn number_keys_switch_preview_modes() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();

        let action = handle_key(
            &database,
            &mut state,
            KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE),
        )
        .unwrap();

        assert!(matches!(action, TuiAction::Continue));
        assert_eq!(state.preview_mode(), PreviewMode::Timeline);
    }

    #[test]
    fn e_toggles_selected_row_expansion_when_search_is_empty() {
        let temp = tempfile::tempdir().unwrap();
        let database = Database::open(&temp.path().join("index.sqlite")).unwrap();
        let mut parsed = crate::codex::parse_session_file(std::path::Path::new(
            "tests/fixtures/session-a.jsonl",
        ))
        .unwrap();
        parsed.source_path = temp.path().join("session-a.jsonl");
        database.upsert_session(&parsed).unwrap();
        let mut state = TuiState::load(&database, 20).unwrap();

        let action = handle_key(
            &database,
            &mut state,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        )
        .unwrap();

        assert!(matches!(action, TuiAction::Continue));
        assert_eq!(
            state.expanded_session_id(),
            Some(parsed.session_id.as_str())
        );
    }
}

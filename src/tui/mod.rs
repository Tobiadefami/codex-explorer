pub mod format;
mod input;
pub mod refresh;
mod render;
pub mod state;
mod terminal;

use std::{io, path::PathBuf, time::Duration};

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::db::{Database, SessionDetail};

use input::{handle_key, TuiAction};
use refresh::{apply_refresh_results, start_refresh, RefreshStatus};
use render::render;
use state::TuiState;
use terminal::TerminalGuard;

const TUI_RESULT_LIMIT: usize = 50;
const REFRESH_TICK: Duration = Duration::from_millis(120);

pub fn run(
    database: &Database,
    db_path: PathBuf,
    sessions_dir: PathBuf,
    current_dir: PathBuf,
) -> Result<Option<String>> {
    let _terminal_guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut state = TuiState::load_scoped(database, current_dir, TUI_RESULT_LIMIT)?;
    let mut refresh_results = start_refresh(db_path.clone(), sessions_dir.clone());
    let mut refresh_status = RefreshStatus::running();

    loop {
        apply_refresh_results(database, &mut state, &mut refresh_status, &refresh_results)?;
        let preview = selected_preview(database, &state)?;
        terminal.draw(|frame| render(frame, &state, &refresh_status, preview.as_ref()))?;

        if event::poll(REFRESH_TICK)? {
            let Event::Key(key_event) = event::read()? else {
                continue;
            };
            match handle_key(database, &mut state, key_event)? {
                TuiAction::Continue => {}
                TuiAction::Quit => return Ok(None),
                TuiAction::Refresh => {
                    if !refresh_status.is_running() {
                        refresh_results = start_refresh(db_path.clone(), sessions_dir.clone());
                        refresh_status = RefreshStatus::running();
                    }
                }
                TuiAction::Resume(session_id) => return Ok(Some(session_id)),
            }
        } else {
            refresh_status.tick();
        }
    }
}

fn selected_preview(database: &Database, state: &TuiState) -> Result<Option<SessionDetail>> {
    let Some(session_id) = state.selected_session_id() else {
        return Ok(None);
    };
    database.get_session(session_id)
}

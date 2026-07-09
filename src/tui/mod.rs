pub mod format;
mod input;
pub mod refresh;
mod render;
pub mod state;
mod terminal;

use std::{
    io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    audit::{
        self, SessionAuditRecord, AUDIT_INPUT_VERSION, AUDIT_PROMPT_VERSION,
        DEFAULT_MODEL as DEFAULT_AUDIT_MODEL,
        DEFAULT_REASONING_EFFORT as DEFAULT_AUDIT_REASONING_EFFORT,
    },
    db::{Database, SessionDetail},
};

use input::{handle_key, TuiAction};
use refresh::{apply_refresh_results, start_refresh, RefreshStatus};
use render::render;
use state::TuiState;
use terminal::TerminalGuard;

const TUI_RESULT_LIMIT: usize = 50;
const REFRESH_TICK: Duration = Duration::from_millis(120);

pub enum TuiExit {
    Resume(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AuditRunStatus {
    Idle,
    Running {
        session_id: String,
        spinner_index: usize,
    },
    Complete {
        session_id: String,
    },
    Failed {
        session_id: String,
        message: String,
    },
}

impl AuditRunStatus {
    fn tick(&mut self) {
        if let Self::Running { spinner_index, .. } = self {
            *spinner_index = (*spinner_index + 1) % AUDIT_SPINNER.len();
        }
    }

    fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    pub(super) fn label_for(&self, session_id: &str) -> Option<String> {
        match self {
            Self::Idle => None,
            Self::Running {
                session_id: running_session_id,
                spinner_index,
            } if running_session_id == session_id => {
                Some(format!("Audit running {}", AUDIT_SPINNER[*spinner_index]))
            }
            Self::Complete {
                session_id: completed_session_id,
            } if completed_session_id == session_id => Some("Audit complete".to_string()),
            Self::Failed {
                session_id: failed_session_id,
                message,
            } if failed_session_id == session_id => Some(format!("Audit failed: {message}")),
            _ => None,
        }
    }
}

const AUDIT_SPINNER: &[&str] = &["|", "/", "-", "\\"];
type AuditResult = std::result::Result<SessionAuditRecord, String>;

pub fn run(
    database: &Database,
    db_path: PathBuf,
    sessions_dir: PathBuf,
    current_dir: PathBuf,
) -> Result<Option<TuiExit>> {
    let _terminal_guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut state = TuiState::load_scoped(database, current_dir, TUI_RESULT_LIMIT)?;
    let mut refresh_results = start_refresh(db_path.clone(), sessions_dir.clone());
    let mut refresh_status = RefreshStatus::running();
    let mut audit_results: Option<Receiver<AuditResult>> = None;
    let mut audit_status = AuditRunStatus::Idle;

    loop {
        apply_refresh_results(database, &mut state, &mut refresh_status, &refresh_results)?;
        apply_audit_results(&mut audit_status, &mut audit_results);
        let preview = selected_preview(database, &state)?;
        let audit = selected_audit(database, preview.as_ref())?;
        terminal.draw(|frame| {
            render(
                frame,
                &state,
                &refresh_status,
                preview.as_ref(),
                audit.as_ref(),
                &audit_status,
            )
        })?;

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
                TuiAction::Audit(session_id) => {
                    state.set_preview_mode(state::PreviewMode::Audit);
                    if !audit_status.is_running() {
                        audit_results = Some(start_audit(db_path.clone(), session_id.clone()));
                        audit_status = AuditRunStatus::Running {
                            session_id,
                            spinner_index: 0,
                        };
                    }
                }
                TuiAction::Resume(session_id) => return Ok(Some(TuiExit::Resume(session_id))),
            }
        } else {
            refresh_status.tick();
            audit_status.tick();
        }
    }
}

fn selected_preview(database: &Database, state: &TuiState) -> Result<Option<SessionDetail>> {
    let Some(session_id) = state.selected_session_id() else {
        return Ok(None);
    };
    database.get_session(session_id)
}

fn selected_audit(
    database: &Database,
    detail: Option<&SessionDetail>,
) -> Result<Option<SessionAuditRecord>> {
    let Some(detail) = detail else {
        return Ok(None);
    };

    database.get_fresh_session_audit(
        &detail.summary.session_id,
        audit::source_modified_unix_seconds(detail),
        AUDIT_INPUT_VERSION,
        AUDIT_PROMPT_VERSION,
        DEFAULT_AUDIT_MODEL,
        DEFAULT_AUDIT_REASONING_EFFORT,
    )
}

fn start_audit(db_path: PathBuf, session_id: String) -> Receiver<AuditResult> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = run_audit(&db_path, &session_id).map_err(|error| format!("{error:#}"));
        let _ = sender.send(result);
    });
    receiver
}

fn run_audit(db_path: &Path, session_id: &str) -> Result<SessionAuditRecord> {
    let database = Database::open(db_path)?;
    let detail = database
        .get_session(session_id)?
        .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
    let record = audit::audit_session(
        &database,
        &detail,
        true,
        DEFAULT_AUDIT_MODEL,
        DEFAULT_AUDIT_REASONING_EFFORT,
    )?;
    Ok(record)
}

fn apply_audit_results(
    audit_status: &mut AuditRunStatus,
    audit_results: &mut Option<Receiver<AuditResult>>,
) {
    let Some(receiver) = audit_results else {
        return;
    };

    let Ok(result) = receiver.try_recv() else {
        return;
    };

    match result {
        Ok(record) => {
            *audit_status = AuditRunStatus::Complete {
                session_id: record.session_id,
            };
        }
        Err(message) => {
            let session_id = match audit_status {
                AuditRunStatus::Running { session_id, .. } => session_id.clone(),
                AuditRunStatus::Failed { session_id, .. } => session_id.clone(),
                AuditRunStatus::Complete { session_id } => session_id.clone(),
                AuditRunStatus::Idle => String::new(),
            };
            *audit_status = AuditRunStatus::Failed {
                session_id,
                message: refresh::short_error_message(&message),
            };
        }
    }
    *audit_results = None;
}

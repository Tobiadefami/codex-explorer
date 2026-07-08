use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};

use anyhow::Result;

use crate::{
    db::Database,
    indexer::{self, ReindexReport},
};

use super::{format::trim_to_chars, state::TuiState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshStatus {
    Running {
        spinner_index: usize,
    },
    Complete {
        scanned_files: usize,
        indexed_sessions: usize,
    },
    Failed {
        message: String,
    },
}

impl RefreshStatus {
    pub fn running() -> Self {
        Self::Running { spinner_index: 0 }
    }

    pub fn complete(scanned_files: usize, indexed_sessions: usize) -> Self {
        Self::Complete {
            scanned_files,
            indexed_sessions,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    pub fn tick(&mut self) {
        if let Self::Running { spinner_index } = self {
            *spinner_index = (*spinner_index + 1) % REFRESH_SPINNER.len();
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Running { spinner_index } => {
                format!("Refreshing {}", REFRESH_SPINNER[*spinner_index])
            }
            Self::Complete {
                scanned_files,
                indexed_sessions,
            } => format!("Refreshed {indexed_sessions} sessions from {scanned_files} files"),
            Self::Failed { message } => format!("Refresh failed: {message}"),
        }
    }
}

const REFRESH_SPINNER: &[&str] = &["|", "/", "-", "\\"];

type RefreshResult = std::result::Result<ReindexReport, String>;

pub(super) fn start_refresh(db_path: PathBuf, sessions_dir: PathBuf) -> Receiver<RefreshResult> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = Database::open(&db_path)
            .and_then(|database| indexer::reindex(&database, &sessions_dir))
            .map_err(|error| format!("{error:#}"));
        let _ = sender.send(result);
    });
    receiver
}

pub(super) fn apply_refresh_results(
    database: &Database,
    state: &mut TuiState,
    refresh_status: &mut RefreshStatus,
    refresh_results: &Receiver<RefreshResult>,
) -> Result<()> {
    while let Ok(result) = refresh_results.try_recv() {
        match result {
            Ok(report) => {
                *refresh_status =
                    RefreshStatus::complete(report.scanned_files, report.indexed_sessions);
                state.reload(database)?;
            }
            Err(message) => {
                *refresh_status = RefreshStatus::failed(short_error_message(&message));
            }
        }
    }

    Ok(())
}

fn short_error_message(message: &str) -> String {
    trim_to_chars(message, 96)
}

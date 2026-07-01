use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::{codex, db::Database};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReindexReport {
    pub scanned_files: usize,
    pub indexed_sessions: usize,
    pub malformed_records: usize,
    pub failed_files: Vec<String>,
}

pub fn reindex(database: &Database, sessions_dir: &Path) -> Result<ReindexReport> {
    let mut report = ReindexReport::default();

    if !sessions_dir.exists() {
        return Ok(report);
    }

    for entry in WalkDir::new(sessions_dir) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report.failed_files.push(error.to_string());
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }

        report.scanned_files += 1;

        match codex::parse_session_file(path) {
            Ok(session) => {
                report.malformed_records += session.malformed_records;
                database
                    .upsert_session(&session)
                    .with_context(|| format!("index session file {}", path.display()))?;
                report.indexed_sessions += 1;
            }
            Err(error) => {
                report
                    .failed_files
                    .push(format!("{}: {error:#}", path.display()));
            }
        }
    }

    Ok(report)
}

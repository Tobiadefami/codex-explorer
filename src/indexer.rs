use std::{collections::HashSet, path::Path};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::{codex, db::Database};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReindexWarning {
    pub path: String,
    pub malformed_records: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReindexReport {
    pub scanned_files: usize,
    pub indexed_sessions: usize,
    pub malformed_records: usize,
    pub failed_files: Vec<String>,
    pub warnings: Vec<ReindexWarning>,
}

pub fn reindex(database: &Database, sessions_dir: &Path) -> Result<ReindexReport> {
    let mut report = ReindexReport::default();
    let mut seen_source_paths = HashSet::new();
    let mut traversal_failed = false;

    if !sessions_dir.exists() {
        return Ok(report);
    }

    for entry in WalkDir::new(sessions_dir) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                traversal_failed = true;
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
        seen_source_paths.insert(path.display().to_string());

        match codex::parse_session_file(path) {
            Ok(session) => {
                if session.is_subagent_thread() {
                    database
                        .delete_source_path(path)
                        .with_context(|| format!("remove subagent session {}", path.display()))?;
                    continue;
                }
                if session.malformed_records > 0 {
                    report.warnings.push(ReindexWarning {
                        path: path.display().to_string(),
                        malformed_records: session.malformed_records,
                    });
                }
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

    if !traversal_failed {
        database
            .prune_missing_source_paths(sessions_dir, &seen_source_paths)
            .with_context(|| format!("prune stale sessions under {}", sessions_dir.display()))?;
    }

    Ok(report)
}

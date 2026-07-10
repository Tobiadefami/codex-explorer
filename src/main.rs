mod audit;
mod cli;
mod codex;
mod codex_cmd;
mod db;
mod indexer;
mod tui;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use crate::audit::{format_audit_input, SessionAuditRecord};
use crate::cli::{Cli, Commands};
use crate::db::{Database, SessionDetail, SessionSummary};
use crate::tui::TuiExit;

fn main() -> Result<()> {
    let Cli {
        db,
        sessions_dir,
        command,
    } = Cli::parse();

    match command {
        Some(Commands::Reindex) => {
            let database = open_database(db)?;
            let sessions_dir = resolve_sessions_dir(sessions_dir)?;
            let report = indexer::reindex(&database, &sessions_dir)?;
            println!(
                "scanned {} files, indexed {} sessions, skipped {} malformed records",
                report.scanned_files, report.indexed_sessions, report.malformed_records
            );
            for warning in report.warnings {
                eprintln!(
                    "warning: skipped {} malformed records in {}",
                    warning.malformed_records, warning.path
                );
            }
            let failed_count = report.failed_files.len();
            if failed_count > 0 {
                eprintln!("failed files:");
                for failure in &report.failed_files {
                    eprintln!("  {failure}");
                }
                anyhow::bail!("failed to index {failed_count} session files");
            }
        }
        Some(Commands::List { limit }) => {
            let database = open_database(db)?;
            print_summaries(database.list_sessions(limit)?);
        }
        Some(Commands::Search { query, limit }) => {
            let database = open_database(db)?;
            print_summaries(database.search_sessions(&query, limit)?);
        }
        Some(Commands::Show { session_id }) => {
            let database = open_database(db)?;
            match database.get_session(&session_id)? {
                Some(detail) => {
                    println!("{}  {}", detail.summary.session_id, detail.summary.title);
                    println!("cwd: {}", detail.summary.cwd);
                    println!("started: {}", detail.summary.started_at);
                    println!("source: {}", detail.summary.source_path);
                    println!();
                    for message in detail.messages {
                        println!("{}: {}", message.role, message.text.replace('\n', " "));
                    }
                }
                None => {
                    anyhow::bail!("session not found: {session_id}");
                }
            }
        }
        Some(Commands::Audit {
            refresh,
            print_input,
            model,
            reasoning_effort,
            session_id,
        }) => {
            let database = open_database(db)?;
            let detail = get_existing_session(&database, &session_id)?;
            if print_input {
                println!("{}", format_audit_input(&detail));
                return Ok(());
            }

            audit_session(&database, &session_id, refresh, &model, &reasoning_effort)?;
        }
        Some(Commands::Resume { session_id }) => {
            let command = codex_cmd::resume_command(&session_id);
            let code = codex_cmd::run(command)?;
            std::process::exit(code);
        }
        None => {
            let db_path = resolve_db_path(db)?;
            let sessions_dir = resolve_sessions_dir(sessions_dir)?;
            let current_dir =
                std::env::current_dir().context("could not resolve current directory")?;
            let database = Database::open(&db_path)?;
            if let Some(exit) = tui::run(&database, db_path, sessions_dir, current_dir)? {
                match exit {
                    TuiExit::Resume(session_id) => {
                        let command = codex_cmd::resume_command(&session_id);
                        let code = codex_cmd::run(command)?;
                        std::process::exit(code);
                    }
                }
            }
        }
    }

    Ok(())
}

fn get_existing_session(database: &Database, session_id: &str) -> Result<SessionDetail> {
    database
        .get_session(session_id)?
        .with_context(|| format!("session not found: {session_id}"))
}

fn audit_session(
    database: &Database,
    session_id: &str,
    refresh: bool,
    model: &str,
    reasoning_effort: &str,
) -> Result<()> {
    let detail = get_existing_session(database, session_id)?;
    let audit = audit::audit_session(database, &detail, refresh, model, reasoning_effort)?;
    print_audit(&audit);
    Ok(())
}

fn open_database(explicit_db_path: Option<PathBuf>) -> Result<Database> {
    let db_path = resolve_db_path(explicit_db_path)?;
    Database::open(&db_path)
}

fn resolve_db_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    let data_dir = dirs::data_dir().context("could not resolve local data directory")?;
    Ok(data_dir.join("cx").join("index.sqlite"))
}

fn resolve_sessions_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    let home = dirs::home_dir().context("could not resolve home directory")?;
    Ok(home.join(".codex").join("sessions"))
}

fn print_summaries(summaries: Vec<SessionSummary>) {
    if summaries.is_empty() {
        println!("no sessions found");
        return;
    }

    for summary in summaries {
        println!(
            "{}  {}  {}  {}",
            summary.started_at, summary.session_id, summary.cwd, summary.title
        );
    }
}

fn print_audit(audit: &SessionAuditRecord) {
    println!("status: {}", audit.result.status.as_str());
    println!("gist: {}", audit.result.gist);
    println!("hinge: {}", audit.result.hinge);
    println!("next: {}", audit.result.next);
    println!("audited_with: {} ({})", audit.model, audit.reasoning_effort);
    println!("created_at: {}", audit.created_at);
    println!("signals:");
    for signal in &audit.result.signals {
        println!("- {signal}");
    }
}

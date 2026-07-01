mod cli;
mod codex;
mod db;
mod indexer;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::db::{Database, SessionSummary};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = resolve_db_path(cli.db)?;
    let database = Database::open(&db_path)?;

    match cli.command {
        Commands::Reindex => {
            let sessions_dir = resolve_sessions_dir(cli.sessions_dir)?;
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
            if !report.failed_files.is_empty() {
                eprintln!("failed files:");
                for failure in report.failed_files {
                    eprintln!("  {failure}");
                }
            }
        }
        Commands::List { limit } => {
            print_summaries(database.list_sessions(limit)?);
        }
        Commands::Search { query, limit } => {
            print_summaries(database.search_sessions(&query, limit)?);
        }
        Commands::Show { session_id } => match database.get_session(&session_id)? {
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
        },
        Commands::Resume { session_id } => {
            println!("codex resume {session_id}");
        }
    }

    Ok(())
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
    for summary in summaries {
        println!(
            "{}  {}  {}  {}",
            summary.started_at, summary.session_id, summary.cwd, summary.title
        );
    }
}

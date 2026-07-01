mod cli;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Reindex => {
            println!("reindex command is wired");
        }
        Commands::List { limit } => {
            println!("list command is wired with limit {limit}");
        }
        Commands::Search { query, limit } => {
            println!("search command is wired for {query:?} with limit {limit}");
        }
        Commands::Show { session_id } => {
            println!("show command is wired for {session_id}");
        }
        Commands::Resume { session_id } => {
            println!("resume command is wired for {session_id}");
        }
    }

    Ok(())
}

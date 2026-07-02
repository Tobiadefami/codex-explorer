use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cx")]
#[command(about = "Codex Explorer - find and resume Codex sessions")]
#[command(after_help = "Run without a command to open the TUI and refresh the index.")]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<PathBuf>,

    #[arg(long, global = true, value_name = "PATH")]
    pub sessions_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Reindex,
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Show {
        session_id: String,
    },
    Resume {
        session_id: String,
    },
}

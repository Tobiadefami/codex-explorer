use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn resume_command(session_id: &str) -> ExternalCommand {
    ExternalCommand {
        program: "codex".to_string(),
        args: vec!["resume".to_string(), session_id.to_string()],
    }
}

pub fn run(command: ExternalCommand) -> Result<i32> {
    let status = std::process::Command::new(&command.program)
        .args(&command.args)
        .status()
        .with_context(|| format!("run {} {}", command.program, command.args.join(" ")))?;

    Ok(status.code().unwrap_or(1))
}

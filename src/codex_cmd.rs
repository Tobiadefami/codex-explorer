use anyhow::{Context, Result};
use std::io::Write;

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

pub fn audit_command(
    schema_path: &str,
    model: &str,
    reasoning_effort: &str,
    prompt: &str,
) -> ExternalCommand {
    ExternalCommand {
        program: "codex".to_string(),
        args: vec![
            "exec".to_string(),
            "--ephemeral".to_string(),
            "--sandbox".to_string(),
            "read-only".to_string(),
            "--ignore-rules".to_string(),
            "--skip-git-repo-check".to_string(),
            "-m".to_string(),
            model.to_string(),
            "-c".to_string(),
            format!("model_reasoning_effort=\"{reasoning_effort}\""),
            "--output-schema".to_string(),
            schema_path.to_string(),
            prompt.to_string(),
        ],
    }
}

pub fn run(command: ExternalCommand) -> Result<i32> {
    let status = std::process::Command::new(&command.program)
        .args(&command.args)
        .status()
        .with_context(|| format!("run {} {}", command.program, command.args.join(" ")))?;

    Ok(status.code().unwrap_or(1))
}

pub fn run_with_stdin(command: ExternalCommand, stdin_text: &str) -> Result<String> {
    let mut child = std::process::Command::new(&command.program)
        .args(&command.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("run {} {}", command.program, command.args.join(" ")))?;

    let Some(mut stdin) = child.stdin.take() else {
        anyhow::bail!("could not open stdin for {}", command.program);
    };
    stdin
        .write_all(stdin_text.as_bytes())
        .with_context(|| format!("write stdin for {}", command.program))?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .with_context(|| format!("wait for {}", command.program))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "{} exited with status {}: {}",
            command.program,
            output.status,
            stderr.trim()
        );
    }

    String::from_utf8(output.stdout)
        .with_context(|| format!("read stdout from {}", command.program))
}

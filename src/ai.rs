use anyhow::{Context, Result};
use std::process::Command;

/// Model to use for opencode CLI. Change this constant to use a different model.
const MODEL: &str = "opencode/deepseek-v4-flash-free";

/// Call the opencode CLI to generate a commit message from the given diff text.
///
/// The prompt is constructed so the model returns only a single short imperative
/// commit message (≤72 characters) without any commentary.
pub fn generate_commit_message(diff: &str, truncated: bool) -> Result<String> {
    // The prompt string is intentionally written out verbatim for easy inspection.
    let instruction =
        "Given these diffs (staged then unstaged), write a single short imperative English commit message (<=72 chars), no explanation, no quotes, nothing else.";

    let prompt = if truncated {
        format!(
            "{}\n\nDiffs (truncated):\n{}\n\nNote: diff was truncated. Output only the commit message anyway.",
            instruction, diff
        )
    } else {
        format!("{}\n\nDiffs:\n{}", instruction, diff)
    };

    let output = Command::new("opencode")
        .arg("run")
        .arg("--model")
        .arg(MODEL)
        .arg(&prompt)
        .output()
        .context("opencode CLI not found. Is it installed and in PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("opencode returned an error: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let message = stdout.trim().to_string();

    if message.is_empty() {
        anyhow::bail!("opencode returned an empty response");
    }

    // Take only the first line in case the model outputs multiple lines.
    let message = message.lines().next().unwrap_or(&message).to_string();

    // Enforce the 72-character maximum.
    let message = if message.len() > 72 {
        message[..72].to_string()
    } else {
        message
    };

    Ok(message)
}

use anyhow::{Context, Result};
use std::process::Command;
/// Model to use for opencode CLI. Override via OPENCODE_MODEL env var.
pub fn get_model() -> String {
    std::env::var("OPENCODE_MODEL")
        .unwrap_or_else(|_| "opencode/big-pickle".to_string())
}
/// Result of AI commit message generation.
pub struct GenerationResult {
    pub message: String,
    pub suggested_version: Option<String>,
}

/// Call the opencode CLI to generate a commit message from the given diff text.
///
/// If `current_version` is `Some`, the prompt asks the AI to also determine
/// the next semantic version based on the change scope (major/minor/patch).
/// The prompt is constructed so the model returns only the commit message
/// and optionally the suggested version.
pub fn generate_commit_message(
    diff: &str,
    truncated: bool,
    current_version: Option<&str>,
) -> Result<GenerationResult> {
    let instruction = "Given these diffs (staged then unstaged), write a short imperative English subject line (<=90 chars) followed by a blank line and a detailed description body explaining what changed and why. No quotes.";

    let prompt = if let Some(ver) = current_version {
        let version_instruction = format!(
            "{}\n\nCurrent version: {}\n\
             Analyze the changes and determine the next semantic version:\n\
             - Breaking changes / major rewrites -> bump major (e.g. v1 -> v2.0.0)\n\
             - New features -> bump minor (e.g. v1.0 -> v1.1.0)\n\
             - Bug fixes / refactors / chores -> bump patch (e.g. v1.0.0 -> v1.0.1)\n\
             Output format:\n\
             <subject line>\n\
             <description body>\n\
             Next version: <version>\n\n",
            instruction, ver,
        );

        if truncated {
            format!(
                "{}\n\nDiffs (truncated):\n{}\n\nNote: diff was truncated. Output only the commit message and suggested version anyway.",
                version_instruction, diff
            )
        } else {
            format!("{}\n\nDiffs:\n{}", version_instruction, diff)
        }
    } else {
        if truncated {
            format!(
                "{}\n\nDiffs (truncated):\n{}\n\nNote: diff was truncated. Output only the commit message anyway.",
                instruction, diff
            )
        } else {
            format!("{}\n\nDiffs:\n{}", instruction, diff)
        }
    };

    let output = Command::new("opencode")
        .arg("run")
        .arg("--model")
        .arg(get_model())
        .arg(&prompt)
        .output()
        .context("opencode CLI not found. Is it installed and in PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("opencode returned an error: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = stdout.trim().to_string();

    if raw.is_empty() {
        anyhow::bail!("opencode returned an empty response");
    }

    // Parse the response for version info
    let (mut message, suggested_version) = if current_version.is_some() {
        parse_versioned_response(&raw)
    } else {
        (raw.clone(), None)
    };

    // Enforce 90-char maximum on the first line (subject), breaking at word boundary.
    truncate_subject(&mut message);

    Ok(GenerationResult {
        message,
        suggested_version,
    })
}

fn char_boundary(s: &str, max_byte: usize) -> usize {
    if s.len() <= max_byte {
        return s.len();
    }
    let mut b = max_byte;
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    b
}

fn truncate_subject(message: &mut String) {
    if let Some(first_newline) = message.find('\n') {
        let subject = &message[..first_newline];
        let rest = &message[first_newline..];
        if subject.len() > 90 {
            let end = char_boundary(subject, 90);
            let cutoff = subject[..end].rfind(' ').unwrap_or(end);
            *message = format!("{}{}", &subject[..cutoff], rest);
        }
    } else if message.len() > 90 {
        let end = char_boundary(message, 90);
        let cutoff = message[..end].rfind(' ').unwrap_or(end);
        *message = message[..cutoff].to_string();
    }
}

/// Parse the response: "<subject>\n<body>\nNext version: <ver>"
fn parse_versioned_response(raw: &str) -> (String, Option<String>) {
    let lines: Vec<&str> = raw.lines().collect();
    let mut message_lines: Vec<&str> = vec![];
    let mut version = None;

    for line in &lines {
        let trimmed = line.trim();
        if let Some(ver) = trimmed.strip_prefix("Next version:") {
            let ver = ver.trim();
            let clean = ver.strip_prefix('v').unwrap_or(ver);
            if clean.split('.').count() == 3 && clean.chars().all(|c| c.is_ascii_digit() || c == '.') {
                version = Some(format!("v{}", clean));
            }
        } else {
            message_lines.push(line);
        }
    }

    let message = message_lines.join("\n").trim().to_string();
    (message, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_versioned_response() {
        let raw = "feat: add user login\n\nImplemented login flow with OAuth2\n\nNext version: v1.1.0";
        let (msg, ver) = parse_versioned_response(raw);
        assert_eq!(msg, "feat: add user login\n\nImplemented login flow with OAuth2");
        assert_eq!(ver, Some("v1.1.0".to_string()));
    }

    #[test]
    fn test_parse_versioned_response_no_version() {
        let raw = "fix: resolve crash on startup\n\nFixed null pointer in User::new()";
        let (msg, ver) = parse_versioned_response(raw);
        assert_eq!(msg, "fix: resolve crash on startup\n\nFixed null pointer in User::new()");
        assert_eq!(ver, None);
    }

    #[test]
    fn test_parse_versioned_response_extra_lines() {
        let raw = "chore: update dependencies\n\nBumped reqwest to 0.12\n\nNext version: v1.0.1\n";
        let (msg, ver) = parse_versioned_response(raw);
        assert_eq!(msg, "chore: update dependencies\n\nBumped reqwest to 0.12");
        assert_eq!(ver, Some("v1.0.1".to_string()));
    }

}

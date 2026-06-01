use anyhow::{Context, Result};
use std::process::Command;







///----------------------------------------------------------------------------------------------
///----------------------------------------------------------------------------------------------
///----------------------------------------------------------------------------------------------
/// Model to use for opencode CLI. Change this constant to use a different model.
const MODEL: &str = "opencode/deepseek-v4-flash-free";
///----------------------------------------------------------------------------------------------
///----------------------------------------------------------------------------------------------
///----------------------------------------------------------------------------------------------








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
    let instruction = "Given these diffs (staged then unstaged), write a single short imperative English commit message (<=72 chars), no explanation, no quotes, nothing else.";

    let prompt = if let Some(ver) = current_version {
        let version_instruction = format!(
            "{}\n\nCurrent version: {}\n\
             Analyze the changes and determine the next semantic version:\n\
             - Breaking changes / major rewrites -> bump major (e.g. v1 -> v2.0.0)\n\
             - New features -> bump minor (e.g. v1.0 -> v1.1.0)\n\
             - Bug fixes / refactors / chores -> bump patch (e.g. v1.0.0 -> v1.0.1)\n\
             Output format (exactly two lines):\n\
             <commit message>\n\
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
        .arg(MODEL)
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
    let (message, suggested_version) = if current_version.is_some() {
        parse_versioned_response(&raw)
    } else {
        (raw.clone(), None)
    };

    // Take only the first line if no version was parsed separately
    let message = if current_version.is_some() {
        message
    } else {
        message.lines().next().unwrap_or(&message).to_string()
    };

    // Enforce the 72-character maximum.
    let message = if message.len() > 72 {
        message[..72].to_string()
    } else {
        message
    };

    Ok(GenerationResult {
        message,
        suggested_version,
    })
}

/// Parse the two-line response: "<message>\nNext version: <ver>"
fn parse_versioned_response(raw: &str) -> (String, Option<String>) {
    let lines: Vec<&str> = raw.lines().collect();
    let mut message = String::new();
    let mut version = None;

    for line in &lines {
        let trimmed = line.trim();
        if let Some(ver) = trimmed.strip_prefix("Next version:") {
            let ver = ver.trim();
            // Validate basic semver format (vX.Y.Z or X.Y.Z)
            let clean = ver.strip_prefix('v').unwrap_or(ver);
            if clean.split('.').count() == 3 && clean.chars().all(|c| c.is_ascii_digit() || c == '.') {
                version = Some(format!("v{}", clean));
            }
            // Don't add this line to the message
        } else if !trimmed.is_empty() && message.is_empty() {
            // First non-empty line that isn't a version line = the commit message
            message = trimmed.to_string();
        }
    }

    if message.is_empty() {
        message = lines.first().unwrap_or(&"").trim().to_string();
    }

    (message, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_versioned_response() {
        let raw = "feat: add user login\nNext version: v1.1.0";
        let (msg, ver) = parse_versioned_response(raw);
        assert_eq!(msg, "feat: add user login");
        assert_eq!(ver, Some("v1.1.0".to_string()));
    }

    #[test]
    fn test_parse_versioned_response_no_version() {
        let raw = "fix: resolve crash on startup";
        let (msg, ver) = parse_versioned_response(raw);
        assert_eq!(msg, "fix: resolve crash on startup");
        assert_eq!(ver, None);
    }

    #[test]
    fn test_parse_versioned_response_extra_lines() {
        let raw = "chore: update dependencies\n\nNext version: v1.0.1\n";
        let (msg, ver) = parse_versioned_response(raw);
        assert_eq!(msg, "chore: update dependencies");
        assert_eq!(ver, Some("v1.0.1".to_string()));
    }

}

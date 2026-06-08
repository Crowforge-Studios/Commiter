use anyhow::{Context, Result};
use git2::{DiffOptions, Repository, Signature};

/// Maximum bytes of combined diff text sent to the AI.
/// If the combined staged+unstaged diff exceeds this, it is truncated.
/// Override via COMMITER_DIFF_CUTOFF env var.
fn diff_cutoff() -> usize {
    std::env::var("COMMITER_DIFF_CUTOFF")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8192)
}

#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub repo_path: String,
    pub branch: String,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub changed_files: Vec<String>,
    pub combined_diff: String,
    pub truncated: bool,
    pub has_changes: bool,
}

/// Open the git repository in the current working directory, collect staged and
/// unstaged diffs, and return summary information. Returns an error if CWD is
/// not inside a git repository.
pub fn get_repo_info() -> Result<RepoInfo> {
    let repo = Repository::open(".").context("Not a git repository")?;

    let repo_path = repo
        .workdir()
        .unwrap_or_else(|| repo.path().parent().unwrap())
        .to_string_lossy()
        .to_string();

    let branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "HEAD".to_string());

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    opts.context_lines(3);

    let (staged_count, staged_text, staged_files) = {
        let diff = repo
            .diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
            .context("Failed to get staged diff")?;

        let count = diff.deltas().count();
        let files: Vec<String> = diff
            .deltas()
            .map(|d| {
                d.new_file()
                    .path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .collect();
        let mut text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if let Ok(content) = std::str::from_utf8(line.content()) {
                text.push_str(content);
            }
            true
        })
        .context("Failed to print staged diff")?;

        (count, text, files)
    };

    let (unstaged_count, unstaged_text, unstaged_files) = {
        let index = repo.index().context("Failed to open index")?;
        let diff = repo
            .diff_index_to_workdir(Some(&index), Some(&mut opts))
            .context("Failed to get unstaged diff")?;

        let count = diff.deltas().count();
        let files: Vec<String> = diff
            .deltas()
            .map(|d| {
                d.new_file()
                    .path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .collect();
        let mut text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if let Ok(content) = std::str::from_utf8(line.content()) {
                text.push_str(content);
            }
            true
        })
        .context("Failed to print unstaged diff")?;

        (count, text, files)
    };

    // Combine file lists (deduped, staged first)
    let mut changed_files: Vec<String> = staged_files.clone();
    for f in &unstaged_files {
        if !changed_files.contains(f) {
            changed_files.push(f.clone());
        }
    }

    let has_changes = staged_count > 0 || unstaged_count > 0;

    let combined = if staged_text.is_empty() && unstaged_text.is_empty() {
        String::new()
    } else {
        let mut s = String::new();
        if !staged_text.is_empty() {
            s.push_str(&format!(
                "===== STAGED CHANGES ({} files) =====\n",
                staged_count
            ));
            s.push_str(&staged_text);
            s.push('\n');
        }
        if !unstaged_text.is_empty() {
            s.push_str(&format!(
                "===== UNSTAGED CHANGES ({} files) =====\n",
                unstaged_count
            ));
            s.push_str(&unstaged_text);
        }
        s
    };

    let cutoff_bytes = diff_cutoff();
    let truncated = combined.len() > cutoff_bytes;
    let combined_diff = if truncated {
        let cutoff = combined
            .char_indices()
            .nth(cutoff_bytes)
            .map(|(i, _)| i)
            .unwrap_or(combined.len());
        let mut t = combined[..cutoff].to_string();
        t.push_str("\n... [diff truncated due to size]");
        t
    } else {
        combined
    };

    Ok(RepoInfo {
        repo_path,
        branch,
        staged_count,
        unstaged_count,
        changed_files,
        combined_diff,
        truncated,
        has_changes,
    })
}

/// Stage all changes (equivalent to `git add -A`) and create a commit with
/// the given message. Returns the short commit hash on success.
pub fn stage_all_and_commit(repo_path: &str, message: &str) -> Result<String> {
    let repo =
        Repository::open(repo_path).context("Failed to open repository for commit")?;

    let mut index = repo.index().context("Failed to open index")?;
    // Stage all files including untracked (equivalent to git add -A).
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .context("Failed to stage files")?;
    index.write().context("Failed to write index")?;

    let tree_id = index.write_tree().context("Failed to write tree")?;
    let tree = repo.find_tree(tree_id).context("Failed to find tree")?;

    let sig = get_signature(&repo);

    let parents: Vec<git2::Commit> = if let Ok(head) = repo.head() {
        if let Some(target) = head.target() {
            if let Ok(commit) = repo.find_commit(target) {
                vec![commit]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    } else {
        vec![]
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .context("Failed to create commit")?;

    Ok(oid.to_string())
}

/// Detect the current version from git tags or Cargo.toml.
/// Returns `Some("vX.Y.Z")` if a version is found, `None` otherwise.
/// This is used to enable AI-driven version bump suggestions.
pub fn detect_current_version() -> Option<String> {
    // Try git tags first (most reliable indicator of versioned releases)
    if let Ok(repo) = Repository::open(".") {
        if let Ok(tags) = repo.tag_names(None) {
            let mut versions: Vec<semver::Version> = tags
                .iter()
                .flatten()
                .filter_map(|name| {
                    let name = name.strip_prefix('v').unwrap_or(name);
                    semver::Version::parse(name)
                })
                .collect();
            versions.sort();
            if let Some(latest) = versions.last() {
                return Some(format!("v{}", latest));
            }
        }
    }

    // Fallback: read version from Cargo.toml
    if let Ok(content) = std::fs::read_to_string("Cargo.toml") {
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("version =") {
                let ver = rest.trim().trim_matches('"').trim_matches('\'');
                if !ver.is_empty() {
                    return Some(format!("v{}", ver));
                }
            }
        }
    }

    None
}

/// Produce a git signature by reading the user's git config, falling back to a
/// sensible default if the config is missing.
fn get_signature(repo: &Repository) -> Signature<'static> {
    let name = repo
        .config()
        .ok()
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "Commiter".to_string());
    let email = repo
        .config()
        .ok()
        .and_then(|c| c.get_string("user.email").ok())
        .unwrap_or_else(|| "commiter@local".to_string());
    Signature::now(&name, &email).expect("Failed to create git signature")
}

// Manual semver parse to avoid adding a semver dependency.
// Simplified: only handles X.Y.Z format.
mod semver {
    use std::cmp::Ordering;

    #[derive(Debug, Eq, PartialEq)]
    pub struct Version {
        pub major: u64,
        pub minor: u64,
        pub patch: u64,
    }

    impl Version {
        pub fn parse(s: &str) -> Option<Self> {
            let parts: Vec<&str> = s.split('.').collect();
            if parts.len() != 3 {
                return None;
            }
            let major = parts[0].parse().ok()?;
            let minor = parts[1].parse().ok()?;
            let patch = parts[2].parse().ok()?;
            Some(Version {
                major,
                minor,
                patch,
            })
        }
    }

    impl Ord for Version {
        fn cmp(&self, other: &Self) -> Ordering {
            self.major
                .cmp(&other.major)
                .then(self.minor.cmp(&other.minor))
                .then(self.patch.cmp(&other.patch))
        }
    }

    impl PartialOrd for Version {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl std::fmt::Display for Version {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_parse() {
        let v = semver::Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_semver_ordering() {
        let v1 = semver::Version::parse("1.0.0").unwrap();
        let v2 = semver::Version::parse("2.0.0").unwrap();
        let v3 = semver::Version::parse("1.1.0").unwrap();
        let v4 = semver::Version::parse("1.0.1").unwrap();
        assert!(v1 < v2);
        assert!(v1 < v3);
        assert!(v1 < v4);
        assert!(v3 < v2);
        assert!(v4 < v3);
    }

    #[test]
    fn test_semver_invalid() {
        assert!(semver::Version::parse("1.2").is_none());
        assert!(semver::Version::parse("abc").is_none());
        assert!(semver::Version::parse("1.2.3.4").is_none());
    }
}

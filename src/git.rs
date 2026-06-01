use anyhow::{Context, Result};
use git2::{DiffOptions, Repository, Signature};

/// Maximum bytes of combined diff text sent to the AI.
/// If the combined staged+unstaged diff exceeds this, it is truncated.
pub const DIFF_CUTOFF: usize = 8192;

#[derive(Clone)]
pub struct RepoInfo {
    pub repo_path: String,
    pub staged_count: usize,
    pub unstaged_count: usize,
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

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    opts.context_lines(3);

    let (staged_count, staged_text) = {
        let diff = repo
            .diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
            .context("Failed to get staged diff")?;

        let count = diff.deltas().count();
        let mut text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if let Ok(content) = std::str::from_utf8(line.content()) {
                text.push_str(content);
            }
            true
        })
        .context("Failed to print staged diff")?;

        (count, text)
    };

    let (unstaged_count, unstaged_text) = {
        let mut index = repo.index().context("Failed to open index")?;
        let diff = repo
            .diff_index_to_workdir(Some(&mut index), Some(&mut opts))
            .context("Failed to get unstaged diff")?;

        let count = diff.deltas().count();
        let mut text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            if let Ok(content) = std::str::from_utf8(line.content()) {
                text.push_str(content);
            }
            true
        })
        .context("Failed to print unstaged diff")?;

        (count, text)
    };

    let has_changes = staged_count > 0 || unstaged_count > 0;

    let combined = if staged_text.is_empty() && unstaged_text.is_empty() {
        String::new()
    } else {
        let mut s = String::new();
        if !staged_text.is_empty() {
            s.push_str(&format!("===== STAGED CHANGES ({} files) =====\n", staged_count));
            s.push_str(&staged_text);
            s.push('\n');
        }
        if !unstaged_text.is_empty() {
            s.push_str(&format!("===== UNSTAGED CHANGES ({} files) =====\n", unstaged_count));
            s.push_str(&unstaged_text);
        }
        s
    };

    let truncated = combined.len() > DIFF_CUTOFF;
    let combined_diff = if truncated {
        let mut t = combined[..DIFF_CUTOFF].to_string();
        t.push_str("\n... [diff truncated due to size]");
        t
    } else {
        combined
    };

    Ok(RepoInfo {
        repo_path,
        staged_count,
        unstaged_count,
        combined_diff,
        truncated,
        has_changes,
    })
}

/// Stage all changes (equivalent to `git add -A`) and create a commit with
/// the given message. Returns the short commit hash on success.
pub fn stage_all_and_commit(repo_path: &str, message: &str) -> Result<String> {
    let repo = Repository::open(repo_path).context("Failed to open repository for commit")?;

    let mut index = repo.index().context("Failed to open index")?;
    index
        .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
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

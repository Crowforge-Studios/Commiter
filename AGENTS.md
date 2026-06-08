# Commiter — Agent Guide

Single-crate Rust TUI app (`ratatui` + `crossterm`) that reads git diffs, calls the
`opencode` CLI to generate a commit message, copies it to clipboard, and optionally
commits.

## Build

```sh
cargo build --release                              # dynamic release + strip
make build                                          # above + strip + cp to ./
make release                                        # static musl (needs musl-tools)
make docker-build                                   # static via Docker
```

Tests: `cargo test` (unit tests in `src/ai.rs`, `src/app.rs`, `src/git.rs`).

No linter/formatter/CI config exists.

## Runtime requirements (not in Cargo.toml)

- **`opencode` CLI** in `$PATH` — called as `opencode run --model <model> <prompt>`
- **Clipboard tool** (one of): `wl-copy` (Wayland), `xclip` or `xsel` (X11)
- **`git2`** is vendored with `vendored-libgit2` feature — no system libgit2 needed

## Configuration

| Env var | Default | CLI flag | Description |
|---|---|---|---|
| `OPENCODE_MODEL` | `opencode/deepseek-v4-flash-free` | `--model` | AI model for opencode |
| `COMMITER_DIFF_CUTOFF` | `8192` | `--diff-cutoff` | Max diff bytes sent to AI |

## Architecture

- `src/main.rs` — entrypoint, CLI arg parsing, TUI event loop, keyboard dispatch, edit-mode input handling
- `src/app.rs` — `App` struct, state machine, UI rendering, edit-mode text buffer with cursor
- `src/git.rs` — `get_repo_info()` (staged+unstaged diffs, truncated via `COMMITER_DIFF_CUTOFF`),
  `stage_all_and_commit()` (git add -A), `detect_current_version()` (git tags → Cargo.toml)
- `src/ai.rs` — `generate_commit_message()` calls `opencode run`, reads model from `OPENCODE_MODEL`, parses `Next version:` line
- `src/clipboard.rs` — tries `wl-copy` → `xclip` → `xsel`

## Keys

| Key | Context | Action |
|---|---|---|
| `Enter` | PreGenerated | Copy message to clipboard |
| `Enter` | Idle + changes | Generate commit message |
| `Enter` | Ready | Commit |
| `e` | Ready + message | Enter edit mode |
| `Esc` | Editing | Exit edit mode |
| `r` | PreGenerated / Ready | Regenerate message |
| `F1` | Any | Toggle file list |
| `q` | Any | Quit |
| Char/Backspace/Delete/←/→/Home/End | Editing | Text editing |

## Workflow quirks

- The app **pre-generates** a commit message on launch (if changes exist), then user presses
  Enter to copy it, Enter again to commit. `r` regenerates.
- Press `e` in Ready state to edit the message before committing.
- Version detection: sorts git tags by semver, picks latest; fallback to Cargo.toml `version` field.
  If found, the AI prompt asks for a suggested next version (major/minor/patch bump).
- Subject line is truncated at word boundary to 90 chars in `src/ai.rs:108-119`.
- Commit uses `git add -A` (all files including untracked). Signature reads from git config
  or falls back to `Commiter <commiter@local>`.

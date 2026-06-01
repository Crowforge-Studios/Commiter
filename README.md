# Commiter

A minimal Rust TUI app that inspects the current Git repository, generates a
short imperative commit message via the [opencode CLI](https://opencode.ai), copies it
to the system clipboard, and optionally stages & commits all changes.

## Requirements

- **Rust stable** (edition 2021)
- **opencode CLI** installed and available in `PATH` — see [opencode.ai](https://opencode.ai)
- **Linux system dependencies** (for the clipboard crate `arboard` with X11 support):

  ```bash
  # Ubuntu / Debian
  sudo apt install pkg-config libssl-dev libxcb1-dev libxcb-render0-dev \
                   libxcb-shape0-dev libxcb-xfixes0-dev

  # Fedora
  sudo dnf install pkg-config openssl-devel libxcb-devel
  ```

## Build

```bash
cargo build --release
```

## Run

Inside any Git repository:

```bash
cargo run --release
```

The UI is keyboard-only:

| Key      | Action                      |
| -------- | --------------------------- |
| `Enter`  | Activate the focused button |
| `q`      | Quit                        |

### Workflow

1. Open the app inside a Git repo with staged and/or unstaged changes.
2. Press **Enter** to run the `Create commit message` button.
3. The app gathers diffs, calls `opencode run --model "opencode/deepseek-v4-flash-free" "<prompt>"`, and copies the
   returned message to the clipboard.
4. A second button, **Commit changes**, appears. Press **Enter** to stage all
   changes and commit with the generated message.

## AI Model

The app uses `opencode/deepseek-v4-flash-free` (a free model) via the opencode CLI.
The model is defined as a constant in `src/ai.rs`:

```rust
const MODEL: &str = "opencode/deepseek-v4-flash-free";
```

To use a different model, change this constant and rebuild.

## Example opencode prompt

The prompt passed to `opencode run` is constructed as follows (verbatim):

```
$ opencode run --model "opencode/deepseek-v4-flash-free" "Given these diffs (staged then unstaged), write a single short imperative English commit message (<=72 chars), no explanation, no quotes, nothing else.

Diffs:
===== STAGED CHANGES (2 files) =====
diff --git a/src/main.rs b/src/main.rs
...

===== UNSTAGED CHANGES (1 file) =====
diff --git a/src/lib.rs b/src/lib.rs
..."
```

If the combined diff exceeds the cutoff (see below), a `Diffs (truncated):`
header is used and a note is appended:

```
Note: diff was truncated. Output only the commit message anyway.
```

## Diff size cutoff

The combined staged + unstaged diff sent to the AI model is capped at
**8192 bytes** (defined as `DIFF_CUTOFF` in `src/git.rs`). If the diff is
larger, it is truncated and a `[diff truncated]` indicator is shown both in
the UI and in the prompt sent to opencode.

This keeps prompt sizes reasonable and avoids hitting token limits.

## License

MIT

# Commiter

A Rust TUI app that inspects the current Git repository, generates a
short imperative commit message via the [opencode CLI](https://opencode.ai), copies it
to the system clipboard, and optionally stages & commits all changes.

## Quick start (binary)

1. Download the latest `commiter` binary from [Releases](https://github.com/trk/commiter/releases).
2. Make it executable and run it inside any Git repository:

```bash
chmod +x commiter
./commiter
```

## Requirements

- **opencode CLI** — [install](https://opencode.ai) and ensure it's in `PATH`
- **Clipboard tool** (one of):
  - Wayland: `wl-copy` (`wl-clipboard` package)
  - X11: `xclip` or `xsel`

## Build from source

### Normal (dynamic)

```bash
cargo build --release
```

### Fully static binary (recommended for distribution)

Requires the `x86_64-unknown-linux-musl` target and musl toolchain:

```bash
# Install musl target
rustup target add x86_64-unknown-linux-musl

# Ubuntu / Debian: install musl-gcc
sudo apt install musl-tools

# Build
make release

# Or manually:
cargo build --target x86_64-unknown-linux-musl --release
strip target/x86_64-unknown-linux-musl/release/commiter
```

**Docker build** (no toolchain setup needed):

```bash
make docker-build
```

The resulting binary at `./commiter` has **no external library dependencies** — it runs on any Linux x86_64 system.

## Run

Inside any Git repository:

```bash
./commiter
```

The UI is keyboard-only:

| Key      | Action                      |
| -------- | --------------------------- |
| `Enter`  | Activate the focused button |
| `F1`     | Toggle file list visibility |
| `q`      | Quit                        |

### Workflow

1. Open the app inside a Git repo with staged and/or unstaged changes.
2. Press **Enter** to run the `Create commit message` button.
3. The app gathers diffs, calls `opencode run`, and copies the returned message to the clipboard.
4. A second button, **Commit changes**, appears. Press **Enter** to stage all changes and commit.

### Version detection

If your project has semver git tags (e.g. `v1.0.0`) or a `version` field in
`Cargo.toml`, the AI prompt includes version-aware instructions. The model
returns a suggested next version alongside the commit message, displayed
in the UI.

## AI Model

The app uses `opencode/deepseek-v4-flash-free` (configurable in `src/ai.rs`).

## License

MIT

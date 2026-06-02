# Commiter

A Rust TUI app that inspects the current Git repository, generates a
short imperative commit message via the [opencode CLI](https://opencode.ai), copies it
to the system clipboard, and optionally stages & commits all changes.

## Quick start (binary)

1. Download the latest `commiter` binary from [Releases](https://github.com/Crowforge-Studios/Commiter/releases).
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

### Makefile targets

| Target           | Description                                                         |
| ---------------- | ------------------------------------------------------------------- |
| `make build`     | Build & strip release binary (dynamic linking)                      |
| `make release`   | Build & strip fully static binary (`x86_64-unknown-linux-musl`)     |
| `make docker-build` | Build fully static binary via Docker (no toolchain setup needed) |
| `make clean`     | Remove all build artifacts                                          |

All targets output the final binary as `./commiter`.

### Normal (dynamic)

```bash
make build
```

### Fully static binary (recommended for distribution)

Requires the musl toolchain:

```bash
# Install musl target
rustup target add x86_64-unknown-linux-musl

# Ubuntu / Debian: install musl-gcc
sudo apt install musl-tools

# Build
make release
```

**Docker build** (no toolchain setup at all):

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

| Key      | Action                                      |
| -------- | ------------------------------------------- |
| `Enter`  | Use message / Generate / Commit (context)   |
| `r`      | Regenerate message (re-runs with full diff) |
| `F1`     | Toggle file list visibility                 |
| `q`      | Quit                                        |

### Workflow

1. Open the app inside a Git repo with staged and/or unstaged changes.
2. The app **immediately starts pre-generating** a commit message in the background (using just the changed file list for speed).
3. When ready, `✓ Ready — Press Enter` appears.
4. Press **Enter** to instantly copy the message to the clipboard.
5. A **Commit changes** button appears. Press **Enter** again to stage all changes and commit.
6. Not happy with the message? Press **`r`** to regenerate using the full diff.

### Version detection

If your project has semver git tags (e.g. `v1.0.0`) or a `version` field in
`Cargo.toml`, the AI prompt includes version-aware instructions. The model
returns a suggested next version alongside the commit message, displayed
in the UI.

## AI Model

The app uses `opencode/deepseek-v4-flash-free` (configurable in `src/ai.rs`).

## License

MIT

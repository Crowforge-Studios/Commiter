# Commiter

A Rust TUI app that inspects the current Git repository, generates a
detailed commit message with description via the [opencode CLI](https://opencode.ai),
copies it to the system clipboard, and optionally stages & commits all changes.

## Install

### One-liner (recommended)
```bash
curl -fsSL https://raw.githubusercontent.com/Crowforge-Studios/Commiter/master/install.sh | sh
```

Installs to `~/.local/bin/commiter`. Run `commiter` in any git repo.

### Options
```bash
./install.sh -v v1.0.0                   # specific version
./install.sh -p /usr/local/bin           # custom prefix (needs sudo)
./install.sh -u                          # uninstall
```

### Cargo
```bash
cargo install --git https://github.com/Crowforge-Studios/Commiter
```

## Requirements
- **opencode CLI** — [install](https://opencode.ai), ensure it's in `PATH`
- **Clipboard tool** (one of):
  - Wayland: `wl-copy` (`wl-clipboard`)
  - X11: `xclip` or `xsel`

## Usage

Inside any Git repository:
```bash
commiter
```

### Keys

| Key       | Action                                    |
|-----------|-------------------------------------------|
| `Enter`   | Use message / Generate / Commit (context) |
| `s`       | Settings (uninstall, version info)         |
| `r`       | Regenerate message                        |
| `F1`      | Toggle file list                          |
| `q`       | Quit                                      |

### Workflow

1. Open the app inside a Git repo with staged/unstaged changes.
2. The app **pre-generates** a commit message in the background.
3. Press **Enter** to copy to clipboard.
4. Press **Enter** again to stage all and commit.
5. Press **`r`** to regenerate with full diff.
6. Press **`s`** for Settings to uninstall or view version info.

### Settings

Press `s` to open the settings panel:
- **Version info** — current installed version and latest available
- **`u` Update** — downloads and replaces binary, auto-restarts
- **`x` Uninstall** — removes binary from system

Only shows when installed system-wide (not local `./commiter`).

## Build from source

### Makefile

| Target            | Description                                   |
|-------------------|-----------------------------------------------|
| `make build`      | Build & strip release (dynamic)               |
| `make release`    | Fully static binary (musl)                    |
| `make docker-build` | Static binary via Docker                    |
| `make clean`      | Remove build artifacts                        |

```bash
make build
```

For a fully static binary (no deps):
```bash
# Install musl target
rustup target add x86_64-unknown-linux-musl
sudo apt install musl-tools   # Debian/Ubuntu
make release
```

Or via Docker (no toolchain setup):
```bash
make docker-build
```

The resulting binary at `./commiter` has no external dependencies.

## AI Model

Uses `opencode/deepseek-v4-flash-free` by default (configurable in `src/ai.rs`).

## License

MIT

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

### Uninstall
```bash
curl -fsSL https://raw.githubusercontent.com/Crowforge-Studios/Commiter/master/install.sh | sh -s -- -u
```

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

### CLI Options

| Flag                  | Description                                   |
|-----------------------|-----------------------------------------------|
| `--model <model>`     | AI model (default: `opencode/deepseek-v4-flash-free`), or `OPENCODE_MODEL` env var |
| `--diff-cutoff <bytes>` | Max diff bytes sent to AI (default: 8192), or `COMMITER_DIFF_CUTOFF` env var |
| `--version, -V`       | Print version and exit                        |
| `--help, -h`          | Print this help                               |

### Workflow

1. Open the app inside a Git repo with staged/unstaged changes.
2. The app **pre-generates** a commit message in the background.
3. Press **Enter** to copy to clipboard.
4. Press **Enter** again to stage all and commit.
5. Press **`e`** to edit the message before committing.
6. Press **`r`** to regenerate with full diff.

### Keys

| Key       | Context                 | Action                     |
|-----------|-------------------------|----------------------------|
| `Enter`   | PreGenerated            | Copy message to clipboard  |
| `Enter`   | Idle + changes          | Generate commit message    |
| `Enter`   | Ready                   | Commit                     |
| `e`       | Ready + message         | Enter edit mode            |
| `Esc`     | Editing                 | Exit edit mode             |
| `r`       | PreGenerated / Ready    | Regenerate message         |
| `F1`      | Any                     | Toggle file list           |
| `q`       | Any                     | Quit                       |

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

Uses `opencode/deepseek-v4-flash-free` by default. Override with `--model` flag
or `OPENCODE_MODEL` environment variable.

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests on
[GitHub](https://github.com/Crowforge-Studios/Commiter).

## License

MIT

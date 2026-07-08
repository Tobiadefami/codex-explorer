# Codex Explorer

Codex Explorer is a local Codex session manager. The `cx` command indexes `~/.codex/sessions/**/*.jsonl` so you can search, preview, and resume old Codex sessions without remembering the exact date.

## Requirements

- Rust and Cargo
- The Codex CLI available as `codex` on your `PATH` for `cx resume`
- A terminal with TUI support

## Install

From this checkout:

```bash
cargo install --path . --locked
```

Cargo installs the `cx` binary into `~/.cargo/bin`. Make sure that directory is on your `PATH`, then verify the installed command:

```bash
cx --version
```

To reinstall from a newer checkout:

```bash
cargo install --path . --locked --force
```

From GitHub:

```bash
cargo install --git https://github.com/Tobiadefami/codex-explorer --locked
```

## Commands

```bash
cx
cx reindex
cx list
cx search "turnstile worker"
cx show <session-id>
cx resume <session-id>
```

Run `cx` without a command to open the terminal session browser. The browser opens immediately with cached sessions and refreshes the index in the background.

TUI keys:

- type to search
- `Backspace` edits search
- `Up`/`Down` moves selection
- `a` shows all projects
- `p` returns to the current directory
- `PgUp`/`PgDn` scrolls the preview
- `Enter` resumes the selected session
- `Esc` quits
- `q` quits when the search box is empty

## Local Data

`cx` stores its search index in your platform data directory under `cx/index.sqlite`. On Linux this is typically `~/.local/share/cx/index.sqlite`.

The index is local SQLite data. It contains session metadata, source file paths, titles, searchable text, and full user/assistant message text so `cx show`, search, and previews work without reparsing every JSONL file each time. Use `--db <PATH>` to write the index somewhere else.

`cx` reads Codex session files from `~/.codex/sessions` by default. Use `--sessions-dir <PATH>` to index a different directory.

## Development

Run tests:

```bash
cargo test
```

Run the full local check before opening a pull request:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo package --allow-dirty --no-verify --offline
```

Run against fixture data:

```bash
cargo run -- --db /tmp/cx.sqlite --sessions-dir tests/fixtures reindex
cargo run -- --db /tmp/cx.sqlite search turnstile
```

## License

MIT

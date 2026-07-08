# Codex Explorer

Codex Explorer is a local Codex session manager. The `cx` command indexes `~/.codex/sessions/**/*.jsonl` so you can search, preview, and resume old Codex sessions without remembering the exact date.

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

## Development

Run tests:

```bash
cargo test
```

Run against fixture data:

```bash
cargo run -- --db /tmp/cx.sqlite --sessions-dir tests/fixtures reindex
cargo run -- --db /tmp/cx.sqlite search turnstile
```

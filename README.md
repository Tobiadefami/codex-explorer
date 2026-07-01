# cx

`cx` is a local Codex session manager. It indexes `~/.codex/sessions/**/*.jsonl` so you can search, preview, and resume old Codex sessions without remembering the exact date.

## Commands

```bash
cx reindex
cx list
cx search "turnstile worker"
cx show <session-id>
cx resume <session-id>
```

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

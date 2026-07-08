# Contributing

Thanks for taking the time to improve Codex Explorer.

## Development

Install Rust, then run the checks used by CI:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

For manual testing against fixture data:

```bash
cargo run -- --db /tmp/cx.sqlite --sessions-dir tests/fixtures reindex
cargo run -- --db /tmp/cx.sqlite search turnstile
```

## Pull Requests

- Keep changes focused.
- Add or update tests for behavior changes.
- Do not commit personal Codex session files, generated indexes, secrets, or local machine paths unless they are sanitized fixtures.
- Update `README.md` when changing user-visible behavior.

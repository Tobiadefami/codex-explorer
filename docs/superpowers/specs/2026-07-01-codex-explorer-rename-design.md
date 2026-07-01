# Codex Explorer Rename Design

## Purpose

Rename the project identity from the workspace-oriented `computer-use` folder to `Codex Explorer`, while keeping the fast `cx` command.

## Decisions

- Repository folder: `/home/chief/codex-explorer`
- Product name in docs/help text: `Codex Explorer`
- CLI command: `cx`
- Rust package and binary name: keep `cx`
- Default database path: keep `~/.local/share/cx/index.sqlite`

## Rationale

`computer-use` describes a broad workspace, not this tool. `Codex Explorer` describes the app's purpose: browse, search, preview, and resume Codex sessions.

Keeping `cx` avoids breaking the command muscle memory and preserves the current database path. The command can be understood as short for `Codex Explorer`.

## Scope

This rename updates product-facing text and moves the repository folder. It does not rename the binary, package, database directory, or existing historical planning docs.

# Codex Explorer TUI Visual Refresh Design

## Goal

Make the `cx` terminal UI clearly more useful than `codex resume` by helping users recognize the right session quickly. The interface should answer three questions at a glance:

- What was I trying to do?
- Where was I working?
- Is this the session I want to resume?

The refresh should improve presentation and information hierarchy using the data already indexed today. It should not add tags, semantic summaries, background indexing, or a new persistence model.

## Current Problem

The existing TUI works functionally but presents sessions like raw database rows. The list shows timestamp, title, and cwd, while the preview prints metadata and raw messages. This makes the tool feel too close to `codex resume`: technically searchable, but not obviously better for recognition.

The primary issue is not lack of data. The issue is that the UI does not package the data into a decision-oriented view.

## Design Direction

Use a structured finder layout:

- A compact header shows product name, result count, and current mode.
- A prominent search box invites searching by task, repo, file, error, or phrase.
- The left pane is a polished two-line session list optimized for scanning.
- The right pane is a sectioned preview optimized for confirming the selected session.
- A short footer shows the available keys.

The visual style should be calm and information-dense. Use color and text weight for hierarchy, not decoration.

## Layout

The screen is divided vertically into header, search, body, and footer.

The body is split horizontally:

- Left pane: session list, about 42 percent width.
- Right pane: selected session preview, remaining width.

On narrow terminals, keep the same structure and let Ratatui truncate content. A responsive stacked layout is outside this refresh.

## Header

The header should show:

- `Codex Explorer`
- result count such as `24 sessions` or `3 matches`
- a short state label such as `Recent` or `Search`

The header should not contain long help text. Its job is orientation.

## Search

The search box should be visually prominent and include useful empty-state text when there is no query:

`Search sessions by task, repo, error, file...`

When a query is active, the box shows the typed query and the list switches to search results. Empty search results should show a helpful message:

`No sessions match "query"`

## Session List

Each session row should use two lines:

Line 1:

- session title, trimmed to fit

Line 2:

- repo or cwd label
- relative or compact date

The selected row should be visibly stronger than surrounding rows using an accent color and bold title. Unselected metadata should be dim.

The list should avoid showing full UUIDs. Session IDs belong in the preview metadata, shortened unless the user explicitly needs the full value.

## Preview

The preview should be sectioned instead of a raw dump:

`Task`

- The selected session title.

`Context`

- Project path, preferably shortened to repo name plus parent context when possible.
- Started time in a readable compact format.
- Source path only if there is enough room, shortened.

`Conversation`

- Recent meaningful user and assistant turns.
- Skip bootstrap/environment messages.
- Trim long lines and preserve enough text to identify the session.
- Use role labels with subtle styling.

`Resume`

- Show a shortened session id.
- Show `Enter` as the primary action hint.

The preview should make it possible to decide whether to resume without reading an unstructured transcript.

## Visual Style

Use restrained terminal styling:

- Accent color for selection and active elements: cyan or teal.
- Dim gray for secondary metadata.
- Yellow only for warnings or search emphasis.
- Bold for primary titles and section labels.
- Avoid dense borders around every small element.

The UI should feel like a practical terminal tool, not a dashboard with decorative chrome.

## Interaction

Keep the current core interactions:

- Type to search.
- `Backspace` edits search.
- `Up` and `Down` move selection.
- `j` and `k` move selection when the search box is empty.
- `Enter` resumes the selected session.
- `Esc` quits.
- `q` quits when the search box is empty.

The footer should reflect these behaviors accurately.

## Implementation Notes

The refresh should be implemented mostly inside `src/tui.rs`.

Potential helper functions:

- format compact timestamps
- shorten paths
- shorten session IDs
- trim text to terminal width
- build preview sections
- build session row lines

Tests should focus on formatting and state behavior, not pixel-perfect terminal rendering.

## Out of Scope

- AI-generated session summaries.
- Tags, archive, delete, or fork actions.
- Mouse support.
- Semantic search.
- Persisted UI preferences.
- Major database migrations.

## Success Criteria

- Opening `cx` immediately communicates what the tool is for.
- Recent sessions are easier to distinguish than in `codex resume`.
- Search results can be scanned by task and project.
- The preview answers whether the selected session is worth resuming.
- Existing CLI commands and TUI behavior continue to work.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cargo build` pass.

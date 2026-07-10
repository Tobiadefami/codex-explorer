# Strict TUI Keybindings Design

## Problem

The TUI currently treats an empty search query as an implicit command mode. Plain
characters such as `a`, `p`, `r`, and `1` invoke commands instead of becoming the
first character of a query. As a result, users cannot directly search for terms
such as `apple`, `review`, or `123`.

## Interaction Contract

Unmodified printable characters always belong to search, regardless of whether
the query is empty. Application commands that currently use printable characters
move into a single `Alt` shortcut namespace:

| Shortcut | Action |
| --- | --- |
| `Alt+A` | Show all projects |
| `Alt+P` | Return to the current project |
| `Alt+R` | Refresh the index |
| `Alt+I` | Run or refresh the selected session audit |
| `Alt+E` | Expand or collapse the selected row |
| `Alt+1` | Show overview preview |
| `Alt+2` | Show conversation preview |
| `Alt+3` | Show tool activity preview |
| `Alt+4` | Show timeline preview |
| `Alt+5` | Show audit preview |
| `Alt+D` / `Alt+U` | Scroll the preview down / up |
| `Alt+J` / `Alt+K` | Move selection down / up |
| `Alt+Q` | Quit |

Existing non-printable controls remain unchanged: arrow keys move selection,
`PgUp`/`PgDn` and `Home` scroll the preview, `Backspace` edits the query, `Enter`
resumes the selected session, and `Esc` quits. `Ctrl+C` also remains a quit
shortcut.

`Alt` is used consistently instead of `Ctrl` because terminal protocols encode
some control combinations as other keys; for example, `Ctrl+I` is commonly
indistinguishable from Tab. Alt-modified printable keys avoid those collisions
while keeping mnemonic command letters.

## Input Handling

The input handler will match command characters only when the key event has the
`Alt` modifier. Command matching will no longer depend on the search query being
empty. A character event with no modifiers will append that character to the
query and refresh search results. Unsupported modified character events will be
ignored, preserving current behavior.

Command actions and state transitions remain in their existing components; this
change only tightens the conditions under which the input handler invokes them.

## User-Facing Documentation

The persistent help bar and README key list will show the Alt-prefixed bindings.
The help text will continue to lead with the typing behavior so it is clear that
the search field accepts input immediately.

## Testing

Input-handler tests will establish both halves of the contract:

- Typing representative conflicting terms, including `apple` and a query that
  begins with a digit, produces the exact query without invoking commands.
- Every Alt-prefixed command still produces its expected action or state change.
- Plain `q` becomes search input, while `Alt+Q`, `Esc`, and `Ctrl+C` quit.
- Existing non-printable navigation and editing behavior remains covered by the
  current test suite.

The full Rust test suite and formatting checks will run after implementation.

## Scope

This change does not introduce a focus mode, configurable shortcuts, or a keymap
abstraction. It only resolves the ambiguity between immediate search input and
existing printable-character commands.

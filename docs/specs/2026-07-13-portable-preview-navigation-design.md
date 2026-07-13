# Portable Preview Navigation

## Problem

Some terminal emulators reserve `Alt+1` through `Alt+5` for switching terminal tabs. Those keystrokes never reach Codex Explorer, so they cannot reliably select preview modes.

## Design

Replace direct `Alt+1` through `Alt+5` preview selection with sequential navigation:

- `Tab` selects the next preview.
- `Shift+Tab` selects the previous preview.
- Navigation wraps between Overview and Audit.
- Changing previews resets the preview scroll position.
- Plain letters and digits remain exclusively available for search input.

The preview order remains Overview, Conversation, Tools, Timeline, and Audit. Existing shortcuts unrelated to preview selection remain unchanged.

## Interface Changes

Remove `Alt+1` through `Alt+5` from input handling, the in-app help line, and the README. Document `Tab` and `Shift+Tab` in their place.

## Testing

Input tests will verify forward navigation, backward navigation, wrapping in both directions, and scroll reset. Existing tests will continue to protect unrestricted search input and all unrelated shortcuts.

# 05: Cover adjustment screen

Status: needs-triage
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

- Add "ADJUST COVER" to the context menu of a book. It opens a new screen (not a dialog) for that cover.
- Controls: exposure, contrast, and the luminance of each channel (red, green, blue).
- A large preview shows the dithered cover. It updates while a control moves.
- Buttons to reset, to save and to go back. Esc goes back without a save.
- Store the values for each book in `library.json`. Apply them before the dithering (`src/dither.rs`) everywhere the cover shows.
- Books without a cover have no "ADJUST COVER" item, or it is disabled.

## Acceptance criteria

- A saved change shows on the library card at once and after a restart.
- Reset gives the cover as it was without adjustments.
- The original cover file is not changed.

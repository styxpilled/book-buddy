# 08: Library background

Status: needs-triage
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

- Replace the grid behind the library (`style::grid` in `src/style.rs`) with a dithered freeform gradient: a few soft shapes of light and dark that blend, dithered to the two theme tones with the Bayer dithering of `src/dither.rs`.
- The background stays quiet, so the covers and text are easy to read. Use a low contrast between the two tones, or a small part of dark dots.
- It works in light and dark themes and changes with the theme.
- It fills the window at any size without visible tiles. It must not slow down scroll or resize.

## Acceptance criteria

- The library shows the dithered gradient in light and dark themes.
- Scroll and window resize are as smooth as with the grid.
- Book titles and tags stay readable on all parts of the background.

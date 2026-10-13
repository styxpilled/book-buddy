# 08: Library background

Status: done
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

## Comments

- `src/backdrop.rs` replaces `style::grid` in the library and on the cover adjustment screen.
- The shapes are warped sine waves in logical pixels, so they do not repeat and do not move when the window changes size. Empty parts stay between them.
- Bayer dithering of `src/dither.rs`, 2 px dots. The dots are the background color with 12% of the text color, at most 75% of the dots in the darkest part. It follows the theme.
- One image for the app, at device pixels. It grows in steps of 128 dots when the window becomes larger and never shrinks, so most resizes make no new image. A new image is made on a background thread: 50 to 76 ms in a release build for a window up to 2048 x 1280 logical px.

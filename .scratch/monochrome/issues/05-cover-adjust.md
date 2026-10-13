# 05: Cover adjustment screen

Status: done
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

## Comments

- Books without a cover show "ADJUST COVER" disabled. A feature for these books comes later.
- Values go from -100 to 100. At the ends, a value multiplies or divides by 4: the channel weight in the luma, the luma (exposure), or the distance from mid gray (contrast).
- Second pass, for finer control:
  - Black point and white point (0 to 100) and midtones (gamma) replace exposure.
  - Eight hue sliders (red, orange, yellow, green, aqua, blue, purple, magenta) replace R/G/B. A slider changes the luma of its hue. The change scales with the chroma, so grays do not change.
  - Dither method for each cover: Bayer, Atkinson or Floyd-Steinberg.
  - COMPARE: while the button is held, the preview shows the original cover in color.
  - Order of the steps: hues, levels, midtones, contrast.
  - AUTO sets black and white to the darkest and lightest tones of the cover, after the hue sliders. The darkest and lightest 1% of the dots are ignored.
- The preview uses the same dots as the library card, at 2x size. A dot is a whole number of device pixels.
- Only books with a change have `cover_adjust` in `library.json`.
- The library card is the only place that shows the cover thumbnail.

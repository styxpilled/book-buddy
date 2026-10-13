# Monochrome look: follow-up work

Status: needs-triage

## Summary

The app uses the MONOCHROME design from issue 13 (`mockups/monochrome.html`). The first pass is in the working tree: theme `assets/themes/monochrome.json`, shared parts in `src/style.rs`, cover dithering in `src/dither.rs`. The issues here refine the library and the reader, and add cover and metadata editing.

## Rules

- Keep the look of the first pass: square shapes, 1 px ink lines, IBM Plex Mono for the UI, DotGothic16 for numbers, two-tone dithering.
- Book text uses a reading font from the FONT setting: Literata (default), Newsreader, Source Serif 4, Alegreya, EB Garamond, Inter or IBM Plex Mono.
- New settings go in `library.json` with a default, like the settings of v1.
- Minimal text in the UI.

## Issues

| # | Issue |
| --- | --- |
| 01 | [Library toolbar](issues/01-library-toolbar.md) |
| 02 | [Book cards](issues/02-book-cards.md) |
| 03 | [Reader margins and bars](issues/03-reader-bars.md) |
| 04 | [Faster images on book open](issues/04-image-loading.md) |
| 05 | [Cover adjustment screen](issues/05-cover-adjust.md) |
| 06 | [Edit book details](issues/06-edit-details.md) |
| 07 | [Poetry layout](issues/07-poetry.md) |
| 08 | [Library background](issues/08-library-background.md) |

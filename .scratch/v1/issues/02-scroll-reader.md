# 02: Scroll mode reader for one book

Status: done
Spec: [../spec.md](../spec.md) (Reader: Rendering, Scroll mode)
Depends on: 01

## Scope

- `book-buddy.exe <path>` opens a window that shows the book.
- Make the custom block renderer: headings, paragraphs with bold and italic runs, quotes, list items, pre, images, rules.
- Bundle a serif font and a sans font with an OFL license (regular, italic, bold). Use serif as the default.
- Center the text column and give it a maximum width. Use a line height of about 1.5x the font size.
- Show the whole book as one GPUI `list` with variable item heights, one item per block.
- Parse in the background. Show the first chapter as soon as it is ready.
- Scale images down to fit the column. Never scale them up.

## Acceptance criteria

- `basic-v3plus2.epub` opens and scrolls smoothly from start to end.
- Bold, italic, headings and the cover image show correctly.
- The window stays responsive while parsing runs.

## Comments

### 2026-10-11: implementation notes

- Code: `src/main.rs` (window, app setup) and `src/reader.rs` (reader view). Fonts are in `assets/fonts/` with their OFL files: Literata (serif) and Inter (sans), static regular, bold, italic and bold italic from Google Fonts. They are embedded with `include_bytes!`.
- The book opens on a background task. Chapters parse one by one in the background and are appended to the list as they are ready.
- Images: the size comes from the image header when the chapter loads. An image decodes in the background when its block is first drawn, downscaled to at most 2048 px. At most 16 images stay decoded. Older ones are released with `Window::drop_image`, because GPUI keeps decoded images until they are dropped.
- Sim tools from `findings/` are in `scripts/` and `src/sim.rs` (feature `sim`), renamed to book-buddy.
- Verified with sim on `basic-v3plus2.epub`: cover, headings, paragraphs, the em dash and the italic run at the end show correctly, and the end of the book can be reached.
- Known: GPUI line wrapping can put closing punctuation on a new line (`Fodder` / `?"`). Not fixed.

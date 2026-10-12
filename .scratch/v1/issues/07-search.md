# 07: Search in book

Status: done
Spec: [../spec.md](../spec.md) (Reader: Chrome)
Depends on: 05

## Scope

- Ctrl+F opens a search box. Search the full text of the book, case-insensitive.
- The results list shows the chapter title and some text around each match.
- Click a result to jump to it and highlight the match.
- F3 and Shift+F3 go to the next and previous match. Esc closes the search.
- Run the search off the UI thread.

## Acceptance criteria

- Search finds matches in chapters that are not on screen.
- A match that spans two runs with different styles (for example a bold word) is found.

## Comments

### 2026-10-12: implementation notes

- Code: `src/search.rs` (`find`: case-insensitive byte ranges, no overlap), `src/reader/search.rs` (search state, panel, F3 and Shift+F3), and `Block::text` in `src/book/xhtml.rs`. The reader item text now comes from `Block::text`, so `pre` blocks are searched and highlighted too.
- The search runs on the item texts of the loaded chapters, on a background task, 150 ms after the last change. At most 10000 results are kept ("First 10000 matches").
- Panel: on the right, with the input, the number of matches and a list of results (chapter title from the TOC, and up to 30 characters before and 80 after the match). While it is open, the text column is centered in the space left of the panel, so a match is never under the panel.
- Enter and F3 go to the next match, Shift+Enter and Shift+F3 to the previous one. With no current match, F3 starts at the top of the window. The current match has a `selection` background in the text and is highlighted in the list. Esc closes the panel and removes the highlight.
- The panel is outside the "Reader" key context. GPUI dispatches key bindings before text input, so inside the reader context T and Space would not reach the input. It has its own "Search" context for Esc, F3 and Shift+F3.
- The input sends a change event when Enter is pressed. A change with the same query is ignored, because a new search clears the current match.
- Unit tests: case folding (including non-ASCII), and a match that spans plain, bold and italic runs.
- Verified with sim: "taj mahal" from the start of `basic-test.epub` finds both matches (two chapters, off screen), Enter, F3, Shift+F3, click on a result, Esc. "yore shelf. debt florist" in `basic-v3plus2.epub` finds the match across the plain and italic runs.
- Known: the 3-line margin above a match only applies inside one block. A match near the end of the book shows lower in the window.

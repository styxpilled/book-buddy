# 09: Paginated mode

Status: done
Spec: [../spec.md](../spec.md) (Reader: Paginated mode)
Depends on: 05, 06, 08

## Scope

- Measure the blocks with the GPUI text system and lay them out into pages that fit the window.
- A paragraph can break between lines across two pages. An image that does not fit goes to the next page.
- A new chapter always starts on a new page.
- If the window is wide enough for two columns at the maximum width, show two pages side by side.
- Calculate the pages again after a resize (debounced) and after a font or size change. Keep the position by character offset.
- Make page keys, the seek bar, TOC, links, search and selection work in this mode.
- Enable the mode setting from issue 06. The setting is global.

## Acceptance criteria

- Switch modes, resize the window, and change the font size: the text at the saved position stays on screen each time.
- No line is cut off at the bottom of a page.
- Two pages show when the window is wide, and one page when it is narrow.

## Comments

### 2026-10-12: implementation notes

- Code: `src/reader/pages.rs` (geometry, pages, page turns, wheel), and `render_block` and `spacing` in `src/reader.rs`. Scroll mode and paginated mode use the same block element.
- Pages are made for one chapter at a time, when the chapter is shown, during render. Root views render in the prepaint phase, so `layout_as_root` can measure each block there. The measurement uses the same block element without mouse handlers.
- A page is a list of slices: a block, a y offset into it, and a height. Each slice shows the full block element, moved up and clipped. So links, selection, search highlights and images work as in scroll mode.
- A text block breaks between two lines. GPUI rounds the line height, so the line height comes from the measured height and the number of lines. The space below a block can go past the bottom of the page. Headings, images, rules and error blocks do not break: they go to the next page. A block that is taller than an empty page is cut. A heading moves to the next page if the first line after it does not fit.
- The state is a character position. The page shown is the one that holds it. A page index is kept too, because an image page has no text and starts at the same offset as the page after it. Key handlers queue page turns, and the next render applies them after the pages are made. The saved position is the start of the page after a page turn. After a jump, a mode switch or a resize, it is the exact offset.
- The offset at a page break inside a block is the part of the block height, as in scroll mode.
- Two pages show when the space left of the search panel fits two 720 px columns, a gap and the margins. A spread starts on an even page of the chapter. A chapter always starts on the left page.
- The pages are made again at once after a font or size change and when the search panel opens or closes. After a resize, the old pages stay until the size is the same for 150 ms.
- Keys: Right, PgDn and Space go to the next page or spread. Left, PgUp and Shift+Space go back. Home and End go to the first and last page of the chapter. Ctrl+Home and Ctrl+End go to the start and end of the book. Up and Down do nothing. The mouse wheel turns one page for each notch.
- Settings: the Paginated button is enabled. A mode switch keeps the position.
- Unit test: `page_at` with two pages at the same offset.
- Verified with sim on `basic-v3plus2.epub` and `basic-test.epub`: a switch to paginated mode and back keeps the text on screen. Page turns work inside a chapter and across chapters, and the cover image fits a page. Two pages show at 1700 px width and one page at 1000 px. Ctrl+= and Ctrl+- three times return to the same page. Search jumps to the page of the match and shows the highlight, and the pages are made again for the panel width. The TOC highlights the current entry and its entries jump to the correct page. A link jumps and Alt+Left goes back. A double-click selects a word and Ctrl+C copies it. Home, End, Ctrl+End, the seek bar and the wheel work. A restart opens the same page in paginated mode.
- Known: no line is cut at the bottom of a page. A widow line (the last line of a paragraph alone on the next page) can occur. The percent on the last page of the book is the percent at the start of that page, not 100%.

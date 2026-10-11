# 09: Paginated mode

Status: ready-for-agent
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

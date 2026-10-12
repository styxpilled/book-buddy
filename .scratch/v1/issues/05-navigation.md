# 05: TOC, links, seek bar, keyboard

Status: done
Spec: [../spec.md](../spec.md) (Reader: Chrome, Keyboard; Content pipeline)
Depends on: 03

## Scope

- TOC side panel with nested entries. Highlight the current chapter. Click an entry to jump to it. T toggles the panel.
- Internal links in runs jump to their anchor. Alt+Left goes back to the position before the jump.
- Top bar with the book title and chapter title. It hides while you read and shows when the mouse moves to the top edge.
- Bottom bar with the percent read and a seek bar. Click or drag on it to jump to that percent.
- The keyboard map from the spec, except search and font size (other issues).

## Acceptance criteria

- Every TOC entry in the fixtures jumps to the correct heading.
- A footnote link jumps to the note, and Alt+Left returns to the same position.
- All keys in the spec table work in scroll mode.

## Comments

### 2026-10-12: implementation notes

- Code: `src/reader.rs` (actions and key bindings in `init`, navigation methods, top bar, bottom bar, TOC panel). `Position` in `src/library.rs` is now ordered by spine, then offset.
- TOC panel: an overlay on the left, so the text column does not reflow. Labels are truncated to one line. The panel scrolls to the current entry when it opens. A click jumps and closes the panel. Esc closes the panel before it goes to the library.
- Current chapter: the last TOC entry whose start (spine and anchor offset) is at or before the position. The top bar shows the same entry.
- Links: each link run is clickable through `InteractiveText`. A link that `Book::locate` cannot find (for example `http:`) does nothing. Each jump pushes the position on a back stack. Alt+Left pops it. TOC and seek jumps do not push.
- A jump to a chapter that is not parsed yet waits until it is loaded. This uses the same field as the restore of the saved position (`pending`).
- Top bar: shows when the pointer is within 12 px of the top. It hides when the pointer moves below the bar or when the text scrolls.
- Bottom bar: the gpui-kit `Slider` (0 to 100) and the percent. It is disabled until all chapters are loaded, because the percent needs the totals.
- Keys: a page is the list height less one line. Home goes to the first block of the current chapter. End puts the end of the chapter at the bottom of the window (scroll to the next chapter, then up one screen after one layout). Ctrl+End scrolls to the end of the book.
- Unit test: every TOC entry of the fixtures (including `basic-test.epub`) lands on a heading, or on the cover image.
- Verified with sim on `basic-test.epub`: TOC open, highlight and click ("Navigation Test"), the link back to `link-target-1a` and Alt+Left to the same pixel, top bar, seek click and drag, all keys in the table for scroll mode, Esc closes the panel and then goes to the library with the position saved.
- Known: near the end of the book the list cannot put the target at the top, so the target shows lower in the window. The light theme TOC highlight (`list_active`) is faint.

# 12: Selection across blocks (stretch goal)

Status: done
Spec: [../spec.md](../spec.md) (Reader: Selection, Stretch goals)
Depends on: 08

## Scope

- Click and drag to select text across two or more blocks, in both directions.
- Double-click and drag extends by words across blocks.
- Blocks between the start and the end are fully highlighted.
- When the pointer is held near the top or bottom edge of the list, the list scrolls, and the selection continues into the blocks that come into view.
- When the pointer is over a gap, a list marker, an image or a rule, the selection end snaps to the nearest text block.
- Ctrl+C joins the selected text of the blocks with a blank line between them. Images and rules are skipped.

## Notes

- The selection becomes two endpoints, each an item index and a byte index into the item text. Item indices are stable, because chapters are only appended.
- The list only lays out items near the window, and `TextLayout::index_for_position` panics for text that was not painted. So each painted text item records its bounds and layout for that frame, and one window listener during the drag picks the item under the pointer from those records.
- Auto-scroll needs a timer while the button is down near an edge.
- List markers (bullets, numbers) are not in the item text. Decide if copy adds them.
- Paginated mode (09) can use the same endpoints.

## Acceptance criteria

- Select from the middle of one paragraph to the middle of a later paragraph, copy, and paste: the text is correct, with one blank line between the paragraphs.
- A drag past the bottom edge scrolls the list and extends the selection.
- Selection does not cause visible lag while scrolling.

## Comments

### 2026-10-13: implementation notes

- Code: `src/reader/selection.rs`. The selection has two carets, an anchor and a head. Each caret is an item index and a byte index into the item text. The highlight of an item is the part between the two carets. Items between them are fully highlighted.
- During a drag, each painted text item records its item index, its layout and its visible bounds (the text bounds inside the clip) for that frame. `Reader::render` clears the records. One zero-size `canvas` in the reader root puts a mouse-move listener on the window. The listener picks the record nearest to the pointer, clamps the pointer into its visible bounds and gets the index. So a pointer over a gap, a list marker, an image, a rule or a bar snaps to the nearest text.
- GPUI gives index 0 for a point on the bottom edge of a text, so the pointer is clamped 1 px inside.
- Auto-scroll (scroll mode only): when the pointer is less than 40 px from the top or bottom of the list, or outside it, a task scrolls the list every 16 ms. The speed grows with the distance, to at most 40 px per tick. Each tick extends the selection with the records of the last frame, then scrolls. The task stops when the pointer leaves the zone or the button goes up.
- Paginated mode uses the same carets. A drag works across the slices and both pages of a spread. A drag does not turn pages.
- Double-click and drag extends by words across blocks. The first word always stays selected.
- Copy: the selected part of each block, joined with a blank line. Images and rules have no text and are skipped. A list item gets its marker ("• " or "3. ") when the selection has the start of the item and more than one block. A selection in one block copies only the text, as before.
- With hidden bars, the bars do not show during a drag.
- Unit test: `range_across_items`.
- Verified with sim on `pg1342-images-3.epub`: a drag from the middle of a paragraph to the middle of a paragraph two blocks later copied the text with one blank line between the paragraphs (paginated mode). In scroll mode, a hold at the bottom edge scrolled past an image and extended the selection into the caption below it. A hold at the top edge scrolled up and extended the selection. A drag that ends over an image selects up to the end of the paragraph above it. Double-click on "Darcy" and drag forward copied "Darcy only can answer.” ... I shall", and back copied "Darcy mean,” ... Mr. Darcy".
- Not verified: copy of list items with markers. Lag: not measured. The extra work per frame is one record for each painted text item, and only during a drag.

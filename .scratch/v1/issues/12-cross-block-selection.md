# 12: Selection across blocks (stretch goal)

Status: needs-triage
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

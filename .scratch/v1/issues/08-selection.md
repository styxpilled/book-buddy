# 08: Text selection and copy

Status: done
Spec: [../spec.md](../spec.md) (Reader: Selection)
Depends on: 02

## Scope

- Click and drag to select text inside one block. Double-click selects a word.
- Ctrl+C copies the selected text to the clipboard.
- A drag that leaves the block keeps the selection inside the start block.
- A click on a link does not start a selection.

## Acceptance criteria

- Select part of a paragraph that has bold and italic runs, copy it, and paste it: the text is correct.
- Selection does not cause visible lag while scrolling.

## Comments

### 2026-10-12: implementation notes

- Code: `src/reader/selection.rs` (selection state, the text element of an item, mouse handling, copy, word bounds). The text element of headings, paragraphs, list items and `pre` blocks is now built there, with the search and selection highlights and the link clicks.
- The selection is an item index and two byte indices into the item text. A press on any part of the reader clears it (capture phase on the reader root). A press on text then starts a new one.
- `TextLayout::index_for_position` panics for text that was not painted, so it is only used in listeners of painted text. Mouse down is on the text element. During a drag, the selected item adds a zero-size `canvas` that puts a mouse-move listener on the window in each paint. So the drag continues when the pointer leaves the block, and the index stays inside the block (above it is the start, below it is the end).
- Double-click selects a word: letters, digits, `_` and apostrophes. Any other char is selected alone.
- Double-click and drag extends the selection by whole words in both directions. The first word always stays selected.
- A press on a link does not start a selection, so the link click still works.
- Ctrl+C (`CopySelection`, "Reader" context) copies the selected text. In the search box, Ctrl+C goes to the input.
- The search match highlight is now `warning` at 35% opacity, so it is different from the selection colour.
- Verified with sim: a drag across a plain and an italic run in `basic-v3plus2.epub` copied "...entity florist oil bar yore shelf. Debt florist's mush " exactly as shown. A drag up out of the paragraph stops at its start. A double-click on "HORMONE" copied "HORMONE". Double-click on "HORMONE" and drag forward copied "HORMONE BANG! ... are dun wander", and back copied "furrier! ... worsen HORMONE". A click clears the selection. A drag on a link selects nothing and the link still jumps. The selection stays in place while the list scrolls.
- Sim: `down:x,y,2` in `src/sim.rs` sends a held second press of a double-click.
- Lag: not measured. The extra work per frame is one highlight range and, only during a drag, one listener.

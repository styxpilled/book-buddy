# 02: Book cards

Status: done
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

- Show the progress (squares and percent) on all books, also on new books (0%).
- Put the progress on the cover, at the bottom left.
- Put the "NEW" tag on the cover, at the bottom right.
- Show the progress and the "NEW" tag only while the pointer is on the book.
- The "FILE MISSING" and "HIDDEN" tags stay visible.
- Make the clickable area match what you see. Now each card has a fixed height (`COVER_HEIGHT + CARD_TEXT_HEIGHT` in `src/library_view.rs`), so a click on the empty space under a short title opens the book. Only the cover and the text lines must open the book and show the context menu.

## Acceptance criteria

- Without hover, a card shows only the cover, title and author.
- With hover, progress is on the bottom left of the cover and "NEW" (new books only) is on the bottom right.
- A click below the last text line of a card does nothing.
- The grid rows keep the same height for all cards.

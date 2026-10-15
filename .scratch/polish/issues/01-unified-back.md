# 01: Unified back

Status: done
Depends on: none

## Scope

One "back" goes to the previous state: the previous view, the previous
navigation (a link or a search result), or the previous book. The mouse back
button and `alt-left` both do it.

## Current behavior

- Escape dispatches `Back`. It closes the top layer: a popup menu, settings,
  the details dialog, cover adjust, search, the contents, and then the book
  (`reader.rs` `back`).
- The mouse back button sends an Escape keystroke (`shell.rs` `back_button`).
  Thus it closes the search panel, and does not go back from a link.
- `alt-left` dispatches `GoBack` in the reader only. It goes back through the
  link history (`Reader::back`). A link records a position. A search result,
  a contents entry and the seek bar do not record one.
- When a book closes, its link history is lost. Nothing opens the previous book.

## Wanted behavior

One linear history for the window, as in a browser. An entry is a view (the
library or a book) or a position in a book.

Back (mouse back button, `alt-left`), in order:

1. If an overlay is open (popup menu, settings, details, search, contents),
   close it. This does not change the history.
2. Go to the previous entry: a previous position in the book, the previous
   book, or the library.
3. In the library with no previous entry, do nothing.

Forward (mouse forward button, `alt-right`) goes to the next entry. It can
open a book again after back closed it.

These add a new entry and clear the forward entries:

- A link, a search result, a contents entry, and the seek bar (one entry for
  each drag, not for each move). The entry is the position before the jump.
- Opening a book (from the library or from a later start).
- Escape that closes a book. The library is a new entry, so back in the
  library opens that book again.

Escape keeps its current behavior: it closes only the top layer.

The positions of a book are dropped when the book closes. A book that opens
again from the history opens at its saved position, with no positions in its
history.

## Decisions

- Back from a book goes to the view that opened it. A chain of books comes
  only from later starts that open a book while a book is on screen.
- The history is not saved to disk.

## Acceptance criteria

- The mouse back button and `alt-left` do the same thing in each view.
- After a link, a search result, a contents entry or a seek, back returns to
  the position before the jump, and forward returns to the jump.
- Back from a book shows the view that opened it. Forward opens the book again.
- After Escape closes a book, back in the library opens it again.
- A new jump clears the forward entries.
- Escape still closes only the top layer.
- The sim can check each case (`scripts/sim.ps1`: `bclick`, a new `fclick`, and
  `kdown:18`, `kdown:37`, `kdown:39`).

## Comments

Implementation: two layers that act as one history.

- `Reader` keeps the positions of the open book (`back`, `forward`). A jump
  calls `record_jump`, which emits `ReaderEvent::Jumped`, so `Shell` clears
  its forward views. With no position to go to, `Reader` emits
  `ReaderEvent::HistoryBack` or `HistoryForward`.
- `Shell` keeps the views (`View::Library`, `View::Book(path)`). Back closes
  settings, details or cover adjust first.
- A search records one position for each time the search opens, at the first
  result shown. Steps between results do not record.
- The mouse buttons dispatch `HistoryBack` and `HistoryForward`. A popup menu
  closes on back.

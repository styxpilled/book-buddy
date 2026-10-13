# 06: Edit book details

Status: needs-triage
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

- Add "EDIT DETAILS" to the context menu of a book. It opens a modal dialog.
- Fields: title and author. More fields can come later.
- Buttons to save and to cancel. Enter saves, Esc cancels.
- Store the edited values in `library.json` as overrides. The EPUB file is not changed.
- A rescan of the watched folders must not replace the overrides.
- The library card, the filter, the sort and the reader top bar use the edited values.

## Acceptance criteria

- An edited title shows on the card and in the reader top bar, and stays after a restart and a rescan.
- Filter and sort by title use the edited title.
- An empty field goes back to the value from the book.

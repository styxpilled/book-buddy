# 06: Edit book details

Status: done
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

## Comments

- The overrides are `custom_title` and `custom_author` on each book in `library.json`. A book without an edit does not have them.
- A scan writes only `title` and `author`, so the overrides stay.
- A value that is empty, or the same as the value from the file, removes the override.
- An empty field shows the value from the file as its placeholder.
- The cover adjustment screen also shows the edited title.

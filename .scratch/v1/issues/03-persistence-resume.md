# 03: Library JSON, save position, resume

Status: done
Spec: [../spec.md](../spec.md) (Data and storage, Startup)
Depends on: 02

## Scope

- Read and write `book-buddy-data/library.json` next to the exe. Write to a temp file, then rename it.
- Add a `version` field to the file.
- Store `size` and `modified` for each book, for the hash cache (see spec).
- Store the position as `{ spine, offset }`, where offset is a character offset into the plain text of the chapter.
- Calculate the percent read from the character totals.
- Save the position 1 s after the last scroll, when the user leaves the book, and when the app closes.
- On startup, open `last_book` at its saved position.
- Opening a path from the command line adds the book to the library if it is not there.

## Acceptance criteria

- Close the app at a position and start it again: the same text is at the top.
- If you kill the process during a write, the JSON file is still valid.
- If `library.json` is not there, the app starts with an empty library.

## Comments

### 2026-10-11: implementation notes

- Code: `src/library.rs` (`Library`, `BookEntry`, `Position`, load, save, open), `src/store.rs` (the app global, data folder next to the exe) and the position code in `src/reader.rs`.
- Save: the file is written to `library.json.tmp`, synced, then renamed over `library.json`. A file that is not valid JSON is moved to `library.json.bad` and the app starts with an empty library. If the file cannot be read for another reason, the app does not save, so the file is not overwritten.
- Position: the start of the top block, plus the scrolled part of the block height as a part of its characters. Restore scrolls to the block, then to the same part of its height after it is measured.
- The save runs 1 s after the last scroll and on app quit. "When the user leaves the book" comes with the library view (issue 04).
- Chapters still parse in spine order. The reader shows "Loading..." until the chapter at the saved position is ready. The spec is updated.
- `author` is all creators, joined with ", ".
- Unit tests: missing file, save and load, invalid file, matching by path and then by ID. 19 tests pass.
- Verified with sim: scroll to the middle of a paragraph, wait, kill the process, start with no arguments. The same text is at the same place.

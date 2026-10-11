# 04: Library view

Status: done
Spec: [../spec.md](../spec.md) (Library)
Depends on: 03

## Scope

- Grid of covers with title, author and percent read. Books with no cover get a placeholder that shows the title.
- Generate cover thumbnails at about 2x the display size and cache them in `book-buddy-data/covers/`.
- Sort by last opened (default), title, author or date added.
- Filter box that matches the title or author as you type.
- Context menu with Hide, Remove and Show in Explorer. A toggle shows hidden books.
- Watched folders: scan them recursively on startup, in the background. The view updates as the scan finds books.
- Import with drag-and-drop and Ctrl+O. Store the path only. Do not copy the file.
- Match scanned files to entries by path first, then by book ID. Calculate the hash again only when the path, size or modified time changed.
- Missing files: keep the entry, show it greyed out, and relink it when a scan finds the same book ID.
- A changed file at the same path keeps its entry and position and gets the new ID.
- Remove on a book that is in a watched folder works as Hide.
- Esc in the reader goes to the library. Click a book to open it.
- If `last_book` is missing, startup shows the library.

## Acceptance criteria

- The library shows its stored books at once, before the scan ends.
- Move a book file out of the watched folder: it shows as missing. Move it back: it is relinked and keeps its position.
- Remove does not delete the file.

## Comments

### 2026-10-11: implementation notes

- Code: `src/shell.rs` (root view: library or reader), `src/library_view.rs` (toolbar and grid), `src/scan.rs` (folder scan, import, cover thumbnails), `src/image_cache.rs` (image cache with a size limit), and `hidden`, `missing`, `folders`, `add`, `unchanged`, `update_missing`, `remove` in `src/library.rs`.
- Grid: `uniform_list` of rows. The number of covers in a row comes from the window width. Covers are 160 x 240, and thumbnails are 320 x 480 PNG files in `book-buddy-data/covers/` (`fnv1a-<hash>.png`).
- Covers are drawn through `BoundedImageCache`, which keeps at most 150 decoded covers and drops the least recently used one.
- Scan: on startup, mark missing files, walk the watched folders, open only files that are new or changed (path, size, modified time), then mark missing files again. Each found book is saved at once, so the grid updates during the scan.
- Matching by ID only relinks an entry whose file is gone. Two copies of one book are two entries. The spec is updated.
- Books opened from the command line also get a cover thumbnail.
- Watched folders have no UI until issue 06. Until then, add them to `folders` in `library.json`.
- Verified with sim: scan of a folder with a subfolder, open a book and press Esc (position saved, card shows 4%), context menu, Hide, Show hidden, filter by author, a moved file shows "File missing" and is relinked when it comes back, drag-and-drop import of a copy, resume of the last book at startup.
- Not verified in sim: Ctrl+O (native file dialog) and Show in Explorer (starts Explorer).
- Known: when a book file changes, the old cover thumbnail stays in `covers/`.

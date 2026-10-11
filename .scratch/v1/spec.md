# book-buddy v1 spec

Status: needs-triage

## Summary

book-buddy is a lightweight EPUB reader for personal use on Windows. It is written in Rust with GPUI. It does not use a web view. It keeps a library of books and remembers the reading position in each book. It ignores all styling from the book.

## Goals

- Open fast and resume the last book at the saved position.
- Keep a library from watched folders and manual imports.
- Show book text with basic emphasis, images, and internal links.
- Support continuous scroll and paginated modes.
- Use few dependencies and a small code base.

## Non-goals

- Book CSS, embedded fonts, or publisher layout.
- Annotations, highlights, bookmarks, or notes.
- Tables (show table cell text as plain paragraphs).
- Formats other than EPUB 2 and EPUB 3.
- DRM.
- Sync between machines.
- An installer.
- A hard startup time target. The app must feel fast.

## Tech stack

| Concern | Choice | Notes |
| --- | --- | --- |
| UI | `gpui-kit` 0.6.1 with `gpui-pre` 0.3.1 | Pin exact versions (`=`). Upgrade on purpose only. This pair works on Windows with the sim tools in `findings/`. `gpui-component-macros` is pinned to 0.6.1 because 0.6.6 breaks the build. |
| EPUB container, OPF, TOC | `rbook` | EPUB 2 and 3. Verify that it parses the EPUB 3 `nav.xhtml`. If not, parse `nav.xhtml` ourselves. |
| Chapter XHTML | `quick-xml` | Fall back to `scraper` (html5ever) when the XHTML is not valid XML. |
| Persistence | `serde` + `serde_json` | One JSON file. Write to a temp file, then rename. |
| Images | `image` | Cover thumbnails and inline images. |
| Windows APIs | `windows` crate | Single instance, file association, open in Explorer. |

Rationale and risks for GPUI are in [Risks](#risks).

## Data and storage

The app is portable. All data lives next to the executable:

```
book-buddy.exe
book-buddy-data/
  library.json
  covers/<book-id>.png
```

### `library.json`

```jsonc
{
  "version": 1,
  "folders": ["D:\\Books"],
  "settings": {
    "mode": "scroll",        // "scroll" | "paginated"
    "font": "serif",         // "serif" | "sans"
    "font_size": 18,
    "theme": "dark"          // "light" | "dark"
  },
  "window": { "x": 0, "y": 0, "w": 1200, "h": 900, "maximized": false, "fullscreen": false },
  "last_book": "<book-id>",
  "books": [
    {
      "id": "<book-id>",
      "path": "D:\\Books\\foo.epub",
      "size": 712848,
      "modified": "2026-10-01T09:30:00Z",
      "title": "Foo",
      "author": "Bar",
      "added": "2026-10-11T12:00:00Z",
      "last_opened": "2026-10-11T12:00:00Z",
      "position": { "spine": 4, "offset": 1832 },
      "percent": 37.2,
      "hidden": false,
      "missing": false
    }
  ]
}
```

- **Book ID:** an FNV-1a hash of the file contents (`fnv1a:<16 hex digits>`). The OPF `dc:identifier` is not used, because many books share placeholder identifiers.
- **Hash cache:** `size` and `modified` are the file size and modified time when the ID was calculated. A scan calculates the hash again only for a file that is not in the library with the same path, size and modified time.
- **Position:** the spine index and a character offset into the plain text of that chapter. This value does not change when the font, the window size, or the mode changes. In scroll mode, the offset is the start of the top block, plus the scrolled part of the block height as a part of its characters. When the layout is the same, a restore goes back to the same pixel.
- **Percent:** the number of characters before the position, divided by the total characters in the book. Calculate it when the position is saved, after all chapters are parsed, and store it for the library view.
- **Save position:** save 1 s after the last scroll or page turn, when the user leaves the book, and when the app closes.

## Library

### Sources

- **Watched folders:** scan them recursively for `*.epub` on startup, in the background. The library shows the stored data at once and updates as the scan finds changes.
- **Manual import:** use drag-and-drop onto the window, or Ctrl+O (file dialog). The app stores the path. It does not copy the file.
- **Explorer:** see [Windows integration](#windows-integration).

### Missing files

- If a stored path does not exist, set `missing: true`. Keep the entry and its position. Show the entry greyed out.
- A scan matches a file to an entry by path first. It matches by book ID only when no entry has that path, and only an entry whose file is gone. So two copies of one book are two entries.
- If a scan finds a file with the book ID of a missing entry, update `path` and set `missing: false`.
- If a tool changes the file contents (for example, metadata edits), the ID changes. At the same path, the entry keeps its position and gets the new ID. A file that is changed and moved is a new book.

### Library view

- Grid of cover thumbnails, with title, author and percent read below each cover.
- If a book has no cover, show a placeholder with the title.
- Sort by: last opened (default), title, author, date added.
- Use a filter box to match the title or author as you type.
- Context menu on a book:
  - **Hide:** sets `hidden`. A toggle in the view shows hidden books.
  - **Remove:** deletes the entry. The file stays on disk. A book that is in a watched folder comes back on the next scan, so for those books Remove works as Hide.
  - **Show in Explorer.**
- Generate cover thumbnails once, at about 2x the display size, and cache them in `covers/`.

## Content pipeline

1. Open the EPUB and read the OPF metadata, the spine and the TOC.
2. For each spine item, parse the XHTML into a list of blocks. Ignore `<style>`, `<script>`, `<link>`, and `style`/`class` attributes.
3. Do this on a background thread, one chapter at a time, in spine order. Show the book when the chapter at the saved position is ready. Parsing is fast (milliseconds for each chapter), so this is simpler than parsing the saved chapter first.

### Block model

```rust
struct Chapter {
    blocks: Vec<Block>,
    anchors: HashMap<String, usize>, // id -> offset in this chapter
    len: usize,
}

struct Block {
    offset: usize, // start of the block in the chapter text
    quote: u8,     // blockquote depth
    kind: BlockKind,
}

enum BlockKind {
    Heading { level: u8, runs: Vec<Run> },
    Paragraph { runs: Vec<Run> },
    ListItem { depth: u8, marker: Option<Marker>, runs: Vec<Run> },
    Pre { text: String },
    Image { href: String, alt: String },
    Rule,
    Error { message: String },
}

struct Run {
    text: String,
    bold: bool,
    italic: bool,
    link: Option<String>, // absolute href, resolved with Book::locate
}
```

- The block list is flat. A block quote is a `quote` depth on each block, so that the reader can show one list item per block.
- Offsets count the characters of the block text, plus one separator after each block. Images and rules have no text.
- Mapping: `h1`-`h6` to Heading. `p`, `div` text and table cells to Paragraph. `li` to ListItem (`marker` is `None` for the second and later paragraphs of one item). `pre` to Pre. `img`, and `image` inside SVG, to Image. `hr` to Rule. `b`/`strong` to bold. `i`/`em`/`cite`/`dfn`/`var` to italic. `a href` to link. `br` to a line break in the run text.
- Collapse whitespace the same way HTML does, except in `pre`.
- Map `&#128;`-`&#159;` to Windows-1252 characters, as browsers do. Books use `&#151;` for an em dash.
- Record each `id` attribute, and `a name`, as an anchor in its chapter. The spine index and the anchor offset give the position. Links and TOC entries use these anchors.
- Footnote links jump to the target. Alt+Left goes back to the position before the jump.

## Reader

### Rendering

- One custom block renderer serves both modes. We do not use the `TextView` from gpui-component, because paginated mode needs the measured line positions.
- The text column has a fixed maximum width and is centered. Line height is fixed at about 1.5x the font size.
- Fonts: bundle one serif and one sans-serif font with an OFL license (for example Literata and Inter, regular and italic plus bold). The setting chooses between serif and sans.
- Images scale down to fit the column width and the page height. They never scale up.

### Scroll mode

- The whole book is one continuous vertical list. There is one list item per block, and chapters follow each other. Use the GPUI `list` with variable item heights so that only the blocks on screen are laid out.
- The position is the first block visible at the top, plus a character offset into it.

### Paginated mode

- Lay out the chapter blocks into pages that fit the window. A paragraph can break between lines across two pages. An image goes to the next page if it does not fit.
- If the window width fits two columns at the maximum width, show two pages side by side.
- Calculate the pages again after a resize (debounced) and after a font or size change. Keep the position by character offset.
- A new chapter always starts on a new page.

### Selection

- Click and drag to select text inside one block. Double-click selects a word. Ctrl+C copies the selection.
- Selection across blocks is not supported.

### Chrome

- **Top bar:** book title and chapter title. It hides while you read and shows when the mouse moves to the top edge.
- **Bottom bar:** percent read and a seek bar. Click or drag on it to jump to that percent.
- **TOC panel:** a side panel with the nested TOC. The current chapter is highlighted. Click an entry to jump to it.
- **Search (Ctrl+F):** search the full text of the book, case-insensitive. The results list shows the chapter and some text around each match. Click a result to jump to it and highlight the match. F3 and Shift+F3 go to the next and previous match.

### Keyboard

| Key | Action |
| --- | --- |
| Right, PgDn, Space | Next page, or scroll down one screen |
| Left, PgUp, Shift+Space | Previous page, or scroll up one screen |
| Down / Up | Scroll by one line (scroll mode only) |
| Home / End | Start / end of chapter |
| Ctrl+Home / Ctrl+End | Start / end of book |
| T | Toggle the TOC panel |
| Ctrl+F, F3, Shift+F3 | Search |
| Alt+Left | Go back after a link jump |
| Ctrl+= / Ctrl+- | Font size |
| F11 | Fullscreen |
| Esc | Close the panel or search. Otherwise, go back to the library. |
| Ctrl+O | Import a file |

## Settings

Show settings in a small panel that opens from the library and the reader:

- Mode: scroll or paginated (global).
- Font: serif or sans.
- Font size.
- Theme: light or dark.
- Watched folders: add or remove.
- Button: register as the handler for `.epub` files.

## Startup

1. Read `library.json`.
2. Restore the window position and size.
3. If `last_book` exists and its file is present, open it at the saved position. If not, show the library.
4. Start the folder scan in the background.

## Windows integration

- **File association:** the settings button writes the ProgID and the `.epub` association under `HKCU\Software\Classes`. No admin rights are necessary. The registry entries contain the current path of the exe, so press the button again if you move the exe.
- **Opening a file:** `book-buddy.exe <path>` adds the book to the library if it is not there, then opens it.
- **Single instance:** use a named mutex. A second instance sends its path to the first instance through a named pipe, and then exits. The first instance brings its window to the front and opens the book.
- **Window state:** save the position, size, maximized and fullscreen state when the app closes. Restore them on startup. Do not restore a position that is off-screen.

## Stretch goal

- Cover thumbnails as braille or ASCII art, for a terminal look in the library grid. Make the cover image grayscale, apply dithering, and map each 2x4 pixel cell to a braille character. This is a library view option only. It is not in the first release.

## Risks

1. **`gpui-pre` makes breaking changes each week.** Pin exact versions of `gpui-kit` and `gpui-pre` and upgrade on purpose.
2. **GPUI has little documentation.** Learn it from the source and from the `gpui-kit` examples (`large-text`, `text_selection`, `html`).
3. **Paginated layout needs text measurement through the GPUI text system.** This is the hardest part. Build scroll mode first.
4. **DX11 renderer on Windows.** The app needs a GPU with DX11 support. There can be problems with DPI and with GPU drivers.
5. **Malformed EPUBs.** The XHTML parser has a fallback, and a chapter that fails to parse shows an error block. The rest of the book still opens.

If GPUI becomes blocked, the fallback is `iced`.

## Milestones

1. Parse EPUB into blocks. Use the files in `books/` for unit tests.
2. Window with scroll mode reader for one book given on the command line.
3. Library JSON, position save and resume.
4. Library view: grid, covers, sort, filter, hide and remove, folder scan, missing files.
5. TOC panel, links, back navigation, seek bar, keyboard map.
6. Settings: font, size, theme.
7. Search.
8. Selection and copy.
9. Paginated mode with two-page spread.
10. Windows integration: single instance, file association, window state.
11. Stretch goal: braille covers.

## Test fixtures

`books/` contains `minimal-v2.epub`, `minimal-v3.epub`, `minimal-v3plus2.epub` and `basic-v3plus2.epub`. They cover EPUB 2 only (NCX), EPUB 3 only (nav), and EPUB 3 with an EPUB 2 fallback. `basic-v3plus2.epub` also has a cover image, CSS and an embedded font. Use it to test covers and to confirm that the app ignores the CSS and the font.

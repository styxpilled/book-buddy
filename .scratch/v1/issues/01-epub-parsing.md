# 01: Parse EPUB into blocks

Status: done
Spec: [../spec.md](../spec.md) (Tech stack, Content pipeline)
Depends on: none

## Scope

- Create the Cargo project. Pin exact versions of `gpui-kit` and `gpui-pre`.
- Open an EPUB with `rbook`: read the metadata (title, author), the spine, the TOC and the cover.
- Confirm that `rbook` parses the EPUB 3 `nav.xhtml`. If it does not, parse `nav.xhtml` ourselves.
- Parse the XHTML of each chapter into `Vec<Block>` with `quick-xml`. Fall back to `scraper` if the XML is not valid.
- Collapse whitespace. Ignore `style`, `script`, `link`, and the `style`/`class` attributes.
- Record anchors: `id` to `(spine, offset)`.
- Calculate the book ID: a hash of the file contents.
- A chapter that fails to parse becomes one error block.

## Acceptance criteria

- Unit tests pass for all four files in `books/`: metadata, spine length, TOC entries, block types and runs.
- The TOC comes from the NCX for `minimal-v2.epub` and from the nav for `minimal-v3.epub`.
- The tests show that the CSS and the embedded font in `basic-v3plus2.epub` have no effect on the blocks.
- No UI code in this issue.

## Comments

### 2026-10-11: implementation notes

- Code: `src/book/mod.rs` (`Book`, `TocEntry`) and `src/book/xhtml.rs` (block model, parser). 14 unit tests pass.
- Versions are those from `findings/README.md`: `gpui-kit` 0.6.1 and `gpui-pre` 0.3.1. `gpui-component-macros` is pinned to 0.6.1, because 0.6.6 breaks the build of `gpui-component` 0.6.1.
- `rbook` parses both the EPUB 3 nav and the NCX. No custom nav parser was necessary. When a book has both, `rbook` keeps only the nav (the default `retain_variants(false)`).
- The block model is flat. A block quote is a `quote` depth on each block. The spec is updated.
- Anchors are stored for each chapter (`id` to offset). With the spine index, they give `(spine, offset)`.
- All four fixtures have the same `dc:identifier`. Many real books also share placeholder identifiers. Decision: the book ID is an FNV-1a hash of the file contents (`fnv1a:<16 hex digits>`). The spec is updated, and issues 03 and 04 have the path matching and hash cache.

# Refactor proposals: summary

Sources: `proposals/01` to `proposals/06`. IDs are `<report>-P<n>`, for example
`03-P1` is P1 in `proposals/03-book-images.md`.

## Conflicts and joins

- `03-P1` puts `render_image` in `image_cache.rs`. `06-P4` makes `image_cache`
  private to `library_view`, but `reader.rs` must also use the helper. Put
  `render_image` in `ui/` (for example `ui/image.rs`, after `06-P3`). Use it
  in all four places, together with the `reader.rs` copy.
- `03-P4` (filter not optional) and `04-P3` (`cover_cache` helper) change the
  same two call sites. Do `03-P4` first, then `04-P3` without `Some(...)`.
- `02-P2` (quote gap helper) covers 2 of 3 copies. A `quote` field in
  `Spacing` covers all 3 (`reader.rs:1233` also). Use the field and keep the
  float order the same.
- The one-column width is in `pages.rs` and `reader.rs`. Use one helper,
  after `01-P3` and `01-P4`.
- `01-P1` changes `shell.rs:64` (one line). It does not conflict with `05-P2`.
- A new candidate from `04`: the underlined input has 3 copies
  (`library_view.rs`, `details.rs`, `reader/search.rs`). Use a helper in
  `ui/style`.

## Order

### 1. Tests first
- `03-P2` dither output hash test
- `04-P1` step 1: sort order test

### 2. Pure module moves (one commit each, no edits)
- `06-P1` `platform/` (association, instance, sim)
- `06-P3` `ui/` (menu, style, title_bar)
- `06-P4` `library_view/` (backdrop, cover_adjust, details, image_cache)
- `01-P3` `reader/images.rs`
- `01-P4` `reader/toc.rs`, `reader/bars.rs`

### 3. Dead code and duplicates
- `01-P1` dead no-book and "Loading..." path
- `05-P1` `enabled` flag of `choice`
- `05-P2` settings panel and subscription in one `Option`
- `03-P1` `render_image` helper (see conflicts)
- `01-P2` `Picture::reset`, `drop_images`
- `06-P2` shared named-pipe helper
- `03-P4`, `03-P5`, `04-P3` cover cache
- `04-P2` remove `Library::open`
- `04-P4` `Store::remove_book`
- `06-P5` association `get` and command
- `01-P5`, `02-P2` (as `Spacing` field), `02-P3`, `02-P4`, `03-P6`, `04-P6`, `05-P4`, `05-P5`, `06-P6`

### 4. Performance
- `03-P3` dither output by rows (needs `03-P2`)
- `04-P1` `sort_by_cached_key`, no eager clone
- `02-P1` link hrefs at click, not at render
- `03-P7` `layout` counts without a `String` per paragraph
- `05-P3` remove the first theme install at startup

### 5. Optional
- `04-P5` grid keeps indices, not clones. Medium risk: an index can go stale
  if the view is cached later.
- `01-P6` remove the extra `cx.notify()` calls. Depends on gpui internals.
  The value is low.

## Suspected bugs (not part of the refactor)

Confirmed by reading the code:
- `reader/search.rs:52`: panic. Enter or F3 during the search debounce sets
  `current` on the old results. The new results do not reset it.
- `reader.rs:569`: panic at open. `self.pictures[href]` has no entry for an
  image that did not read or could not be probed (for example SVG).

Not confirmed yet:
- `01` B2: a tone change during a decode starts extra decodes and leaks one image.
- `02`: `find` gives an empty range on a lowercase expansion (`İ`). A search
  during load does not include chapters that load later.
- `03`: the cover adjust preview ignores the tones. `line_break` keeps a
  trailing space across a style change.
- `04`: `upsert` leaves the old cover file when the ID changes. `in_folder` and
  `same_path` differ in case outside Windows.
- `05`: a book from a later start does not close the overlays. A slider
  preview can stay if the panel closes during a drag.
- `06`: after a failed send, two processes write `library.json`. The path is
  sent lossy. The mutex is per session but the pipe is per machine.

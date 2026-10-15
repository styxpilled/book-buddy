# Scope 2: reader parts

Files: `src/reader/pages.rs`, `src/reader/search.rs`, `src/reader/selection.rs`, `src/search.rs`.

The four files are coherent and each has one responsibility. The proposals are
small. There is no file split to propose.

### P1: Look up the link href on click, not at each render
- Files: `src/reader/selection.rs:93-108`
- Problem: `text_element` runs for each text block on screen in each frame. For
  a block with links, it collects `links: Vec<(Range<usize>, String)>`, which
  clones each href `String` (`selection.rs:93-97`), and then a second
  `link_ranges` vector from it (`selection.rs:98`). The hrefs are read only in
  the click handler (`selection.rs:105`). Footnote-heavy books have many linked
  blocks on screen.
- Change: Build only `link_ranges` from `item.styles`:
  `item.styles.iter().filter(|(_, run)| run.link.is_some()).map(|(range, _)| range.clone()).collect()`.
  Test `link_ranges.is_empty()` in place of `links.is_empty()`. In the
  `on_click` closure, find the href in the reader:
  `view.update(cx, |reader, cx| { let Some(href) = reader.items[ix].styles.iter().filter_map(|(_, run)| run.link.as_ref()).nth(link).cloned() else { return }; reader.follow(&href, cx) })`.
  The clone moves out of the render path into the click. It is necessary
  because `follow` takes `&mut self`.
- Parity: the filter order is the same as before, so index `link` gives the
  same href. Items are only appended (`reader.rs:532-547`), so `ix` names the
  same item at click time. The element tree, the ids and the mouse handlers do
  not change.
- Performance: better. Render path: one `Vec` and one `String` clone per link
  go away for each linked block per frame. Click path: one `String` clone added.
- Verify: `cargo build --features sim`. Sim: open a book with a footnote link,
  `shot:before`, `click` on the link, `shot:after-link`, `bclick` (go back),
  `shot:back`. Screens must be equal to the screens of the build before the
  change. Also drag-select across a link and check that a press on a link does
  not start a selection.
- Size: S
- Depends on: none
- Value: medium. Risk: low.

### P2: One helper for the gap under a quote block
- Files: `src/reader/pages.rs:277-281`, `src/reader/pages.rs:501-505`
- Problem: the same expression is in `estimated_height` and in `measure`:
  `if self.items[ix].block.quote > 0 { self.font_size * QUOTE_GAP } else { 0. }`.
- Change: add `fn quote_gap(&self, ix: usize) -> f32` to the `impl Reader`
  block in `pages.rs` with that body. Use it at both places. In
  `estimated_height`, `size` is `self.font_size`, so the value is the same.
- Parity: the same product `font_size * QUOTE_GAP`, then the same sums in the
  same order (`spacing.top + content + spacing.bottom + quote`,
  `height - spacing.bottom - quote`). Page breaks and the page count do not
  change.
- Performance: no effect.
- Verify: `cargo test`, `cargo build --features sim`. Sim: paginated mode, a
  book with block quotes, `shot` of a spread with a quote at the page bottom,
  before and after; also the page number in the bottom bar.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P3: One accessor for the shown pages
- Files: `src/reader/pages.rs:140-144`, `src/reader/pages.rs:535-538`
- Problem: `top_item` and `render_pages` both resolve
  `self.pages.page` through `self.pages.chapters.get(&spine)` with the same
  closure shape.
- Change: add to `impl Pages`:
  `fn shown(&self) -> Option<(&[Page], usize)> { let (spine, index) = self.page?; Some((self.chapters.get(&spine)?, index)) }`.
  In `top_item`: `self.pages.shown().and_then(|(pages, index)| pages.get(index)).and_then(|page| page.slices.first())`.
  In `render_pages`: `let pages = self.pages.shown();` (the later
  `pages.get(index + side)` works on a slice too).
- Parity: the same lookups. No change to layout or input.
- Performance: no effect.
- Verify: `cargo build --features sim`. Sim: paginated mode, `keys` page turns
  forward and back across a chapter end, `shot` at each step, before and after.
  Search with F3 from the top of the page (uses `top_item`).
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P4: One function for the drag-scroll speed
- Files: `src/reader/selection.rs:248-290`
- Problem: `start_scroll` (`selection.rs:249-255`) and `scroll_tick`
  (`selection.rs:271-282`) both read `self.list.viewport_bounds()` and
  `self.paged()`, then call `edge_speed` on the selection pointer with the same
  "not paged" guard.
- Change: add `fn drag_speed(&self) -> f32`: 0 when there is no selection, when
  it is not dragging, or when paged; else `edge_speed(&self.list.viewport_bounds(), selection.pointer)`.
  `start_scroll`: `let speed = self.drag_speed();` then take `&mut self.selection`
  and return if `selection.scrolling || speed == 0.`. `scroll_tick`: compute
  `speed` first, then take `&mut self.selection`; keep `pointer = selection.pointer`
  and the rest as now.
- Parity: `start_scroll` is called only from `select_drag`, which runs only
  while `dragging()` is true (the listener exists only then,
  `selection.rs:152`) and returns before it when the left button is up. So the
  added `dragging` test in `start_scroll` is always true there. The tick
  interval, the executor and the order of `extend_to` and `scroll_by` do not
  change.
- Performance: no effect.
- Verify: `cargo build --features sim`. Sim: scrolled mode, `down` on text,
  `move` to the bottom edge of the list, `wait:500`, `shot`, `up`, `shot`.
  Compare with the build before. Also the same in paginated mode: no scroll.
- Size: S
- Depends on: none
- Value: low. Risk: low.

## Suspected bugs

- Stale current match can panic. `start_search` sets `current = None`
  (`search.rs:111`) but keeps the old `results` until the debounced task ends
  (`search.rs:121-133`). The task replaces `results` and does not reset
  `current`. If the user presses Enter or F3 in that window (150 ms plus the
  search time), `step_match` (`search.rs:151`) picks an index into the old
  results. When the new, shorter results arrive, `Search::marked`
  (`search.rs:52`) indexes out of range in the next render. If the index is in
  range, the highlight and the "NN/MM" count show a wrong match.
- A match can have an empty range. In `find` (`src/search.rs:20-23`), a
  lowercase expansion gives two chars with the same byte offset (for example
  `İ` gives `i` and U+0307). A query `i` on `İ` gives `0..0`. The highlight is
  then empty and the snippet bold mark is empty.
- Search on a book that is still loading uses only the loaded items
  (`search.rs:119-120`). Results do not update when more chapters load,
  until the query changes. Possibly deliberate.

## Out of scope notes

- `src/reader.rs:1229-1235` (`render_block`) also has `font_size * QUOTE_GAP`.
  The quote gap could be a field of `Spacing` (`reader.rs:1278`), which would
  remove the P2 helper. That is a scope 1 change.
- The single-column width `(width - 2. * MARGIN).clamp(100., COLUMN_WIDTH)`
  with `width = viewport.width - panel_width()` is in `pages.rs:174-186` and in
  `render_item` (`reader.rs:1116-1118`). A shared helper would need scope 1.
- `page_count` (`pages.rs:239-240`) and `scroll_to_position`
  (`reader.rs:683-688`) compute the part of an item in two ways. They differ
  when `item.len == 0` (`max(1)` against `0.`). Do not merge them without a
  check of that case.

## Kept as is

- `page_at` searches twice (`pages.rs:597-603`): image pages share a start
  offset, and the first of them must be found. The test records it.
- `Pages::page` next to `Pages::at`: the offset alone cannot tell which of two
  pages with the same start is shown (comment at `pages.rs:75-77`).
- `measure` derives the line height from the measured height
  (`pages.rs:507-511`): GPUI rounds the line height.
- `+ 0.5` tolerances in `paginate` and the heading group: they absorb float
  error in measured heights. Changing them changes page breaks.
- `update_pages` spawns a timer only for a resize; other geometry changes apply
  at once (`Geometry::resized`, commit d992172).
- `caret_at` clamps to `bottom_right - 1px`: on the bottom edge the index is 0
  (comment at `selection.rs:237`).
- `start_search` returns early when the query is equal: the input sends a
  change on Enter too (comment at `search.rs:106`).
- `find` allocates the lowercase needle and haystack per item. It runs on the
  background executor; a reusable buffer would need a new API in the lib with
  one caller, for a small gain.
- `search()` clones each `SharedString` into an `Arc<Vec<_>>` per query: the
  clones are reference counts, and the snapshot must move to the background.
- `snippet` uses `format!` and then `replace`: `'\n'` and `' '` have the same
  byte length, so the mark range stays valid. The cost is small (visible rows only).

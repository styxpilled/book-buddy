# Scope 3: book and images

Files: `src/book/mod.rs`, `src/book/xhtml.rs`, `src/dither.rs`, `src/image_cache.rs`,
`src/cover_adjust.rs`, `src/backdrop.rs`.

Summary: the code in scope is in good condition. The parser, the dither module and
the backdrop are coherent and need no splits. The main finding is one helper that
has four copies (RGBA to BGRA `RenderImage`). The other proposals are small.

### P1: One helper to make a `RenderImage` from RGBA pixels
- Files: `src/image_cache.rs:113-126`, `src/cover_adjust.rs:205-211`, `src/backdrop.rs:135-154`
- Problem: the same 5 lines (swap R and B, wrap in `Frame`, `RenderImage`, `Arc`) occur
  in `image_cache.rs:121-125`, `cover_adjust.rs:205-211` (`render_image`),
  `backdrop.rs:149-153`, and also `reader.rs:1346-1350` (out of scope).
- Change: move `render_image` from `cover_adjust.rs:205-211` to `image_cache.rs` as
  `pub(crate) fn render_image(mut pixels: RgbaImage) -> Arc<RenderImage>`, with its
  `// RenderImage is BGRA.` comment. Then:
  - `load_filtered`: replace lines 120-125 with
    `Ok(render_image(filter(path, image.into_rgba8())))`.
  - `cover_adjust.rs`: delete the local fn, `use crate::image_cache::render_image;`,
    remove the now unused `Frame` import.
  - `backdrop.rs::render`: return `render_image(dither_small(...))`, remove the
    `Frame` import and the `mut` binding.
  - Scope 1 can then use it in `reader.rs::decode_image` (see out of scope notes).
  If scope 6 wants a different home (for example a `ui/image.rs`), the helper can move
  there in a pure move later. `image_cache.rs` is the module that already owns the
  `RenderImage` creation for files.
- Parity: the byte swap and the construction are the same code. Same thread in each
  caller (background for `load_filtered` and `backdrop::render`, main for the
  cover preview, as now). No pixel change.
- Performance: no effect. The function takes the buffer by value, as the copies do.
- Verify: `cargo test`, `cargo build --features sim`. Sim: `shot:library`, open the cover
  adjust screen of a book with a cover and `shot:adjust`; equal before and after.
- Size: S
- Depends on: none
- Value: medium. Risk: low.

### P2: Characterization test for the dither output
- Files: `src/dither.rs` tests (after line 413)
- Problem: the tests check shares of dark dots and square cells, not exact pixels. P3
  changes the inner loop of `dither_small` (a hot path). It must give the same bytes.
- Change: add a test that dithers a fixed image and compares a hash of the output with
  values recorded before P3. Image: `RgbaImage::from_fn(37, 23, |x, y| Rgba([(x * 7) as u8,
  (y * 11) as u8, ((x + y) * 5) as u8, if (x + y) % 9 == 0 { 0 } else { 255 }]))`.
  For each `Method` and for `cell` in `[1, 2, 3, 4]`, call `dither_small` with
  `Adjust::NONE` and with an `Adjust` that has non-zero `black`, `white`, `midtones`,
  `contrast` and one hue. Use dark `[10, 20, 30]` and light `[200, 210, 220]` so that the
  channel order shows. Hash `out.as_raw()` with FNV-1a 64 (inline, as in
  `book::content_hash`) and also check `out.dimensions()`. Record the hashes with the
  current code, then put them in the test as constants.
- Parity: test only.
- Performance: no effect.
- Verify: `cargo test dither`.
- Size: S
- Depends on: none
- Value: medium (it guards P3 and any later change). Risk: low.

### P3: Fill the dither output by rows, not by `put_pixel`
- Files: `src/dither.rs:216-226`
- Problem: `dither_small` writes each output pixel with `put_pixel` (`dither.rs:220-224`):
  `cell * cell` calls per dot, each with index math and a bounds check. The output is
  large: the backdrop is the size of the window in device pixels
  (`backdrop.rs:142`, about 8 M pixels at 4K), and the cover preview runs on the main
  thread at each slider change (`cover_adjust.rs:186`).
- Change: write the first pixel row of each band of `cell` rows, then copy it to the
  other `cell - 1` rows:
  ```rust
  let mut out = RgbaImage::new(small.width() * cell, small.height() * cell);
  if width == 0 {
      return out;
  }
  let cell = cell as usize;
  let row = width * cell * 4;
  for (band, dots) in out.chunks_exact_mut(row * cell).zip(light_dots.chunks_exact(width)) {
      let (first, rest) = band.split_at_mut(row);
      for (block, &is_light) in first.chunks_exact_mut(cell * 4).zip(dots) {
          let [r, g, b] = if is_light { light } else { dark };
          for pixel in block.chunks_exact_mut(4) {
              pixel.copy_from_slice(&[r, g, b, 255]);
          }
      }
      for line in rest.chunks_exact_mut(row) {
          line.copy_from_slice(first);
      }
  }
  out
  ```
  The `width == 0` guard keeps the current behavior for an empty input
  (`chunks_exact(0)` panics). Keep the name `cell` for the `u32` above it or use a new
  name, as the implementer prefers.
- Parity: same bytes. Dot `ix` maps to `x = ix % width`, `y = ix / width`, the same as
  the band and block order here. The tone math, the order of the float operations and
  the dither methods do not change. Checked: the output size, the 255 alpha, the dark
  and light order, an empty image.
- Performance: better. Per dot, `cell` small `copy_from_slice` calls on one row, then one
  `memcpy` per extra row, in place of `cell * cell` bounds-checked `put_pixel` calls.
  Paths: `backdrop::render` (background), the library cover filter (background),
  `reader::decode_image` (background), the cover preview (main thread).
- Verify: P2 test passes before and after. `cargo test`. Sim: `shot:library` (backdrop and
  covers), cover adjust screen, a book with images with `dither_images` on; equal.
- Size: S
- Depends on: P2
- Value: medium. Risk: low.

### P4: The cover cache filter is not optional
- Files: `src/image_cache.rs:21,25,50,80-86`, `src/library_view.rs:133,315` (call sites)
- Problem: `BoundedImageCache::filter` is `Option<Filter>`, but both callers pass
  `Some(...)` (`library_view.rs:133`, `library_view.rs:315`) and `set_filter` always sets
  `Some`. The `None` case dates from before the dithered covers (`git show ec16070`).
- Change: make the field `filter: Filter` and the `new` parameter `filter: Filter`. In
  `set_filter`, assign `self.filter = filter`. In `load`, match on `resource` only:
  `Resource::Path(path) => { let (filter, path) = (self.filter.clone(), path.clone()); ... }`
  and `_ => ImageAssetLoader` as now. Callers drop the `Some(...)`. The `library_view.rs`
  edit is two tokens at the call sites; scope 4 owns that file, so coordinate.
- Parity: the same branch runs for each resource as now (all callers have a filter).
  Non-path resources still use `ImageAssetLoader`.
- Performance: no effect.
- Verify: `cargo build --features sim`, sim `shot:library`, equal.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P5: One function to drop a cache item's image
- Files: `src/image_cache.rs:33-37`, `53-58`, `103-107`
- Problem: three copies of "if the item is loaded, drop its image from the atlas":
  `observe_release` (window `None`), `set_filter` (window `None`) and the eviction in
  `load` (window `Some(window)`).
- Change: add
  `fn drop_item(mut item: ImageCacheItem, window: Option<&mut Window>, cx: &mut App)`
  that does `if let Some(Ok(image)) = item.get() { cx.drop_image(image, window); }`, and
  call it in the three places. In `set_filter`, keep `self.order.retain` inside the
  `if let Some(item) = self.items.remove(&key)`.
- Parity: same calls with the same window argument.
- Performance: no effect.
- Verify: `cargo build --features sim`. Sim: change a cover adjustment and save, then
  `shot:library`; equal.
- Size: S
- Depends on: none (can combine with P4 in one diff)
- Value: low. Risk: low.

### P6: Share the test helpers of the book tests
- Files: `src/book/mod.rs:147-156`, `src/book/xhtml.rs:625-674`
- Problem: `describe_all` and `runs` have two copies, in `book/mod.rs` tests and in
  `xhtml.rs` tests. The `mod.rs` copy of `runs` does not include `ListItem`.
- Change: in `xhtml.rs`, move `describe_all` and `runs` out of `mod tests` to beside
  `describe` as `#[cfg(test)] pub(crate) fn`. In `book/mod.rs` tests, delete both and
  import them with `describe` (`use super::xhtml::{describe, describe_all, runs};`).
- Parity: test only. No assertion gets weaker: in `ignores_book_styling`
  (`mod.rs:283-287`) the `matches!` check rejects a `ListItem` before `runs` is used, and
  in `basic_book` (`mod.rs:269`) the block is a paragraph.
- Performance: no effect.
- Verify: `cargo test book`.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P7: `layout` counts chars without a `String` per paragraph
- Files: `src/book/xhtml.rs:267-289`
- Problem: `layout` collects the paragraphs into a `Vec`, then calls `Block::text()`
  for each, which allocates a `String` only to count its chars (`xhtml.rs:281`). This runs
  for each chapter parse (book open, chapter load, search).
- Change: keep the `Title` check. Then one pass:
  ```rust
  let (mut paragraphs, mut short, mut spaced) = (0, 0, false);
  for block in blocks {
      if let BlockKind::Paragraph { runs } = &block.kind {
          paragraphs += 1;
          let chars: usize = runs.iter().map(|r| r.text.chars().count()).sum();
          if chars <= VERSE_LINE {
              short += 1;
          }
          spaced |= block.gap > 0;
      }
  }
  ```
  and the same condition as `xhtml.rs:284`.
- Parity: the char count of the concatenation equals the sum of the char counts of the
  runs. The integer condition is the same. Tests `verse`, `prose_ignores_gaps_and_insets`,
  `short_lines_without_div_gaps_are_prose`, `title_page` cover it.
- Performance: better. One `Vec` and one `String` per paragraph less in each chapter
  parse (background).
- Verify: `cargo test`.
- Size: S
- Depends on: none
- Value: low. Risk: low.

## Suspected bugs

1. Cover adjust preview keeps old tones. `CoverAdjust::update_preview`
   (`cover_adjust.rs:177-191`) caches the preview by `(cell, adjust)` only. The tones come
   from `style::tones(cx)` (`cover_adjust.rs:185`). The settings overlay can open over the
   cover adjust screen (`shell.rs:184,197`). After "re-dither" or a theme change that
   changes the tones, the preview keeps the old tones until a slider, the method or the
   scale changes. The backdrop includes the tones in its key (`backdrop.rs:24-30`), so it
   is correct there.
2. Trailing space before a line break across a style change. `Builder::line_break`
   (`xhtml.rs:513-526`) checks only the current run for a trailing space. With
   `<p><b>a </b><br/>b</p>`, `current_run()` makes a new non-bold run, so the bold run
   keeps `"a "` and the text becomes `"a \nb"` (one char longer) where
   `<p>a <br/>b</p>` gives `"a\nb"`.

## Out of scope notes

- `src/reader.rs:1346-1350` (`decode_image`) is the fourth copy of the BGRA helper of P1.
  After P1, it can be `Some(render_image(pixels))`. Scope 1.
- `src/library_view.rs:133,315`: the call sites of P4 (`Some(...)` removal). Scope 4.
- `src/reader.rs:194` (`Item::new`): `len = text.chars().count()` could be read from the
  block offsets, but that is scope 1 and the gain is small.

## Kept as is

- `dither::dots` uses `imageops::resize` (Triangle), not `fast_image_resize`: a different
  resizer changes the dither input pixels, so cover pixels would change.
- `Adjust::auto_levels` sorts all lumas: it runs once per scan or button press on a small
  dot image. `select_nth` would be faster but the gain is not visible.
- `content_hash` reads the file a second time after `Epub::open`: the comment explains the
  ID must come from the contents and stay stable across Rust versions.
- `Book::locate` percent-decodes the whole spine per call: it runs on a link click or a
  contents entry, not in a hot path.
- `normalize` keeps the first segment on `..` (`xhtml.rs:254`): this keeps the leading
  empty segment of an absolute href. All hrefs from rbook are absolute.
- `xhtml::resolve` is `pub` in a private module: it is not visible outside `book`, so the
  visibility is already narrow in effect.
- `value_mut` is also used for reads (`cover_adjust.rs:151,240`), with a copy of `adjust`
  in `render`: a second read-only `match` would duplicate the slider order.
- The cover preview dithers on the main thread (`cover_adjust.rs:186`): the dot image is
  small and the result must show in the same frame as the slider move.
- The LRU in `BoundedImageCache::load` uses `VecDeque::position`: the capacity is two
  screens of covers, so the linear scan is short.
- `Backdrop` drops replaced images only in the window that renders next: the app has one
  window (`main.rs:94`).
- `hue_and_chroma` can return 360 through `rem_euclid` rounding: `hue_value` then gives the
  red value, which is correct.

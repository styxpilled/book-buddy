# Scope 1: `src/reader.rs`

`src/reader.rs` is mostly reasonable. It has 1776 lines and four concerns:
open and load, navigation, the image decode pipeline, and rendering (blocks,
bars, TOC). The proposals below remove dead code and small duplicates, and
split the two concerns that are clearly separate (images, bars and TOC).

Line numbers are for commit 2eded73.

### P1: Remove the dead "no book" path and the unreachable "Loading..." text
- Files: `src/reader.rs:159-164`, `:351`, `:368`, `:402-404`, `:1669-1671`. Call site `src/shell.rs:64` (one line).
- Problem: `Reader::new(path: Option<PathBuf>, ..)` (`reader.rs:351`) has one caller, `shell.rs:64`, and it always passes `Some(path)`. This has been so since the initial commit (`aa3ca4c`). Thus `Status::NoBook` (`:160`, `:368`) is never shown and the text "Open a book: book-buddy <file.epub>" (`:1670`) is dead. Also, `Shell::render` (`shell.rs:169`) does not render the reader while `loading()` is true, and `Status::Loading` never comes back after `Ready`. So the `"Loading..."` arm (`:1671`) is never drawn. The project rule is "never show a loading screen", so this text is misleading.
- Change:
  - `pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self`. Set `status: Status::Loading` in the struct literal and call `reader.open(path, cx)` with no `if let`. `open` still sets `Status::Loading` at `:409`; keep it or remove it (it is the same value).
  - Remove `Status::NoBook`.
  - In `render`, replace the `NoBook` and `Loading` arms with one arm: `Status::Loading => div().into_any_element(),`.
  - `shell.rs:64`: `Reader::new(path, cx)`.
- Parity: `new` gives the same state as before (`open` sets `Loading` first). The two removed texts are never on screen: `Shell` draws the library while `loading()` is true, and the reader is created only with a path. `Failed` still shows its message. No persisted data.
- Performance: no effect.
- Verify: `cargo build`, `cargo build --features sim`, `cargo test`. Sim: open a book from the library, `shot` before and after the change; they must be equal.
- Size: S
- Depends on: none
- Value: medium. Risk: low.

### P2: One helper to release a decoded image, one to drop released images
- Files: `src/reader.rs:236-248`, `:473-483`, `:1066-1075`, `:1604-1617`
- Problem: the same pattern is in three places: `if let Decode::Done(image) = mem::replace(&mut picture.decode, Decode::No) { ... }` (`:476`, `:1071`, `:1610`). The loop `for image in self.dropped.drain(..) { window.drop_image(image).ok(); }` is in two places (`:480-482`, `:1615-1617`).
- Change:
  - Add to `impl Picture`:
    ```rust
    /// Sets the picture to not decoded. Returns the image if it was decoded.
    fn reset(&mut self) -> Option<Arc<RenderImage>> {
        match mem::replace(&mut self.decode, Decode::No) {
            Decode::Done(image) => Some(image),
            _ => None,
        }
    }
    ```
    It must replace the state with `Decode::No` in all cases, as the current code does (also `Running` and `Failed`). See suspected bug B2: do not "fix" this here.
  - Add `fn drop_images(&mut self, window: &mut Window)` with the `dropped.drain` loop.
  - `close`: `for image in self.pictures.values_mut().filter_map(Picture::reset) { window.drop_image(image).ok(); }` then `self.drop_images(window);`.
  - `decoded` eviction: `if let Some(image) = self.pictures.get_mut(&old).and_then(Picture::reset) { self.dropped.push(image); }`.
  - `render` tone change: `self.dropped.extend(self.pictures.values_mut().filter_map(Picture::reset));` then `self.drop_images(window);`.
- Parity: the same states are set in the same `HashMap` iteration order. The order of `drop_image` calls stays the same (pictures first in `close`, then `dropped`). No pixels change.
- Performance: no effect. `extend` with a `filter_map` makes no extra allocation compared to the `push` loop.
- Verify: `cargo build`, `cargo build --features sim`. Sim: open a book with images, toggle "dither images" in settings, `shot` before and after; equal.
- Size: S
- Depends on: none. If P4 is accepted, do P4 first and put `reset` in `images.rs`.
- Value: medium. Risk: low.

### P3: Move the image pipeline to `src/reader/images.rs` (pure move)
- Files: `src/reader.rs:60-69` (image constants), `:236-248` (`Picture`, `Decode`), `:565-582` (`wait_for_images`), `:653-672` (`decode_near`, `images_near`), `:1020-1077` (`decode`, `decoded`), `:1319-1365` (`Tones`, `image_tones`, `decode_image`, `shrink`). `docs/notes.md` "Book images" section.
- Problem: about 200 lines of `reader.rs` are one separate concern: probe sizes, decode and dither in the background, keep at most `DECODED_IMAGES` decoded, release them to the window, wait for them at open. It does not depend on the text layout. The other reader concerns (pages, search, selection) already have their own files with `impl Reader` blocks.
- Change: create `src/reader/images.rs` and add `mod images;` next to `mod pages;`. Move the items above with no edits to their bodies. Keep `load_chapter` and `Loaded` in `reader.rs`. Visibility: `Picture`, its fields, `Decode`, `Tones`, `image_tones`, and the moved methods and constants that `reader.rs` or `pages.rs` uses get `pub(super)`. This is the same reach as now (the `reader` module). Child modules can read the private fields of `Reader`, so no field changes. Update `docs/notes.md`: "`decode_image` in `src/reader/images.rs`".
- Parity: pure move. No logic, order or executor changes.
- Performance: no effect.
- Verify: `cargo build`, `cargo build --features sim`, `cargo test`, `cargo clippy`.
- Size: M (move)
- Depends on: none. Scope 6 owns the crate module layout. This is a child of `reader`, in the existing `reader/` directory, so it does not cross module boundaries. Coordinate if scope 6 proposes a different layout.
- Value: medium. Risk: low.

### P4: Move the TOC and the bars to `src/reader/toc.rs` and `src/reader/bars.rs` (pure move)
- Files: `src/reader.rs:736-790` (`toc_target`, `current_toc`, `toc_at`, `open_toc_entry`, `toggle_toc`), `:1516-1599` (`render_toc`), `:78`, `:81` (`TOC_WIDTH`, `TOC_ROW_HEIGHT`) to `toc.rs`. `:1368-1514` (`render_top_bar`, `render_bottom_bar`, `render_seek`), `:80` (`SEEK_STEP`) to `bars.rs`.
- Problem: the second `impl Reader` block (`:1367-1600`) renders the window chrome. It does not share code with the text rendering. The TOC navigation helpers belong with `render_toc`.
- Change: create the two files, add `mod bars;` and `mod toc;`, move the items with no edits. Make the methods that `reader.rs` calls `pub(super)` (`render_top_bar`, `render_bottom_bar`, `render_toc`, `toggle_toc`, `current_toc`). Keep `TOP_BAR_HEIGHT`, `BOTTOM_BAR_HEIGHT`, `BAR_TRIGGER`, `bars_height` and `mouse_moved` in `reader.rs`, because `pages.rs` and `render` use them. Keep `seek_to` and `percent` in `reader.rs` (save uses `percent`).
- Parity: pure move. Element order and style calls stay the same.
- Performance: no effect.
- Verify: `cargo build`, `cargo build --features sim`, `cargo clippy`.
- Size: M (move)
- Depends on: none. Same note about scope 6 as P3.
- Value: medium. Risk: low.

### P5: One helper for the book length
- Files: `src/reader.rs:906-911`, `:931-937`
- Problem: `seek_to` and `percent` both do `if self.chapters.len() < self.spine_len { return .. }` and `let total: usize = self.chapters.iter().map(|c| c.len).sum();`.
- Change: add
  ```rust
  /// Characters in the book. Known when all chapters are loaded.
  fn book_len(&self) -> Option<usize> {
      (self.chapters.len() >= self.spine_len).then(|| self.chapters.iter().map(|c| c.len).sum())
  }
  ```
  `seek_to`: `let Some(total) = self.book_len() else { return; };`. `percent`: `let total = self.book_len()?;`.
- Parity: same check, same sum, same `as f32` casts in the same order.
- Performance: no effect.
- Verify: `cargo build`. Sim: drag the seek bar, `shot` before and after; equal.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P6: Remove `cx.notify()` calls that follow `scrolled`
- Files: `src/reader.rs:620-621`, `:806-807`, `:855-856`, `:875-876`, `:885-886`, `:902-903`
- Problem: `scrolled` (`:941-946`) calls `cx.notify()`. Six callers call `cx.notify()` again right after it. In gpui-pre 0.3.1 the second call does nothing more: `push_effect` drops a second `Effect::Notify` for the same entity (`app.rs:1655-1661`), and `dirty_views` is a set.
- Change: remove the `cx.notify();` line that follows `self.scrolled(cx);` in `go_to`, `scroll_by`, `chapter_start`, `chapter_end_key`, `book_start`, `show_book_end`.
- Parity: the view and the `Shell` observer get one notification per update, as now.
- Performance: no measurable effect.
- Verify: `cargo build`. Sim: Home, End, PageDown, Ctrl+End in both modes, `shot` before and after; equal.
- Size: S
- Depends on: none
- Value: low. Risk: low.

## Suspected bugs

- B1: Panic at open when an image near the saved position has no `Picture`. `wait_for_images` (`reader.rs:569`) indexes `self.pictures[href]`. `images_near` returns the href of each `Image` block, but `load_chapter` (`:1301-1314`) puts a `Picture` only when the resource reads and its size is known. The parser makes an `Image` block for any `img`/`image` with a `src` or `href` (`book/xhtml.rs:409-413`). A missing resource, or a format that `image` cannot probe (for example SVG), in the first `DECODE_AHEAD` blocks at the saved position, makes the `HashMap` index panic while the book loads. `decode` and `render_block` already handle a missing picture.
- B2: A tone change during a decode can start extra decodes and leak an image. At a tone change, `render` sets each picture to `Decode::No`, also a `Running` one (`:1610`). The render then starts a second decode. When the first (stale) result comes back, `decoded` sets the picture to `No` (`:1055`) while the second decode runs, so a third decode starts. The second result sets `Done(img2)`; the third result overwrites it with `Done(img3)` (`:1064`) without a push to `dropped`, so `img2` is never given to `window.drop_image`. The href is also in `decoded` two times. Triggers: theme change, dither toggle, or the re-dither button while an image decodes.

## Out of scope notes

- `src/reader/pages.rs:258` copies `HEADING_SCALE[(*level as usize).clamp(1, 6) - 1]` from `reader.rs:1161`. A `fn heading_scale(level) -> f32` next to the constant would serve both. For scope 2.
- `src/reader/pages.rs:279-283` and `:521-525` compute the quote gap (`if quote > 0 { size * QUOTE_GAP } else { 0. }`) two times, and `reader.rs:1233` uses the same value. A `quote` field in `Spacing` (`reader.rs:1278`) would remove the copies. For scope 2.
- `src/reader/pages.rs:166-176` and `reader.rs:1117-1120` both compute the one-column width `(viewport.width - panel - 2 * MARGIN).clamp(100., COLUMN_WIDTH)`. For scope 2.
- `src/reader/pages.rs:192` has the same "all chapters loaded" check as P5. If P5 is accepted, `estimate_pages` can use `book_len().is_none()`.
- `src/shell.rs:64` needs a one-line change for P1.

## Kept as is

- `on_app_quit` closure returns `async {}`: the gpui signature needs a future.
- `redraw` and the extra `request_animation_frame`: the comment at `:325-327` explains it.
- `Adjust::Above { laid_out }` two-frame scroll: the list must measure the items above first (comment at `:289`).
- `scroll_to_item` clears `adjust` and the caller sets it after: the order is deliberate.
- `loaded()` clears `pending`: the comment says the target chapter is not in the book.
- `image_tones` is read in every `render`, not in `apply_settings`: tones also depend on the theme and the `DitherTones` global, not only on `Store`.
- Images are released only in `render` and `close`: `drop_image` needs a `Window`.
- `shrink` falls back to `thumbnail_exact`: comment; `fast_image_resize` is required (`docs/notes.md`).
- BGRA swap loop in `decode_image`: `RenderImage` is BGRA (comment).
- `store.library.add` in `opened` and `mark_opened` in `show_when_ready`: comments and `docs/notes.md`.
- `render_block` with `view: None`: `pages.rs` uses it to measure without mouse handlers.
- `book_end` and `show_book_end`: `seek_to` needs the version that is not an action handler.
- `chapter_end_key`: the name avoids a clash with the `chapter_end` helper.
- `spine_len` next to `book`: a cached count, read in many places; not worth a change.
- `MONO` behind `cfg(target_os = "windows")`: already gated as required.
- `Picture::bytes: Arc<Vec<u8>>`: shares the bytes with the background task without a copy.
- The `wait_for_images` timer is not cancelled: when it fires late, `show_when_ready` does nothing because `loading()` is false.

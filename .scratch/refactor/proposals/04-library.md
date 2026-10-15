# Scope 4: library

Files: `src/library.rs`, `src/library_view.rs`, `src/scan.rs`, `src/details.rs`, `src/store.rs`.

The scope is in good condition. `scan.rs`, `store.rs` and `details.rs` need almost no change. The proposals are small. Most of them are about repeated work in the library grid render and about dead code.

### P1: Sort keys are calculated once for each book, not once for each comparison
- Files: `src/library_view.rs:55-62`, `src/library_view.rs:76-85`
- Problem: `Sort::Title` and `Sort::Author` use `sort_by_key` (`library_view.rs:79`, `:81`). `sort_by_key` calls the key function for each comparison, which is about 2·n·log n calls. Each call allocates one or two lower-case `String`s (`title_key`, `to_lowercase`). Also, `title_key` uses `map_or(title.clone(), ...)` (`:61`). `map_or` evaluates its default value before the call, so `title` is cloned when an article is found too, and the clone is then dropped. This runs on each render of the library: each filter key press and each `Store` change.
- Change:
  1. First add a characterization test in the `tests` module of `library_view.rs`. Build a `Vec<BookEntry>` with equal keys (two books with the same title, two with the same author and different titles, one with a `custom_title`, titles with "The "/"A "/"An "). Apply `Sort::Title` and `Sort::Author` and assert the full ID order. Equal keys must keep the input order (stable sort).
  2. In `Sort::apply`, change the `Title` and `Author` arms to `sort_by_cached_key` with the same key closures. Keep `sort_by_key` for `LastOpened` and `Added` (their keys are `Copy`).
  3. In `title_key`, remove the eager clone:
     ```rust
     if let Some(rest) = ["the ", "an ", "a "].iter().find_map(|a| title.strip_prefix(a)) {
         return rest.trim_start().to_string();
     }
     title
     ```
- Parity: `sort_by_cached_key` is a stable sort, the same as `sort_by_key` (the std docs say "preserving initial order of equal elements"). The keys are the same values, so the order is the same. `title_key` returns the same strings; the existing `title_key_skips_the` test covers it. No pixel or persisted data change.
- Performance: better. Key allocations go from O(n log n) to O(n) for each render of the library grid with a title or author sort. One `String` allocation less for each `title_key` call on a title with an article. `sort_by_cached_key` adds one `Vec<(K, usize)>` allocation for each sort.
- Verify: `cargo test` (new test passes before and after step 2 and 3). `cargo build --features sim`.
- Size: S
- Depends on: none
- Value: medium. Risk: low.

### P2: Remove `Library::open`, which only the tests use
- Files: `src/library.rs:286-306`, `src/library.rs:311`, tests at `src/library.rs:469`, `:525-554`, `:566`
- Problem: `Library::open` (`library.rs:287`) has no caller outside the tests. Commit b14b8a4 changed the reader to call `add` and then `mark_opened` when the reader shows (see `docs/notes.md`, "Opening a book"; `reader.rs:465`, `reader.rs:595`). After this, `add` is only a wrapper around `upsert` (`library.rs:303-306`).
- Change:
  1. Delete `Library::open`.
  2. Merge `upsert` into `add`: `add` gets the body of `upsert` and returns `&mut self.books[index]`. Keep the doc comments of both (the `upsert` comment explains the ID and path rules).
  3. In `mod tests`, add a helper that does what `open` did:
     ```rust
     fn open(library: &mut Library, info: FileInfo, now: DateTime<Utc>) -> &mut BookEntry {
         let id = info.id.clone();
         library.add(info, now);
         library.mark_opened(&id, now);
         library.book_mut(&id).unwrap()
     }
     ```
     Replace `library.open(x, t)` with `open(&mut library, x, t)` in `save_and_load`, `open_matches_by_path_then_id` and `a_copy_of_a_book_is_the_same_entry`. All assertions stay as they are.
  Note: in `a_copy_of_a_book_is_the_same_entry` the copy keeps ID "a", so `book_mut("a")` finds the same entry that `open` returned. In `open_matches_by_path_then_id` the ID after `add` is `info.id`, so the lookup is correct too.
- Parity: `Library` is in the lib, but the only consumer of the lib is the binary. No production code path changes. Serialized data does not change.
- Performance: no effect.
- Verify: `cargo test`, `cargo build --features sim`.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P3: One function makes the cover cache
- Files: `src/library_view.rs:126-135`, `src/library_view.rs:311-315`
- Problem: `LibraryView::new` and the tone change branch of `render` both do `BoundedImageCache::new(decoded_covers(window), Some(cover_filter(tones, adjusts.clone())), cx)`. Two copies of how the cover cache is set up.
- Change: add
  ```rust
  fn cover_cache(tones: ([u8; 3], [u8; 3]), adjusts: Arc<HashMap<PathBuf, Adjust>>, window: &Window, cx: &mut App) -> Entity<BoundedImageCache> {
      BoundedImageCache::new(decoded_covers(window), Some(cover_filter(tones, adjusts)), cx)
  }
  ```
  and call it at both places. In `render`, `cx` is `&mut Context<Self>`, which derefs to `&mut App`. Keep the order in `render`: assign `self.adjusts` first, then make the cache with `self.adjusts.clone()`.
- Parity: the same calls with the same values in the same order. No pixel change.
- Performance: no effect.
- Verify: `cargo build --features sim`. Sim: change the theme in settings and look at the library grid (covers dither to the new tones); screenshots equal before and after.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P4: `Store` removes a cover file in one place
- Files: `src/store.rs:56-67`, `src/library_view.rs:572-579`
- Problem: two copies of "remove the cover thumbnail of a book":
  - `store.rs:58-64` logs errors other than `NotFound`.
  - `library_view.rs:575` does `std::fs::remove_file(cover_path(store.covers(), &removed.id)).ok()` and drops all errors.
  The cover files belong to `Store` (it owns the `covers` directory and `cover_path`), but `library_view.rs` deletes them itself.
- Change: in `store.rs`, add
  ```rust
  fn remove_cover(&self, id: &str) { /* body of the loop in clear_library */ }
  /// Removes a book and its cover thumbnail, or hides it (see `Library::remove`).
  pub fn remove_book(&mut self, id: &str) {
      if let Some(removed) = self.library.remove(id) {
          self.remove_cover(&removed.id);
      }
      self.save();
  }
  ```
  `clear_library` calls `self.remove_cover(&book.id)` in its loop. The "REMOVE FROM LIBRARY" item calls `cx.update_global(|store: &mut Store, _| store.remove_book(&remove_id))`. `library_view.rs` then does not need `cover_path` for removal (it still uses it at `:293` and `:406`).
- Parity: the same file is removed, then the same save. The only difference is that a failed remove other than `NotFound` is now logged. Log text can change (rules.md). No user-visible error behavior changes.
- Performance: no effect.
- Verify: `cargo build --features sim`. Manual or sim: remove a book outside a watched folder, then make sure that its `covers/<id>.png` is gone.
- Size: S
- Depends on: none
- Value: low. Risk: low.

### P5: The library grid keeps indices, not clones of the visible books
- Files: `src/library_view.rs:191-208`, `src/library_view.rs:328-382`
- Problem: `visible_books` (`:204`) clones each visible `BookEntry` on each render. One clone is 5 `String`/`PathBuf` allocations plus the `Option<String>` overrides. The clones only go into the `uniform_list` closure, which must be `'static`. That closure already gets `cx: &mut App`, so it can read the entries from the `Store` global.
- Change:
  1. `visible_books` returns `Vec<usize>`: indices into `library.books`, filtered as now. Sort the indices with the same key functions on `&library.books[i]` (P1 keys if P1 is done). Keep a stable sort, so equal keys keep the library order.
  2. In `render`, `let books = Rc::new(self.visible_books(cx));`. In the row closure:
     ```rust
     let library = &cx.global::<Store>().library;
     books[start..end].iter().map(|&i| card(&library.books[i], &covers, view.clone(), cx))
     ```
     `card` takes `cx: &App`, so both borrows are shared. Reborrow `cx` as `&App` first.
  3. `Sort::apply` changes to take `&[BookEntry]` and `&mut [usize]`. Move the P1 test to the new signature.
- Parity: the index list is computed in `render` and read in prepaint of the same frame. gpui does not run app updates or global changes during a draw, so `library.books` cannot change between the two. Any change to `Store` notifies the view (`observe_global`, `:124`), so the next render makes new indices. Same sort keys, same stable order, same filter. No pixel change.
- Risk checked: if a later change caches the view (`.cached()`) or calls the list closure outside the frame of the render, the indices could go stale. Today neither happens. An out-of-range index would panic, so this risk must be accepted or each lookup must use `get` and skip (which would hide the problem).
- Performance: better. Removes about 6 allocations for each visible book on each render of the library. The sort moves 8-byte indices, not 200-byte entries.
- Verify: `cargo test`, `cargo build --features sim`. Sim: library with many books; type in the filter, change each sort, scroll to the end; screenshots equal before and after.
- Size: M
- Depends on: P1 (so that the sort test exists first)
- Value: medium. Risk: medium.

### P6: One helper emits a `LibraryEvent` from a weak view
- Files: `src/library_view.rs:512-521`, `:558-570`
- Problem: three copies of `view.update(cx, |_, cx| cx.emit(LibraryEvent::X(...))).ok();` (`:517`, `:559-563`, `:566-569`). `book_items` also clones the view a second time (`details_view`, `:548`) only for this.
- Change: add
  ```rust
  fn emit(view: &WeakEntity<LibraryView>, event: LibraryEvent, cx: &mut App) {
      view.update(cx, |_, cx| cx.emit(event)).ok();
  }
  ```
  and use it at the three places. Each closure still clones its `String`/`PathBuf` into the event, because the closures are `Fn`.
- Parity: same calls, same events.
- Performance: no effect.
- Verify: `cargo build --features sim`. Sim: open a book from the grid, open "EDIT DETAILS" and "ADJUST COVER" from the context menu.
- Size: S
- Depends on: none
- Value: low. Risk: low.

## Suspected bugs

- `library.rs:311-334` (`upsert`): when an edited file at a known path gets a new ID, `entry.id` changes but the cover thumbnail named by the old ID stays in `covers/` and is never removed. Also `info.cover_adjust` (auto levels of the new cover) is ignored, and the old `cover_adjust` is applied to the new cover. Keeping a user adjustment can be deliberate; the orphan file is probably not.
- `library.rs:378-386` (`in_folder`): it compares paths in lower case on all platforms, but `same_path` (`:403-412`) is case sensitive outside Windows. On Linux or macOS, a book in `/books` would count as in the watched folder `/Books`. Only matters for non-Windows builds.
- `library_view.rs:407`: `card` calls `cover.exists()` (a file system stat) for each visible card on each render. This is not a bug, but if a cover file is deleted outside the app, the card keeps the cached image until the cache drops it. Low impact.

## Out of scope notes

- The "underlined input" element is the same in three places: `library_view.rs:231-236`, `details.rs:90-96` and `reader/search.rs:314-318` (`div().flex_1().border_b_1().border_color(theme.border).child(Input::new(x).appearance(false).cleanable(true))`). A helper in `style.rs` (scope 5) would remove the three copies.
- `book_mut(id)` then a change then `store.save()` repeats in `details.rs:214-219`, `library_view.rs:551-556`, `cover_adjust.rs:165-170` and (with a conditional save) `reader.rs:1005-1016`. A `Store::update_book(id, f)` helper could serve them, but it crosses scopes 1, 3 and 4. Not proposed here.
- `scan::book_file` is used by `reader.rs:1285` too. It opens the book and writes the cover; the name "scan" does not fit the reader use. A move is for scope 6 to decide.

## Kept as is

- `render` calls `cover_adjusts(cx)` on each render (`library_view.rs:310`): it allocates one path per book with an adjustment, but this is how a cover adjust change is found. The filter gets a path, so the map must be keyed by path. The cost is small next to the grid.
- `decoded_covers` and `set_capacity` on each render (`:330-332`): the capacity follows the window size; commit 314712f (cover memory leak) added it.
- `uniform_list` with an `image_cache` per `img` (`:358-360`, `:434`): the comment explains it; without it the app cache keeps all images.
- `#[cfg(not(target_os = "windows"))] let _ = (path, missing);` (`:583-584`): stops unused-variable warnings outside Windows.
- `Store::path: Option<PathBuf>` (`store.rs:12-14`): `None` stops saves after a read error, so a bad file is not overwritten.
- `Library::save` writes a temp file, then renames (`library.rs:233-244`): atomic write.
- `merge_copies` (`library.rs:249-269`): migration for libraries of older versions; it has a test.
- `Font::Serif` named "serif" but is Literata (`library.rs:150`): the serde name is persisted.
- `#[serde(default)]` on each field and `skip_serializing_if` on `window`, `cover_adjust`, `custom_*`: persisted format; `save_and_load` asserts `cover_adjust` is not written for `Adjust::NONE`.
- `scan::start` calls `update_missing` before and after the scan (`scan.rs:63`, `:74`): `unchanged` skips missing entries, so the first call is needed; the second marks files found again.
- `Sort::Author` key includes `title_key` as a second key: books by one author sort by title.
- The local closure `field` in `DetailsDialog::open` has the same name as the free function `field`: a rename is taste only.

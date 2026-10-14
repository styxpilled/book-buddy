# 08: Memory leak

Status: done
Depends on: none

## Scope

The app leaks a little bit of memory every time a book gets opened. Measure the leak and fix it. You can use `./scripts/memory.ps1` to help you measure, but you need to update it to refer to the app.

## Acceptance criteria

- The leak is measured, found, and fixed. If other leaks are found, they must also be fixed.
- Opening and closing books does not cause memory usage to go up over time.

## Comments

### 2026-10-14: measurements and fix

- `scripts/memory.ps1` now samples `book-buddy.exe`.
- Open and close of the same books: no growth. Open of books not opened before in the session: about 1.1 MB of heap per book, with no limit. Copies of one book with different IDs gave the same result, so the growth came from each new book, not from new glyphs or images. Measured with a temporary counting allocator and the sim, then removed.
- Cause 1: a GPUI bug. `image_cache(...)` puts its cache in scope for layout and paint, but not for prepaint. `uniform_list` lays out its rows in prepaint, so each cover `img` in the grid also loaded through the app asset cache, which never releases an image. Each cover shown was kept a second time, without the dither (0.55 MB each). Fix: each cover `img` gets the cover cache with `.image_cache(covers)`.
- Cause 2: the cover cache kept 150 decoded covers, about 85 MB on the CPU and the same in the GPU atlas. Each open puts a new cover at the top of the grid. Fix: the cache keeps the covers of two screens (`decoded_covers`), 40 in a 1016x808 window. It changes with the window size.
- Result: 40 new books in a row. The heap starts at 19.3 MB (it was 26.8 MB, because the 15 visible covers were decoded twice). It grows 0.55 MB for each new cover until the cache is full, and then stays at 33 MB. GPU memory stays the same after the cache is full.
- Reader images, the reader entity, library entries (15 KB per book) and the backdrop do not leak.

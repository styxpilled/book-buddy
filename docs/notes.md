# Notes

## Popups

- Do not use the gpui-component menus or popovers. They draw a soft shadow and a ring that cannot be changed.
- Use `src/menu.rs`:
  - `Menu` and `popup` make a popup menu.
  - `shield` makes a layer that blocks the mouse input to the content under it. The settings overlay uses it too.
- The offset shadow box is `style::SHADOW`. For a panel that is as tall as its parent, use `style::side_shadow`.

## Book images

- `decode_image` in `src/reader.rs` resizes with `fast_image_resize`. The `image` crate's `resize` is about 10 times slower.
- When `dither_images` is on, the image is resized to one pixel per dot, then `dither_small` dithers it.
- If the tones or the setting change, the images decode again.
- In the dev profile, the image crates have `opt-level = 3`. Without it, a large JPEG takes seconds to decode.

## Opening a book

- `Shell` shows the library until `Reader::loading()` is false.
- The reader waits for the images at the saved position, for at most `IMAGE_WAIT`.
- The open is recorded (`Library::mark_opened`) only when the reader shows.

## Measuring

- The data directory is next to the executable.
- To measure without changing the user's library or release build, build with `CARGO_TARGET_DIR` set to a temporary directory. Put a test `library.json` in `<target>/release/book-buddy-data/`.
- A release build shows no console. To get the log, start it with `RUST_LOG=book_buddy=info` and redirect stderr.

# 04: Faster images on book open

Status: needs-triage
Spec: [../spec.md](../spec.md)
Depends on: none

## Scope

- Images in the book show too slowly after a book opens. Make them show sooner.
- First measure where the time goes. Current behavior (`src/reader.rs`):
  - Chapters load one after the other on a background task (`load_chapter`). It reads the bytes and the size of each image of the chapter.
  - An image decodes only when its block is rendered for the first time (`Decode::No`, then `decode`). The decode makes a full image of up to 2048 px (`IMAGE_MAX`), then swaps the channels.
  - Only 16 decoded images stay in memory (`DECODED_IMAGES`).
- Possible fixes, after measurement: start the decode of the images near the saved position before the first render, decode at the display size, decode more than one image at the same time.

## Acceptance criteria

- Measured times before and after are in the comments of this issue, for a release build and a book with large images.
- When a book opens at a page with an image, the image shows with the text, or with a short delay that is measured.

## Comments

### Measurements (2026-10-13)

Times are in ms from the start of `Reader::open`, from temporary log lines (now removed). Windows 10, scroll mode.

**Book with many small images:** `pg1184-images-3.epub` (450 JPEG files, max 255 KB). It opens at spine 77, a page with 3 images after 77 chapters.

| Step | Release | Debug |
| --- | --- | --- |
| Book parsed | 77-91 | 585 |
| Text ready (chapters 0-77 loaded) | 162-176 | 862 |
| Decode starts (first render) | 166-189 | 870 |
| One decode | 2.3-2.7 | 111-162 |
| All 3 images decoded | 171-194 | 1037 |

In release, the images show about 10 ms after the text. Paginated mode is correct at this position: the first page holds only the heading "VOLUME FOUR", because the image is too tall to fit under it.

**Book with one large image:** `pg174-images-3.epub`. It opens at the cover, 1736 x 2992 px. This is larger than `IMAGE_MAX` (2048), so the image is resized.

| Part of one decode | Release before | Release after | Debug before | Debug, `thumbnail` only | Debug after |
| --- | --- | --- | --- | --- | --- |
| JPEG decode | 10 | 10 | not split | 1122 | 12 |
| Resize | 195 | 57 | not split | 627 | 47 |
| To RGBA, BGRA swap | 4 | 4 | not split | 180 | 115 |
| Total decode | 208 | 72 | 3055 | 1936 | 175 |
| Image shows after text | 222 | 86 | 3080 | 1950 | 180 |

### Causes

- The `Triangle` resize of large images is slow: it takes 195 of the 208 ms.
- In the debug build, the image crates are not optimized. The decode is about 100 times slower.
- Chapter loading and the first render are not the problem. Decode starts within about 10 ms after the text is ready, and the decodes already run at the same time.

### Changes

- `decode_image` uses `DynamicImage::thumbnail` (area average) in place of `resize(.., FilterType::Triangle)`.
- `Cargo.toml`: the image decoder crates use `opt-level = 3` in the dev profile.

### Decode before the first render (2026-10-13)

When the chapter of the saved position loads, `decode_near` in `append` starts the decode of the images in the next 8 blocks (`DECODE_AHEAD`). This happens before the first render. Release, 3 runs each:

| Book | Decode starts | Text ready | First render | Images decoded |
| --- | --- | --- | --- | --- |
| `pg1184`, 3 small images | 146-154 | 148-155 | 148-167 | 152-159 |
| `pg174`, large cover | 8-10 | 9-10 | 13-17 | 82-83 |

- Small images now show with the text: the decode ends before the first render, or at most 4 ms after it.
- The large cover shows about 73 ms after the text. Before this change, it took 86 ms.

### Not done

- Decode at the display size. The column is 720 px and the height is 90% of the window, so at display scale 2 the size is almost the same as 2048. Also, the resize time depends mostly on the size of the source image.
- Debug only: the BGRA swap loop in our crate takes 111 ms for a 5 MP image. In release, it takes 0.8 ms.

### fast_image_resize (2026-10-13)

`decode_image` resizes with `fast_image_resize` 5.5.0 (SIMD, Lanczos3) in place of `thumbnail`. If the resizer does not know the pixel format, it falls back to `thumbnail`.

A benchmark outside the app (release, the `pg174` cover, mean of 10 runs, with decode and RGBA conversion):

| Option | ms |
| --- | --- |
| Decode only | 8.6 |
| `thumbnail` to 2048 px | 66 |
| `fast_image_resize` to 2048 px, Lanczos3 | 22.5 |
| No resize | 14 |

In the app (release), one decode of the cover is 28 ms. Before this change, it was 72 ms.

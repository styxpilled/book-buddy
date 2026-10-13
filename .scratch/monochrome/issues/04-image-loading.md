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

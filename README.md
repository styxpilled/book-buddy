# book-buddy

A lightweight epub reader made with GPUI.

## Install on macOS

1. Download `Book-Buddy.dmg` from [Releases](https://github.com/styxpilled/book-buddy/releases/latest).
2. Open it and drag Book Buddy to Applications.
3. The first time, macOS blocks the app. Go to System Settings > Privacy & Security and click Open Anyway.

## Build

Needs Rust. On macOS, also Xcode.

```sh
cargo run --release
```

To make the macOS app and the dmg:

```sh
scripts/bundle.sh --dmg
```

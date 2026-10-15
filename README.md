# book-buddy

A lightweight epub reader made with GPUI.

## Install on macOS

1. Download `Book-Buddy.dmg` from [Releases](https://github.com/styxpilled/book-buddy/releases/latest).
2. Open it and drag Book Buddy to Applications.
3. The first time, macOS blocks the app. Go to System Settings > Privacy & Security and click Open Anyway.

## Install on Windows

1. Download `Book-Buddy.exe` from [Releases](https://github.com/styxpilled/book-buddy/releases/latest).
2. Run it. The first time, Windows SmartScreen can block the app. Click More info > Run anyway.

## Build

Needs Rust. On macOS, also Xcode.

```sh
cargo run --release
```

To make the macOS app and the dmg:

```sh
scripts/bundle.sh --dmg
```

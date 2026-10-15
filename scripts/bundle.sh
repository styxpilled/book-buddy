#!/bin/sh
# Builds "target/release/Book Buddy.app". macOS only.
#   scripts/bundle.sh
# The app opens .epub files from Finder ("Open With"). It does not become the
# default app for them unless the user chooses it.
set -e
cd "$(dirname "$0")/.."

cargo build --release
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
app="target/release/Book Buddy.app"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/book-buddy "$app/Contents/MacOS/"
cp assets/icon.icns "$app/Contents/Resources/"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Book Buddy</string>
    <key>CFBundleDisplayName</key>
    <string>Book Buddy</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.styxpilled.book-buddy</string>
    <key>CFBundleExecutable</key>
    <string>book-buddy</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.books</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>EPUB Book</string>
            <key>CFBundleTypeRole</key>
            <string>Viewer</string>
            <key>LSHandlerRank</key>
            <string>Alternate</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>org.idpf.epub-container</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
EOF
# An ad-hoc signature. Apple Silicon does not run unsigned code.
codesign --force --sign - "$app"
echo "$app"

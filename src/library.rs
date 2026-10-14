use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::dither::Adjust;

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub version: u32,
    /// Watched folders, scanned for books on startup.
    #[serde(default)]
    pub folders: Vec<PathBuf>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub last_book: Option<String>,
    #[serde(default)]
    pub books: Vec<BookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowState>,
}

/// The window when the app closed. The bounds are the windowed bounds, in
/// logical pixels, also when the window is maximized or fullscreen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub maximized: bool,
    #[serde(default)]
    pub fullscreen: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookEntry {
    pub id: String,
    pub path: PathBuf,
    /// File size and modified time when `id` was calculated.
    pub size: u64,
    pub modified: DateTime<Utc>,
    pub title: String,
    pub author: String,
    pub added: DateTime<Utc>,
    #[serde(default)]
    pub last_opened: Option<DateTime<Utc>>,
    #[serde(default)]
    pub position: Position,
    #[serde(default)]
    pub percent: f32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub missing: bool,
    /// Applied to the cover before it is dithered.
    #[serde(default, skip_serializing_if = "Adjust::is_none")]
    pub cover_adjust: Adjust,
    /// Set by the user in place of `title` and `author`, which come from the
    /// file. A scan does not change them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_author: Option<String>,
}

impl BookEntry {
    pub fn shown_title(&self) -> &str {
        self.custom_title.as_deref().unwrap_or(&self.title)
    }

    pub fn shown_author(&self) -> &str {
        self.custom_author.as_deref().unwrap_or(&self.author)
    }

    /// An empty value, or the value from the file, removes the override.
    pub fn set_details(&mut self, title: &str, author: &str) {
        self.custom_title = custom(title, &self.title);
        self.custom_author = custom(author, &self.author);
    }
}

fn custom(value: &str, from_file: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != from_file).then(|| value.to_string())
}

/// A spine index, and a character offset into the text of that chapter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position {
    pub spine: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub font: Font,
    pub font_size: u32,
    pub theme: Theme,
    pub show_hidden: bool,
    /// Show the bars of the reader only when the pointer is near them.
    pub hide_bars: bool,
    /// Dither the images of books to the two tones of the theme, like the covers.
    pub dither_images: bool,
    /// The hue of the neutral theme colors in degrees. `None` keeps the theme
    /// as drawn.
    pub hue: Option<u16>,
    /// Percent of the saturation of the neutral theme colors as drawn.
    pub saturation: u16,
}

pub const FONT_SIZES: RangeInclusive<u32> = 12..=36;

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Scroll,
            font: Font::Serif,
            font_size: 18,
            theme: Theme::Light,
            show_hidden: false,
            hide_bars: false,
            dither_images: true,
            hue: None,
            saturation: 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Scroll,
    Paginated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Font {
    /// Literata. The name is from the time of two fonts.
    Serif,
    /// Inter.
    Sans,
    Newsreader,
    SourceSerif,
    Alegreya,
    Garamond,
    Mono,
}

impl Font {
    pub const ALL: [Font; 7] = [
        Font::Serif,
        Font::Newsreader,
        Font::SourceSerif,
        Font::Alegreya,
        Font::Garamond,
        Font::Sans,
        Font::Mono,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

/// What the library knows about a book file when it is opened.
pub struct FileInfo {
    pub id: String,
    pub path: PathBuf,
    pub size: u64,
    pub modified: DateTime<Utc>,
    pub title: String,
    pub author: String,
    /// For a new entry.
    pub cover_adjust: Adjust,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            version: VERSION,
            folders: Vec::new(),
            settings: Settings::default(),
            last_book: None,
            books: Vec::new(),
            window: None,
        }
    }
}

impl Library {
    /// A missing file gives an empty library. A file that is not valid JSON is
    /// renamed to `.bad`, so that a save does not overwrite it.
    pub fn load(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(mut library) => {
                library.merge_copies();
                Ok(library)
            }
            Err(e) => {
                let bad = path.with_extension("json.bad");
                log::error!(
                    "{} is not valid ({e}). It is moved to {}.",
                    path.display(),
                    bad.display()
                );
                fs::rename(path, &bad)?;
                Ok(Self::default())
            }
        }
    }

    /// Writes a temp file and renames it, so the file is never half written.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("json.tmp");
        let mut file = File::create(&temp)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path).with_context(|| format!("cannot write {}", path.display()))
    }

    /// Older versions kept a copy of a book as a second entry with the same
    /// ID. The entry opened last stays, with a file that exists, and the
    /// earliest added time.
    fn merge_copies(&mut self) {
        let mut merged: Vec<BookEntry> = Vec::with_capacity(self.books.len());
        for book in self.books.drain(..) {
            let Some(kept) = merged.iter_mut().find(|b| b.id == book.id) else {
                merged.push(book);
                continue;
            };
            let added = kept.added.min(book.added);
            let other = if book.last_opened > kept.last_opened {
                std::mem::replace(kept, book)
            } else {
                book
            };
            if !kept.path.exists() && other.path.exists() {
                (kept.path, kept.size, kept.modified) = (other.path, other.size, other.modified);
                kept.missing = false;
            }
            kept.added = added;
        }
        self.books = merged;
    }

    /// Removes all books and watched folders. The settings stay.
    pub fn clear(&mut self) {
        self.books.clear();
        self.folders.clear();
        self.last_book = None;
    }

    pub fn book(&self, id: &str) -> Option<&BookEntry> {
        self.books.iter().find(|b| b.id == id)
    }

    pub fn book_mut(&mut self, id: &str) -> Option<&mut BookEntry> {
        self.books.iter_mut().find(|b| b.id == id)
    }

    /// Finds the entry for a file, or adds one, and makes it the last opened book.
    pub fn open(&mut self, info: FileInfo, now: DateTime<Utc>) -> &mut BookEntry {
        let index = self.upsert(info, now);
        let id = self.books[index].id.clone();
        self.mark_opened(&id, now);
        &mut self.books[index]
    }

    /// Makes a book the last opened book.
    pub fn mark_opened(&mut self, id: &str, now: DateTime<Utc>) {
        if let Some(entry) = self.book_mut(id) {
            entry.last_opened = Some(now);
            self.last_book = Some(id.to_string());
        }
    }

    /// Finds the entry for a file, or adds one.
    pub fn add(&mut self, info: FileInfo, now: DateTime<Utc>) -> &mut BookEntry {
        let index = self.upsert(info, now);
        &mut self.books[index]
    }

    /// There is one entry for each book ID. A copy of a book at another path
    /// is the same entry, which keeps its own file while that file exists. A
    /// file with a new ID at a known path (an edited file) keeps its entry.
    fn upsert(&mut self, info: FileInfo, now: DateTime<Utc>) -> usize {
        let found = self.books.iter().position(|b| b.id == info.id).or_else(|| {
            self.books
                .iter()
                .position(|b| same_path(&b.path, &info.path))
        });
        match found {
            Some(index) => {
                let entry = &mut self.books[index];
                if entry.id == info.id && !same_path(&entry.path, &info.path) && entry.path.exists()
                {
                    return index;
                }
                if self.last_book.as_deref() == Some(entry.id.as_str()) {
                    self.last_book = Some(info.id.clone());
                }
                entry.id = info.id;
                entry.path = info.path;
                entry.size = info.size;
                entry.modified = info.modified;
                entry.title = info.title;
                entry.author = info.author;
                entry.missing = false;
                index
            }
            None => {
                self.books.push(BookEntry {
                    id: info.id,
                    path: info.path,
                    size: info.size,
                    modified: info.modified,
                    title: info.title,
                    author: info.author,
                    added: now,
                    last_opened: None,
                    position: Position::default(),
                    percent: 0.,
                    hidden: false,
                    missing: false,
                    cover_adjust: info.cover_adjust,
                    custom_title: None,
                    custom_author: None,
                });
                self.books.len() - 1
            }
        }
    }

    /// True if an entry has this path, size and modified time, so its ID is
    /// still correct and the file need not be hashed again.
    pub fn unchanged(&self, path: &Path, size: u64, modified: DateTime<Utc>) -> bool {
        self.books.iter().any(|b| {
            !b.missing && b.size == size && b.modified == modified && same_path(&b.path, path)
        })
    }

    /// Sets `missing` from the file system. Returns true if an entry changed.
    pub fn update_missing(&mut self) -> bool {
        let mut changed = false;
        for book in &mut self.books {
            let missing = !book.path.exists();
            changed |= book.missing != missing;
            book.missing = missing;
        }
        changed
    }

    pub fn in_folder(&self, path: &Path) -> bool {
        let path = path.to_string_lossy().to_lowercase();
        self.folders.iter().any(|folder| {
            let folder = folder.to_string_lossy().to_lowercase();
            let folder = folder.trim_end_matches(['\\', '/']);
            path.strip_prefix(folder)
                .is_some_and(|rest| rest.starts_with(['\\', '/']))
        })
    }

    /// A book in a watched folder would come back on the next scan, so it is
    /// hidden. Other books are removed. Returns the removed entry.
    pub fn remove(&mut self, id: &str) -> Option<BookEntry> {
        let index = self.books.iter().position(|b| b.id == id)?;
        if self.in_folder(&self.books[index].path) {
            self.books[index].hidden = true;
            return None;
        }
        if self.last_book.as_deref() == Some(id) {
            self.last_book = None;
        }
        Some(self.books.remove(index))
    }
}

/// Windows paths are not case sensitive.
#[cfg(target_os = "windows")]
fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

#[cfg(not(target_os = "windows"))]
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("book-buddy-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn time(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    fn info(id: &str, path: &str) -> FileInfo {
        FileInfo {
            id: id.into(),
            path: path.into(),
            size: 10,
            modified: time(100),
            title: "Title".into(),
            author: "Author".into(),
            cover_adjust: Adjust::NONE,
        }
    }

    #[test]
    fn clear_keeps_the_settings() {
        let mut library = Library::default();
        library.settings.font_size = 30;
        library.folders.push("C:\\Books".into());
        library.add(info("a", "C:\\Books\\a.epub"), time(100));
        library.last_book = Some("a".into());
        library.clear();
        assert!(library.books.is_empty() && library.folders.is_empty());
        assert_eq!(library.last_book, None);
        assert_eq!(library.settings.font_size, 30);
    }

    #[test]
    fn missing_file_is_an_empty_library() {
        let dir = temp_dir("missing");
        assert_eq!(
            Library::load(&dir.join("library.json")).unwrap(),
            Library::default()
        );
    }

    #[test]
    fn save_and_load() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("data").join("library.json");
        let mut library = Library::default();
        let book = library.open(info("a", "C:\\Books\\a.epub"), time(200));
        book.position = Position {
            spine: 3,
            offset: 42,
        };
        book.cover_adjust.contrast = -20;
        library.add(info("b", "C:\\Books\\b.epub"), time(300));
        library.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("cover_adjust").count(), 1);
        assert_eq!(Library::load(&path).unwrap(), library);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn settings_round_trip_and_default() {
        let dir = temp_dir("settings");
        let path = dir.join("library.json");
        fs::write(
            &path,
            r#"{ "version": 1, "settings": { "theme": "dark" } }"#,
        )
        .unwrap();
        let mut library = Library::load(&path).unwrap();
        assert_eq!(library.settings.theme, Theme::Dark);
        assert_eq!(library.settings.font_size, 18);
        assert!(!library.settings.show_hidden);
        assert!(!library.settings.hide_bars);
        assert!(library.settings.dither_images);

        library.settings.font = Font::Sans;
        library.settings.font_size = 24;
        library.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#""font": "sans""#));
        assert_eq!(Library::load(&path).unwrap().settings, library.settings);
    }

    #[test]
    fn invalid_file_is_moved_aside() {
        let dir = temp_dir("invalid");
        let path = dir.join("library.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Library::load(&path).unwrap(), Library::default());
        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(path.with_extension("json.bad")).unwrap(),
            "{ not json"
        );
    }

    /// The paths differ only in case, which is the same path on Windows.
    #[test]
    #[cfg(target_os = "windows")]
    fn open_matches_by_path_then_id() {
        let mut library = Library::default();
        let added = library.open(info("a", "C:\\Books\\a.epub"), time(200));
        added.position = Position {
            spine: 1,
            offset: 5,
        };
        assert_eq!(added.added, time(200));

        let same_path = library.open(info("a2", "c:\\books\\A.epub"), time(300));
        assert_eq!(same_path.id, "a2");
        assert_eq!(
            same_path.position,
            Position {
                spine: 1,
                offset: 5
            }
        );
        assert_eq!(same_path.added, time(200));
        assert_eq!(same_path.last_opened, Some(time(300)));

        let moved = library.open(info("a2", "D:\\Moved\\a.epub"), time(400));
        assert_eq!(moved.path, PathBuf::from("D:\\Moved\\a.epub"));
        assert_eq!(
            moved.position,
            Position {
                spine: 1,
                offset: 5
            }
        );

        library.open(info("b", "D:\\b.epub"), time(500));
        assert_eq!(library.books.len(), 2);
        assert_eq!(library.last_book.as_deref(), Some("b"));
    }

    #[test]
    fn a_copy_of_a_book_is_the_same_entry() {
        let dir = temp_dir("copy");
        let first = dir.join("first.epub");
        fs::write(&first, "x").unwrap();
        let mut library = Library::default();
        library.add(info("a", first.to_str().unwrap()), time(100));
        library.open(info("a", "D:\\copy.epub"), time(200));
        assert_eq!(library.books.len(), 1);
        assert_eq!(library.books[0].path, first);
        assert_eq!(library.last_book.as_deref(), Some("a"));
        assert!(library.remove("a").is_some());
        assert!(library.books.is_empty());
    }

    #[test]
    fn copies_merge_on_load() {
        let dir = temp_dir("merge");
        let first = dir.join("first.epub");
        fs::write(&first, "x").unwrap();
        let mut library = Library::default();
        library.add(info("a", first.to_str().unwrap()), time(100));
        library.add(info("b", "D:\\b.epub"), time(150));
        let mut copy = library.books[0].clone();
        copy.path = PathBuf::from("D:\\gone\\copy.epub");
        copy.added = time(200);
        copy.last_opened = Some(time(300));
        copy.position = Position {
            spine: 2,
            offset: 7,
        };
        library.books.push(copy);
        let path = dir.join("library.json");
        library.save(&path).unwrap();

        let loaded = Library::load(&path).unwrap();
        assert_eq!(loaded.books.len(), 2);
        let book = loaded.book("a").unwrap();
        assert_eq!(book.path, first);
        assert_eq!(book.added, time(100));
        assert_eq!(book.last_opened, Some(time(300)));
        assert_eq!(
            book.position,
            Position {
                spine: 2,
                offset: 7
            }
        );
    }

    #[test]
    fn unchanged_and_missing() {
        let dir = temp_dir("missing-files");
        let present = dir.join("present.epub");
        fs::write(&present, "x").unwrap();
        let mut library = Library::default();
        library.add(info("a", present.to_str().unwrap()), time(100));
        library.add(info("b", "D:\\gone.epub"), time(100));
        assert!(library.unchanged(&present, 10, time(100)));
        assert!(!library.unchanged(&present, 11, time(100)));
        assert!(!library.unchanged(&present, 10, time(101)));

        assert!(library.update_missing());
        assert!(!library.books[0].missing);
        assert!(library.books[1].missing);
        assert!(!library.update_missing());

        library.add(info("b", "D:\\found.epub"), time(300));
        assert_eq!(library.books.len(), 2);
        assert!(!library.books[1].missing);
    }

    #[test]
    fn edited_details_stay_after_a_scan() {
        let mut library = Library::default();
        let book = library.add(info("a", "D:\\a.epub"), time(100));
        book.set_details("  New title ", "");
        assert_eq!(book.shown_title(), "New title");
        assert_eq!(book.shown_author(), "Author");
        assert_eq!(book.custom_author, None);

        let mut changed = info("a", "D:\\a.epub");
        changed.title = "Title 2".into();
        let book = library.add(changed, time(200));
        assert_eq!(book.title, "Title 2");
        assert_eq!(book.shown_title(), "New title");

        book.set_details("", "Author");
        assert_eq!(
            (book.custom_title.as_deref(), book.custom_author.as_deref()),
            (None, None)
        );
        assert_eq!(book.shown_title(), "Title 2");
    }

    #[test]
    fn remove_hides_books_in_watched_folders() {
        let mut library = Library::default();
        library.folders.push("D:\\Books\\".into());
        library.add(info("a", "d:\\books\\sub\\a.epub"), time(100));
        library.add(info("b", "D:\\Books2\\b.epub"), time(100));
        library.last_book = Some("b".into());

        assert_eq!(library.remove("a"), None);
        assert!(library.books[0].hidden);
        assert_eq!(library.remove("b").map(|b| b.id), Some("b".into()));
        assert_eq!(library.books.len(), 1);
        assert_eq!(library.last_book, None);
    }
}

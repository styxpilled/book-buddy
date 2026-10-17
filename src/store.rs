use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use book_buddy::library::Library;
use gpui::Global;

pub struct Store {
    pub library: Library,
    /// `None` when the library file could not be read. Then nothing is saved,
    /// so the file is not overwritten.
    path: Option<PathBuf>,
    covers: PathBuf,
    /// How many times the app wrote the cover of a book, by ID. The image
    /// cache loads a cover again when this changes.
    revisions: HashMap<String, u32>,
}

impl Global for Store {}

impl Store {
    pub fn load() -> Self {
        let dir = data_dir();
        let path = dir.join("library.json");
        let covers = dir.join("covers");
        match Library::load(&path) {
            Ok(library) => Self {
                library,
                path: Some(path),
                covers,
                revisions: HashMap::new(),
            },
            Err(e) => {
                log::error!("{e:#}. Changes to the library will not be saved.");
                Self {
                    library: Library::default(),
                    path: None,
                    covers,
                    revisions: HashMap::new(),
                }
            }
        }
    }

    pub fn save(&self) {
        if let Some(path) = &self.path
            && let Err(e) = self.library.save(path)
        {
            log::error!("{e:#}");
        }
    }

    pub fn last_book_path(&self) -> Option<PathBuf> {
        let id = self.library.last_book.as_deref()?;
        let path = &self.library.book(id)?.path;
        path.exists().then(|| path.clone())
    }

    /// Removes a book and its cover thumbnail, or hides it (see `Library::remove`).
    pub fn remove_book(&mut self, id: &str) {
        if let Some(removed) = self.library.remove(id) {
            self.remove_cover(&removed.id);
        }
        self.save();
    }

    /// Removes all books, their cover thumbnails and the watched folders. The
    /// book files stay.
    pub fn clear_library(&mut self) {
        for book in &self.library.books {
            self.remove_cover(&book.id);
        }
        self.library.clear();
        self.save();
    }

    fn remove_cover(&self, id: &str) {
        for cover in [
            cover_path(&self.covers, id),
            text_cover_path(&self.covers, id),
        ] {
            if let Err(e) = fs::remove_file(&cover)
                && e.kind() != ErrorKind::NotFound
            {
                log::warn!("{}: {e}", cover.display());
            }
        }
    }

    pub fn covers(&self) -> &Path {
        &self.covers
    }

    /// The cover image of a book, or else its generated cover, if the file exists.
    pub fn shown_cover(&self, id: &str) -> Option<PathBuf> {
        [
            cover_path(&self.covers, id),
            text_cover_path(&self.covers, id),
        ]
        .into_iter()
        .find(|path| path.exists())
    }

    pub fn has_cover_image(&self, id: &str) -> bool {
        cover_path(&self.covers, id).exists()
    }

    pub fn cover_written(&mut self, id: &str) {
        *self.revisions.entry(id.to_string()).or_default() += 1;
    }

    pub fn cover_revision(&self, id: &str) -> u32 {
        self.revisions.get(id).copied().unwrap_or(0)
    }
}

/// Cover thumbnails are named by book ID. A file name cannot contain `:`.
pub fn cover_path(covers: &Path, id: &str) -> PathBuf {
    covers.join(file_name(id))
}

/// Generated covers have one pixel for each dot.
pub fn text_cover_path(covers: &Path, id: &str) -> PathBuf {
    covers.join(TEXT_COVERS).join(file_name(id))
}

pub fn is_text_cover(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|dir| dir == TEXT_COVERS)
}

const TEXT_COVERS: &str = "text";

fn file_name(id: &str) -> String {
    format!("{}.png", id.replace(':', "-"))
}

/// The data is in Application Support. Dev builds use a separate directory.
#[cfg(target_os = "macos")]
fn data_dir() -> PathBuf {
    let name = if cfg!(debug_assertions) {
        "book-buddy-dev"
    } else {
        "book-buddy"
    };
    std::env::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support")
        .join(name)
}

/// The app is portable: its data is next to the exe.
#[cfg(not(target_os = "macos"))]
fn data_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .unwrap_or(Path::new("."))
        .join("book-buddy-data")
}

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
            },
            Err(e) => {
                log::error!("{e:#}. Changes to the library will not be saved.");
                Self {
                    library: Library::default(),
                    path: None,
                    covers,
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

    /// Removes all books, their cover thumbnails and the watched folders. The
    /// book files stay.
    pub fn clear_library(&mut self) {
        for book in &self.library.books {
            let cover = cover_path(&self.covers, &book.id);
            if let Err(e) = fs::remove_file(&cover)
                && e.kind() != ErrorKind::NotFound
            {
                log::warn!("{}: {e}", cover.display());
            }
        }
        self.library.clear();
        self.save();
    }

    pub fn covers(&self) -> &Path {
        &self.covers
    }
}

/// Cover thumbnails are named by book ID. A file name cannot contain `:`.
pub fn cover_path(covers: &Path, id: &str) -> PathBuf {
    covers.join(format!("{}.png", id.replace(':', "-")))
}

/// The app is portable: its data is next to the exe.
fn data_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .unwrap_or(Path::new("."))
        .join("book-buddy-data")
}

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use book_buddy::book::Book;
use book_buddy::cover;
use book_buddy::dither::{Adjust, dots};
use book_buddy::library::{BookEntry, FileInfo};
use chrono::{DateTime, Utc};
use gpui::{App, AppContext, AsyncApp};

use crate::store::{Store, cover_path, text_cover_path};
use crate::ui::style;

/// Cover thumbnails are made at 2x the size in the library grid.
pub const COVER_WIDTH: u32 = 160;
pub const COVER_HEIGHT: u32 = 240;

/// Opens a book, and makes its cover thumbnail if there is none yet.
pub fn book_file(path: &Path, covers: &Path) -> Result<(Book, FileInfo)> {
    let book = Book::open(path)?;
    let metadata = fs::metadata(path)?;
    let mut info = FileInfo {
        id: book.id.clone(),
        path: path.to_path_buf(),
        size: metadata.len(),
        modified: metadata.modified()?.into(),
        title: book.title.clone(),
        author: book.authors.join(", "),
        cover_adjust: Adjust::NONE,
    };
    let cover = cover_path(covers, &book.id);
    if !cover.exists() {
        match write_cover(&book, &cover) {
            Ok(adjust) => info.cover_adjust = adjust,
            Err(e) => log::warn!("no cover for {}: {e:#}", path.display()),
        }
    }
    Ok((book, info))
}

/// Writes the thumbnail and returns the auto levels for it.
fn write_cover(book: &Book, cover: &Path) -> Result<Adjust> {
    let Some(href) = &book.cover else {
        return Ok(Adjust::NONE);
    };
    let image = image::load_from_memory(&book.resource(href)?)?;
    let thumbnail = image.thumbnail(COVER_WIDTH * 2, COVER_HEIGHT * 2);
    if let Some(dir) = cover.parent() {
        fs::create_dir_all(dir)?;
    }
    let thumbnail = thumbnail.to_rgba8();
    thumbnail.save(cover)?;
    let mut adjust = Adjust::NONE;
    (adjust.black, adjust.white) = adjust.auto_levels(&dots(&thumbnail, style::DITHER_CELL));
    Ok(adjust)
}

/// Writes the generated cover of a book from its settings, title and author.
pub fn write_text_cover(book: &BookEntry, covers: &Path) -> Result<()> {
    let path = text_cover_path(covers, &book.id);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let cover = &book.text_cover();
    cover::render(
        cover,
        book.shown_title(),
        book.shown_author(),
        style::DITHER_CELL,
    )
    .save(path)?;
    Ok(())
}

/// False if the file is missing or was made at another size.
fn text_cover_is_current(path: &Path) -> bool {
    let size = (
        cover::WIDTH * style::DITHER_CELL,
        cover::HEIGHT * style::DITHER_CELL,
    );
    image::image_dimensions(path).is_ok_and(|dimensions| dimensions == size)
}

/// Writes the generated cover again if the book has no cover image.
pub fn update_text_cover(id: &str, store: &mut Store) {
    let Some(book) = store.library.book(id) else {
        return;
    };
    if store.has_cover_image(id) {
        return;
    }
    match write_text_cover(book, store.covers()) {
        Ok(()) => store.cover_written(id),
        Err(e) => log::warn!("cannot write the cover of {id}: {e:#}"),
    }
}

/// Writes the generated covers that are missing or old, in the background.
async fn write_text_covers(cx: &mut AsyncApp) {
    let (books, covers) = cx.read_global(|store: &Store, _| {
        (store.library.books.clone(), store.covers().to_path_buf())
    });
    let written = cx
        .background_spawn(async move {
            books
                .iter()
                .filter(|b| {
                    !cover_path(&covers, &b.id).exists()
                        && !text_cover_is_current(&text_cover_path(&covers, &b.id))
                })
                .filter_map(|b| match write_text_cover(b, &covers) {
                    Ok(()) => Some(b.id.clone()),
                    Err(e) => {
                        log::warn!("cannot write the cover of {}: {e:#}", b.id);
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .await;
    if !written.is_empty() {
        cx.update_global(|store: &mut Store, _| {
            for id in &written {
                store.cover_written(id);
            }
        });
    }
}

/// Scans the watched folders in the background. The library updates as books are found.
pub fn start(cx: &mut App) {
    let store = cx.global::<Store>();
    let folders = store.library.folders.clone();
    cx.spawn(async move |cx| {
        update_missing(cx);
        write_text_covers(cx).await;
        let files = cx
            .background_spawn(async move { epub_files(&folders) })
            .await;
        for (path, size, modified) in files {
            let unchanged =
                cx.read_global(|store: &Store, _| store.library.unchanged(&path, size, modified));
            if !unchanged {
                add(path, cx).await;
            }
        }
        update_missing(cx);
        write_text_covers(cx).await;
    })
    .detach();
}

/// Adds files from the open dialog or from drag-and-drop. Folders are
/// searched with their subfolders.
pub fn import(paths: Vec<PathBuf>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let files = cx
            .background_spawn(async move {
                let (dirs, files): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| p.is_dir());
                let mut files: Vec<_> = files.into_iter().filter(|p| is_epub(p)).collect();
                files.extend(epub_files(&dirs).into_iter().map(|(path, ..)| path));
                files
            })
            .await;
        for path in files {
            add(path, cx).await;
        }
        write_text_covers(cx).await;
    })
    .detach();
}

async fn add(path: PathBuf, cx: &mut AsyncApp) {
    let covers = cx.read_global(|store: &Store, _| store.covers().to_path_buf());
    let shown = path.display().to_string();
    match cx
        .background_spawn(async move { book_file(&path, &covers) })
        .await
    {
        Ok((_, info)) => cx.update_global(|store: &mut Store, _| {
            store.library.add(info, Utc::now());
            store.save();
        }),
        Err(e) => log::warn!("cannot add {shown}: {e:#}"),
    }
}

fn update_missing(cx: &mut AsyncApp) {
    cx.update_global(|store: &mut Store, _| {
        if store.library.update_missing() {
            store.save();
        }
    });
}

/// macOS writes `._` files next to the real files on some drives.
fn is_epub(path: &Path) -> bool {
    let apple_double = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with("._"));
    !apple_double
        && path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("epub"))
}

fn epub_files(folders: &[PathBuf]) -> Vec<(PathBuf, u64, DateTime<Utc>)> {
    let mut files = Vec::new();
    let mut dirs = folders.to_vec();
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            log::warn!("cannot read {}", dir.display());
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let path = entry.path();
            if metadata.is_dir() {
                dirs.push(path);
            } else if is_epub(&path)
                && let Ok(modified) = metadata.modified()
            {
                files.push((path, metadata.len(), modified.into()));
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_books_in_subfolders() {
        let dir = std::env::temp_dir().join(format!("book-buddy-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a/b")).unwrap();
        for file in [
            "top.epub",
            "a/mid.EPUB",
            "a/b/deep.epub",
            "a/b/._deep.epub",
            "a/notes.txt",
        ] {
            fs::write(dir.join(file), b"").unwrap();
        }
        let mut found: Vec<_> = epub_files(&[dir.clone()])
            .into_iter()
            .map(|(path, ..)| path.strip_prefix(&dir).unwrap().to_path_buf())
            .collect();
        found.sort();
        assert_eq!(
            found,
            ["a/b/deep.epub", "a/mid.EPUB", "top.epub"].map(PathBuf::from)
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}

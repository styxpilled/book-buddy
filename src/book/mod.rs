mod xhtml;

#[cfg(test)]
pub(crate) use xhtml::parse as parse_xhtml;
pub use xhtml::{Block, BlockKind, Chapter, Marker, Run};

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use percent_encoding::percent_decode_str;
use rbook::Epub;

pub struct Book {
    epub: Epub,
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    /// Absolute href of the cover image.
    pub cover: Option<String>,
    /// Absolute hrefs of the chapters in reading order.
    pub spine: Vec<String>,
    pub toc: Vec<TocEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    pub label: String,
    pub href: Option<String>,
    pub depth: usize,
}

impl Book {
    pub fn open(path: &Path) -> Result<Self> {
        let epub = Epub::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        let metadata = epub.metadata();
        let id = content_hash(path)?;
        let title = match metadata.title() {
            Some(title) => title.value().trim().to_string(),
            None => path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        };
        let authors = metadata
            .creators()
            .map(|c| c.value().trim().to_string())
            .collect();
        let cover = epub.manifest().cover_image().map(|e| e.href().to_string());
        let spine = epub
            .spine()
            .iter()
            .filter_map(|s| s.manifest_entry())
            .map(|e| e.href().to_string())
            .collect();
        let toc = epub
            .toc()
            .contents()
            .map(|root| {
                root.flatten()
                    .map(|e| TocEntry {
                        label: e.label().trim().to_string(),
                        href: e.href().map(|h| h.to_string()),
                        depth: e.depth().saturating_sub(1),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            epub,
            id,
            title,
            authors,
            cover,
            spine,
            toc,
        })
    }

    /// A chapter that cannot be read becomes one error block.
    pub fn chapter(&self, index: usize) -> Chapter {
        let Some(href) = self.spine.get(index) else {
            return Chapter::error(format!("No chapter {index}"));
        };
        match self.epub.read_resource_str(href.as_str()) {
            Ok(source) => xhtml::parse(&source, href),
            Err(e) => Chapter::error(format!("Cannot read {href}: {e}")),
        }
    }

    pub fn resource(&self, href: &str) -> Result<Vec<u8>> {
        Ok(self.epub.read_resource_bytes(href)?)
    }

    /// Returns the spine index and the fragment of an absolute href.
    pub fn locate<'a>(&self, href: &'a str) -> Option<(usize, Option<&'a str>)> {
        let (path, fragment) = match href.split_once('#') {
            Some((path, fragment)) => (path, Some(fragment)),
            None => (href, None),
        };
        let path = percent_decode_str(path).decode_utf8_lossy();
        let index = self
            .spine
            .iter()
            .position(|s| percent_decode_str(s).decode_utf8_lossy() == path)?;
        Some((index, fragment))
    }
}

/// FNV-1a, so the ID stays the same across Rust versions. Many books share
/// placeholder `dc:identifier` values, so the ID comes from the file contents.
fn content_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0; 64 * 1024];
    let mut hash = 0xcbf29ce484222325u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        for &b in &buffer[..n] {
            hash = (hash ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!("fnv1a:{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::xhtml::describe;
    use super::*;
    use rbook::ebook::toc::TocEntryKind;
    use rbook::epub::metadata::EpubVersion;

    fn open(name: &str) -> Book {
        Book::open(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("books")
                .join(name),
        )
        .unwrap()
    }

    fn describe_all(chapter: &Chapter) -> Vec<String> {
        chapter.blocks.iter().map(describe).collect()
    }

    fn runs(block: &Block) -> &[Run] {
        match &block.kind {
            BlockKind::Heading { runs, .. } | BlockKind::Paragraph { runs } => runs,
            _ => &[],
        }
    }

    #[test]
    fn minimal_books() {
        for name in ["minimal-v2.epub", "minimal-v3.epub", "minimal-v3plus2.epub"] {
            let book = open(name);
            assert_eq!(book.title, "Your title here", "{name}");
            assert!(book.authors.is_empty(), "{name}");
            assert_eq!(book.cover, None, "{name}");
            assert_eq!(book.spine.len(), 1, "{name}");
            assert_eq!(
                book.toc,
                [TocEntry {
                    label: "Section 1".into(),
                    href: Some(book.spine[0].clone()),
                    depth: 0
                }],
                "{name}"
            );

            let chapter = book.chapter(0);
            assert_eq!(
                describe_all(&chapter),
                ["h1 Section 1", "p This is a paragraph."],
                "{name}"
            );
            let offsets: Vec<_> = chapter.blocks.iter().map(|b| b.offset).collect();
            assert_eq!(offsets, [0, 10], "{name}");
            assert_eq!(chapter.len, 31, "{name}");
        }
    }

    #[test]
    fn ids_come_from_file_contents() {
        let names = [
            "minimal-v2.epub",
            "minimal-v3.epub",
            "minimal-v3plus2.epub",
            "basic-v3plus2.epub",
        ];
        let ids: Vec<_> = names.iter().map(|name| open(name).id).collect();
        assert!(
            ids.iter()
                .all(|id| id.starts_with("fnv1a:") && id.len() == 22)
        );
        for (i, id) in ids.iter().enumerate() {
            assert!(
                !ids[i + 1..].contains(id),
                "{} has a duplicate ID",
                names[i]
            );
        }
        assert_eq!(open(names[0]).id, ids[0]);
    }

    #[test]
    fn toc_source() {
        let sources = |name| {
            let book = open(name);
            let toc = book.epub.toc();
            (
                toc.by_kind_version(TocEntryKind::Toc, EpubVersion::EPUB3)
                    .is_some(),
                toc.by_kind_version(TocEntryKind::Toc, EpubVersion::EPUB2)
                    .is_some(),
            )
        };
        assert_eq!(sources("minimal-v2.epub"), (false, true));
        assert_eq!(sources("minimal-v3.epub"), (true, false));
        assert_eq!(sources("minimal-v3plus2.epub"), (true, false));
    }

    #[test]
    fn basic_book() {
        let book = open("basic-v3plus2.epub");
        assert_eq!(book.authors, ["Hingle McCringleberry"]);
        assert_eq!(book.spine.len(), 3);

        let cover = book.cover.as_deref().unwrap();
        assert_eq!(cover, "/EPUB/images/cover.jpg");
        assert_eq!(book.resource(cover).unwrap()[..2], [0xFF, 0xD8]);

        let labels: Vec<_> = book
            .toc
            .iter()
            .map(|e| (e.label.as_str(), e.depth))
            .collect();
        assert_eq!(
            labels,
            [
                ("Ladle Rat Rotten Hut", 0),
                ("Guilty Looks Enter Tree Beers", 0)
            ]
        );
        assert_eq!(
            book.locate(book.toc[1].href.as_deref().unwrap()),
            Some((2, None))
        );

        assert_eq!(
            describe_all(&book.chapter(0)),
            ["img /EPUB/images/cover.jpg Cover Image"]
        );

        let one = book.chapter(1);
        assert_eq!(describe(&one.blocks[0]), "h1 Ladle Rat Rotten Hut");
        assert!(
            describe_all(&one)
                .iter()
                .any(|b| b.ends_with("an den—O bore!\""))
        );

        let two = book.chapter(2);
        let last = runs(two.blocks.last().unwrap()).last().unwrap();
        assert_eq!(
            last.text,
            "Debt florist's mush toe dentures furry ladle beer!"
        );
        assert!(last.italic && !last.bold);
    }

    #[test]
    fn ignores_book_styling() {
        let book = open("basic-v3plus2.epub");
        for index in 1..book.spine.len() {
            let chapter = book.chapter(index);
            for block in &chapter.blocks {
                assert!(matches!(
                    block.kind,
                    BlockKind::Heading { .. } | BlockKind::Paragraph { .. }
                ));
                assert!(runs(block).iter().all(|r| !r.bold && r.link.is_none()));
                let text = describe(block);
                assert!(
                    !text.contains("Redacted") && !text.contains("font-family"),
                    "{text}"
                );
            }
        }
    }

    /// Each entry goes to the start of its heading, or to the cover image.
    #[test]
    fn toc_entries_go_to_their_headings() {
        for name in [
            "minimal-v2.epub",
            "minimal-v3.epub",
            "minimal-v3plus2.epub",
            "basic-v3plus2.epub",
            "basic-test.epub",
        ] {
            let book = open(name);
            for entry in &book.toc {
                let href = entry.href.as_deref().unwrap();
                let (spine, fragment) = book.locate(href).expect(href);
                let chapter = book.chapter(spine);
                let offset = match fragment {
                    Some(fragment) => chapter.anchors[fragment],
                    None => 0,
                };
                let block = chapter
                    .blocks
                    .iter()
                    .find(|b| b.offset == offset)
                    .unwrap_or_else(|| panic!("{name} {href}: no block at {offset}"));
                assert!(
                    matches!(
                        block.kind,
                        BlockKind::Heading { .. } | BlockKind::Image { .. }
                    ),
                    "{name} {href}: {}",
                    describe(block)
                );
            }
        }
    }

    #[test]
    fn locate() {
        let book = open("basic-v3plus2.epub");
        assert_eq!(
            book.locate("/EPUB/xhtml/section0001.xhtml#x"),
            Some((1, Some("x")))
        );
        assert_eq!(book.locate("/EPUB/xhtml/missing.xhtml"), None);
    }
}

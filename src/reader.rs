use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::mem;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use book_buddy::book::{Block, BlockKind, Book, Chapter, Marker, Run};
use book_buddy::library::{FileInfo, Position};
use chrono::Utc;
use gpui::{
    AnyElement, App, AppContext, BorrowAppContext, Context, EventEmitter, FocusHandle, Focusable,
    FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveElement, IntoElement, ListAlignment,
    ListOffset, ListState, ParentElement, Render, RenderImage, SharedString, Styled, StyledText,
    Subscription, Task, UnderlineStyle, Window, div, img, list, px, relative,
};
use gpui_kit::component::{ActiveTheme, Theme};
use image::imageops::FilterType;
use image::{Frame, ImageReader};

use crate::Back;
use crate::scan;
use crate::store::Store;

const SERIF: &str = "Literata";
const MONO: &str = "Consolas";
const FONT_SIZE: f32 = 18.;
const HEADING_SCALE: [f32; 6] = [1.8, 1.5, 1.3, 1.15, 1.0, 1.0];
const COLUMN_WIDTH: f32 = 720.;
const MARGIN: f32 = 32.;
/// GPUI keeps a decoded image until it is dropped, so only this many stay decoded.
const DECODED_IMAGES: usize = 16;
/// Longest side of a decoded image, in pixels.
const IMAGE_MAX: u32 = 2048;
const SAVE_DELAY: Duration = Duration::from_secs(1);

pub fn init(cx: &mut App) {
    let fonts: [&'static [u8]; 8] = [
        include_bytes!("../assets/fonts/Literata-Regular.ttf"),
        include_bytes!("../assets/fonts/Literata-Bold.ttf"),
        include_bytes!("../assets/fonts/Literata-Italic.ttf"),
        include_bytes!("../assets/fonts/Literata-BoldItalic.ttf"),
        include_bytes!("../assets/fonts/Inter-Regular.ttf"),
        include_bytes!("../assets/fonts/Inter-Bold.ttf"),
        include_bytes!("../assets/fonts/Inter-Italic.ttf"),
        include_bytes!("../assets/fonts/Inter-BoldItalic.ttf"),
    ];
    cx.text_system()
        .add_fonts(fonts.into_iter().map(Cow::Borrowed).collect())
        .expect("bundled fonts must load");
}

enum Status {
    NoBook,
    Loading,
    Failed(String),
    Ready,
}

struct Item {
    block: Block,
    spine: usize,
    /// Characters in the block, as counted by `Block::offset`.
    len: usize,
    text: SharedString,
    styles: Vec<(Range<usize>, Run)>,
    chapter_start: bool,
}

impl Item {
    fn new(block: Block, spine: usize, chapter_start: bool) -> Self {
        let mut text = String::new();
        let mut styles = Vec::new();
        if let BlockKind::Heading { runs, .. }
        | BlockKind::Paragraph { runs }
        | BlockKind::ListItem { runs, .. } = &block.kind
        {
            for run in runs {
                let start = text.len();
                text.push_str(&run.text);
                if run.bold || run.italic || run.link.is_some() {
                    styles.push((start..text.len(), run.clone()));
                }
            }
        }
        let len = match &block.kind {
            BlockKind::Pre { text: pre } => pre.chars().count(),
            _ => text.chars().count(),
        };
        Self {
            block,
            spine,
            len,
            text: text.into(),
            styles,
            chapter_start,
        }
    }

    fn styled_text(&self, theme: &Theme) -> StyledText {
        let link = theme.link;
        StyledText::new(self.text.clone()).with_highlights(
            self.styles
                .iter()
                .map(move |(range, run)| (range.clone(), highlight(run, link))),
        )
    }
}

fn highlight(run: &Run, link: Hsla) -> HighlightStyle {
    let is_link = run.link.is_some();
    HighlightStyle {
        font_weight: run.bold.then_some(FontWeight::BOLD),
        font_style: run.italic.then_some(FontStyle::Italic),
        color: is_link.then_some(link),
        underline: is_link.then_some(UnderlineStyle {
            thickness: px(1.),
            color: Some(link),
            wavy: false,
        }),
        ..Default::default()
    }
}

struct Picture {
    bytes: Arc<Vec<u8>>,
    width: u32,
    height: u32,
    decode: Decode,
}

enum Decode {
    No,
    Running,
    Done(Arc<RenderImage>),
    Failed,
}

struct Loaded {
    chapter: Chapter,
    pictures: Vec<(String, Picture)>,
}

struct Opened {
    book: Arc<Book>,
    info: FileInfo,
}

pub struct Reader {
    status: Status,
    book_id: Option<String>,
    spine_len: usize,
    items: Vec<Item>,
    /// First item and text length of each loaded chapter.
    chapters: Vec<(usize, usize)>,
    list: ListState,
    /// The saved position, until its chapter is loaded.
    restore: Option<Position>,
    /// An item and the part of its height to scroll to, once the item is measured.
    adjust: Option<(usize, f32)>,
    save_task: Option<Task<()>>,
    pictures: HashMap<String, Picture>,
    decoded: VecDeque<String>,
    dropped: Vec<Arc<RenderImage>>,
    focus: FocusHandle,
    _quit: Subscription,
}

pub enum ReaderEvent {
    Close,
}

impl EventEmitter<ReaderEvent> for Reader {}

impl Focusable for Reader {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Reader {
    pub fn new(path: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Top, px(1000.));
        let this = cx.entity().downgrade();
        list.set_scroll_handler(move |_, _, cx| {
            this.update(cx, |reader, cx| reader.schedule_save(cx)).ok();
        });
        let mut reader = Self {
            status: Status::NoBook,
            book_id: None,
            spine_len: 0,
            items: Vec::new(),
            chapters: Vec::new(),
            list,
            restore: None,
            adjust: None,
            save_task: None,
            pictures: HashMap::new(),
            decoded: VecDeque::new(),
            dropped: Vec::new(),
            focus: cx.focus_handle(),
            _quit: cx.on_app_quit(|reader, cx| {
                reader.save_position(cx);
                async {}
            }),
        };
        if let Some(path) = path {
            reader.open(path, cx);
        }
        reader
    }

    fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.status = Status::Loading;
        let covers = cx.global::<Store>().covers().to_path_buf();
        cx.spawn(async move |this, cx| {
            let opened = cx
                .background_spawn(async move { open_book(path, covers) })
                .await;
            let book = match opened {
                Ok(opened) => {
                    let book = opened.book.clone();
                    if this
                        .update(cx, |reader, cx| reader.opened(opened, cx))
                        .is_err()
                    {
                        return;
                    }
                    book
                }
                Err(error) => {
                    this.update(cx, |reader, cx| reader.fail(format!("{error:#}"), cx))
                        .ok();
                    return;
                }
            };
            for index in 0..book.spine.len() {
                let book = book.clone();
                let loaded = cx
                    .background_spawn(async move { load_chapter(&book, index) })
                    .await;
                if this
                    .update(cx, |reader, cx| reader.append(loaded, cx))
                    .is_err()
                {
                    return;
                }
            }
            this.update(cx, |reader, cx| reader.loaded(cx)).ok();
        })
        .detach();
    }

    fn opened(&mut self, opened: Opened, cx: &mut Context<Self>) {
        self.book_id = Some(opened.info.id.clone());
        self.spine_len = opened.book.spine.len();
        self.restore = Some(cx.update_global(|store: &mut Store, _| {
            let position = store.library.open(opened.info, Utc::now()).position;
            store.save();
            position
        }));
    }

    /// Saves the position and releases the decoded images.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_position(cx);
        for picture in self.pictures.values_mut() {
            if let Decode::Done(image) = mem::replace(&mut picture.decode, Decode::No) {
                window.drop_image(image).ok();
            }
        }
        for image in self.dropped.drain(..) {
            window.drop_image(image).ok();
        }
    }

    fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        log::error!("{message}");
        self.status = Status::Failed(message);
        cx.notify();
    }

    fn append(&mut self, loaded: Loaded, cx: &mut Context<Self>) {
        let start = self.items.len();
        let spine = self.chapters.len();
        self.chapters.push((start, loaded.chapter.len));
        let blocks = loaded.chapter.blocks.into_iter().enumerate();
        self.items
            .extend(blocks.map(|(i, block)| Item::new(block, spine, i == 0)));
        for (href, picture) in loaded.pictures {
            self.pictures.entry(href).or_insert(picture);
        }
        self.list.splice(start..start, self.items.len() - start);
        if let Some(position) = self.restore
            && position.spine < self.chapters.len()
        {
            self.restore = None;
            self.scroll_to_position(position);
        }
        if self.restore.is_none() && !self.items.is_empty() {
            self.status = Status::Ready;
        }
        cx.notify();
    }

    fn loaded(&mut self, cx: &mut Context<Self>) {
        // The saved chapter is not in the book any more.
        self.restore = None;
        if self.items.is_empty() {
            self.fail("The book has no text.".into(), cx);
        } else {
            self.status = Status::Ready;
            cx.notify();
        }
    }

    fn scroll_to_position(&mut self, position: Position) {
        let (first, _) = self.chapters[position.spine];
        let end = self
            .chapters
            .get(position.spine + 1)
            .map_or(self.items.len(), |(start, _)| *start);
        let found = (first..end)
            .rev()
            .find(|&ix| self.items[ix].block.offset <= position.offset);
        // An empty chapter goes to the start of the next one.
        let Some(ix) = found.or((first < self.items.len()).then_some(first)) else {
            return;
        };
        let item = &self.items[ix];
        let into = position.offset.saturating_sub(item.block.offset);
        let part = if item.len == 0 {
            0.
        } else {
            (into as f32 / item.len as f32).min(1.)
        };
        self.list.scroll_to(ListOffset {
            item_ix: ix,
            offset_in_item: px(0.),
        });
        self.adjust = (part > 0.).then_some((ix, part));
    }

    /// The top block, and how far it is scrolled, as a part of its characters.
    fn position(&self) -> Option<Position> {
        let top = self.list.logical_scroll_top();
        let item = self.items.get(top.item_ix)?;
        let part = match self.list.bounds_for_item(top.item_ix) {
            Some(bounds) if bounds.size.height > px(0.) => {
                (top.offset_in_item / bounds.size.height).clamp(0., 1.)
            }
            _ => 0.,
        };
        let into = (part * item.len as f32).round() as usize;
        Some(Position {
            spine: item.spine,
            offset: item.block.offset + into.min(item.len),
        })
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        if self.restore.is_some() || self.adjust.is_some() {
            return;
        }
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            this.update(cx, |reader, cx| reader.save_position(cx)).ok();
        }));
    }

    fn save_position(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        if self.restore.is_some() || self.adjust.is_some() {
            return;
        }
        let (Some(id), Some(position)) = (self.book_id.as_deref(), self.position()) else {
            return;
        };
        let percent = (self.chapters.len() == self.spine_len).then(|| {
            let total: usize = self.chapters.iter().map(|(_, len)| len).sum();
            let before: usize = self.chapters[..position.spine]
                .iter()
                .map(|(_, len)| len)
                .sum();
            100. * (before + position.offset) as f32 / total.max(1) as f32
        });
        cx.update_global(|store: &mut Store, _| {
            let Some(entry) = store.library.book_mut(id) else {
                return;
            };
            if entry.position == position {
                return;
            }
            entry.position = position;
            if let Some(percent) = percent {
                entry.percent = percent.min(100.);
            }
            store.save();
        });
    }

    fn decode(&mut self, href: &str, cx: &mut Context<Self>) {
        let Some(picture) = self.pictures.get_mut(href) else {
            return;
        };
        if !matches!(picture.decode, Decode::No) {
            return;
        }
        picture.decode = Decode::Running;
        let bytes = picture.bytes.clone();
        let href = href.to_string();
        cx.spawn(async move |this, cx| {
            let image = cx
                .background_spawn(async move { decode_image(&bytes) })
                .await;
            this.update(cx, |reader, cx| reader.decoded(href, image, cx))
                .ok();
        })
        .detach();
    }

    fn decoded(&mut self, href: String, image: Option<Arc<RenderImage>>, cx: &mut Context<Self>) {
        let Some(picture) = self.pictures.get_mut(&href) else {
            return;
        };
        let Some(image) = image else {
            picture.decode = Decode::Failed;
            return;
        };
        picture.decode = Decode::Done(image);
        self.decoded.push_back(href);
        while self.decoded.len() > DECODED_IMAGES {
            let Some(old) = self.decoded.pop_front() else {
                break;
            };
            if let Some(picture) = self.pictures.get_mut(&old)
                && let Decode::Done(image) = mem::replace(&mut picture.decode, Decode::No)
            {
                self.dropped.push(image);
            }
        }
        cx.notify();
    }

    /// Returns the element and the href of an image that must be decoded.
    fn render_item(&self, ix: usize, window: &Window, cx: &App) -> (AnyElement, Option<String>) {
        let item = &self.items[ix];
        let theme = cx.theme();
        let viewport = window.viewport_size();
        let column = (f32::from(viewport.width) - 2. * MARGIN).clamp(100., COLUMN_WIDTH);
        let max_height = f32::from(viewport.height) * 0.9;
        let mut decode = None;

        let mut content = match &item.block.kind {
            BlockKind::Heading { level, .. } => {
                let scale = HEADING_SCALE[(*level as usize).clamp(1, 6) - 1];
                div()
                    .pt(px(FONT_SIZE * 0.8))
                    .pb(px(FONT_SIZE * 0.6))
                    .text_size(px(FONT_SIZE * scale))
                    .line_height(relative(1.25))
                    .font_weight(FontWeight::BOLD)
                    .child(item.styled_text(theme))
            }
            BlockKind::Paragraph { .. } => div()
                .pb(px(FONT_SIZE * 0.75))
                .child(item.styled_text(theme)),
            BlockKind::ListItem { depth, marker, .. } => {
                let marker = match marker {
                    Some(Marker::Bullet) => "•".to_string(),
                    Some(Marker::Number(n)) => format!("{n}."),
                    None => String::new(),
                };
                div()
                    .flex()
                    .pb(px(FONT_SIZE * 0.4))
                    .pl(px(FONT_SIZE * 1.5 * (*depth as f32 - 1.)))
                    .child(div().flex_none().w(px(FONT_SIZE * 1.8)).child(marker))
                    .child(div().flex_1().min_w_0().child(item.styled_text(theme)))
            }
            BlockKind::Pre { text } => div()
                .pb(px(FONT_SIZE * 0.75))
                .font_family(MONO)
                .text_size(px(FONT_SIZE * 0.85))
                .child(text.clone()),
            BlockKind::Image { href, alt } => {
                let frame = match self.pictures.get(href) {
                    Some(picture) => {
                        let (w, h) = (picture.width as f32, picture.height as f32);
                        let scale = (column / w).min(max_height / h).min(1.);
                        let frame = div().w(px(w * scale)).h(px(h * scale));
                        match &picture.decode {
                            Decode::Done(image) => frame.child(img(image.clone()).size_full()),
                            Decode::No => {
                                decode = Some(href.clone());
                                frame
                            }
                            Decode::Running => frame,
                            Decode::Failed => frame.child(alt.clone()),
                        }
                    }
                    None => div()
                        .italic()
                        .text_color(theme.muted_foreground)
                        .child(alt.clone()),
                };
                div()
                    .flex()
                    .justify_center()
                    .pb(px(FONT_SIZE * 0.75))
                    .child(frame)
            }
            BlockKind::Rule => div()
                .flex()
                .justify_center()
                .py(px(FONT_SIZE))
                .child(div().w(px(column * 0.3)).h(px(1.)).bg(theme.border)),
            BlockKind::Error { message } => div()
                .pb(px(FONT_SIZE * 0.75))
                .italic()
                .text_color(theme.danger)
                .child(message.clone()),
        };

        if item.block.quote > 0 {
            content = div()
                .ml(px(FONT_SIZE * (item.block.quote as f32 - 1.)))
                .mb(px(FONT_SIZE * 0.75))
                .pl(px(FONT_SIZE))
                .border_l_2()
                .border_color(theme.border)
                .child(content);
        }

        let mut row = div().w_full().flex().justify_center();
        if ix == 0 {
            row = row.pt(px(MARGIN));
        } else if item.chapter_start {
            row = row.pt(px(FONT_SIZE * 3.));
        }
        if ix + 1 == self.items.len() {
            row = row.pb(px(MARGIN * 2.));
        }
        let element = row
            .child(div().w(px(column)).child(content))
            .into_any_element();
        (element, decode)
    }
}

fn open_book(path: PathBuf, covers: PathBuf) -> anyhow::Result<Opened> {
    let (book, info) = scan::book_file(&path, &covers)?;
    Ok(Opened {
        book: Arc::new(book),
        info,
    })
}

fn load_chapter(book: &Book, index: usize) -> Loaded {
    let chapter = book.chapter(index);
    let pictures = chapter
        .blocks
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Image { href, .. } => Some(href),
            _ => None,
        })
        .filter_map(|href| {
            let bytes = book.resource(href).ok()?;
            let reader = ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()
                .ok()?;
            let (width, height) = reader.into_dimensions().ok()?;
            let picture = Picture {
                bytes: Arc::new(bytes),
                width,
                height,
                decode: Decode::No,
            };
            Some((href.clone(), picture))
        })
        .collect();
    Loaded { chapter, pictures }
}

fn decode_image(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let mut image = image::load_from_memory(bytes).ok()?;
    if image.width() > IMAGE_MAX || image.height() > IMAGE_MAX {
        image = image.resize(IMAGE_MAX, IMAGE_MAX, FilterType::Triangle);
    }
    let mut pixels = image.into_rgba8();
    // RenderImage is BGRA.
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![Frame::new(pixels)])))
}

impl Render for Reader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.dropped.drain(..) {
            window.drop_image(image).ok();
        }
        if let Some((ix, part)) = self.adjust {
            match self.list.bounds_for_item(ix) {
                Some(bounds) => {
                    self.list.scroll_to(ListOffset {
                        item_ix: ix,
                        offset_in_item: bounds.size.height * part,
                    });
                    self.adjust = None;
                }
                None => window.request_animation_frame(),
            }
        }

        let theme = cx.theme();
        let message = |text: SharedString| {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child(text)
                .into_any_element()
        };
        let body = match &self.status {
            Status::NoBook => message("Open a book: book-buddy <file.epub>".into()),
            Status::Loading => message("Loading...".into()),
            Status::Failed(error) => message(error.clone().into()),
            Status::Ready => {
                let view = cx.entity();
                list(self.list.clone(), move |ix, window, cx| {
                    let (element, decode) = view.read(cx).render_item(ix, window, cx);
                    if let Some(href) = decode {
                        view.update(cx, |reader, cx| reader.decode(&href, cx));
                    }
                    element
                })
                .size_full()
                .into_any_element()
            }
        };

        div()
            .id("reader")
            .key_context("Reader")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Back, _, cx| cx.emit(ReaderEvent::Close)))
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(SERIF)
            .text_size(px(FONT_SIZE))
            .line_height(relative(1.5))
            .child(body)
    }
}

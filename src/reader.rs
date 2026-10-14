use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::mem;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use book_buddy::book::{Block, BlockKind, Book, Chapter, Layout, Marker, Run};
use book_buddy::dither::{self, dither_small};
use book_buddy::library::{FileInfo, Font, Mode, Position};
use chrono::Utc;
use fast_image_resize::Resizer;
use gpui::{
    AnyElement, App, AppContext, BorderStyle, BorrowAppContext, Bounds, Context, DispatchPhase,
    Entity, EventEmitter, FocusHandle, Focusable, FontStyle, FontWeight, HighlightStyle, Hsla,
    InteractiveElement, IntoElement, KeyBinding, ListAlignment, ListOffset, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, RenderImage,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, StyledText, Subscription,
    Task, UnderlineStyle, UniformListScrollHandle, Window, actions, canvas, combine_highlights,
    div, fill, img, list, outline, point, prelude::FluentBuilder, px, relative, size, uniform_list,
};
use gpui_kit::component::{ActiveTheme, Theme};
use image::{DynamicImage, Frame, ImageReader};

mod pages;
mod search;
mod selection;

use crate::Back;
use crate::scan;
use crate::settings::OpenSettings;
use crate::store::Store;
use crate::style;
use crate::title_bar;
use pages::Pages;
use search::Search;
use selection::{PaintedText, Selection};

const SERIF: &str = "Literata";
const SANS: &str = "Inter";
#[cfg(target_os = "windows")]
const MONO: &str = "Consolas";
#[cfg(not(target_os = "windows"))]
const MONO: &str = "IBM Plex Mono";
const LINE_SPACING: f32 = 1.5;
const HEADING_SPACING: f32 = 1.25;
const PRE_SCALE: f32 = 0.85;
/// Space below a block quote, in font sizes.
const QUOTE_GAP: f32 = 0.75;
/// Space for one empty element of the book, in font sizes.
const GAP: f32 = 1.;
const MAX_GAP: u8 = 4;
/// Indent of a verse inset, in font sizes.
const INSET: f32 = 2.;
const HEADING_SCALE: [f32; 6] = [1.8, 1.5, 1.3, 1.15, 1.0, 1.0];
const COLUMN_WIDTH: f32 = 720.;
const MARGIN: f32 = 32.;
/// GPUI keeps a decoded image until it is dropped, so only this many stay decoded.
const DECODED_IMAGES: usize = 16;
/// Longest side of a decoded image, in pixels.
const IMAGE_MAX: u32 = 2048;
/// Width of one dot of a dithered image, in pixels of the widest column.
const DITHER_DOT: f32 = 2.;
/// Blocks after the position of an opened book whose images decode at once.
const DECODE_AHEAD: usize = 8;
/// The longest time that an opened book waits for these images.
const IMAGE_WAIT: Duration = Duration::from_millis(300);
const SAVE_DELAY: Duration = Duration::from_secs(1);
const TOP_BAR_HEIGHT: f32 = 40.;
const BOTTOM_BAR_HEIGHT: f32 = 38.;
/// With hidden bars, a bar shows when the pointer is this close to its edge.
const BAR_TRIGGER: f32 = 44.;
/// The space above and below the text of the reader.
const TEXT_TOP: f32 = 48.;
const TEXT_BOTTOM: f32 = TEXT_TOP;
const TOC_WIDTH: f32 = 330.;
/// Distance between the dots of the seek bar.
const SEEK_STEP: f32 = 6.;
const TOC_ROW_HEIGHT: f32 = 30.;

actions!(
    reader,
    [
        PageDown,
        PageUp,
        LineDown,
        LineUp,
        ChapterStart,
        ChapterEnd,
        BookStart,
        BookEnd,
        ToggleToc,
        GoBack,
        OpenSearch,
        NextMatch,
        PreviousMatch,
        CopySelection,
    ]
);

pub fn init(cx: &mut App) {
    let context = Some("Reader");
    cx.bind_keys([
        KeyBinding::new("right", PageDown, context),
        KeyBinding::new("pagedown", PageDown, context),
        KeyBinding::new("space", PageDown, context),
        KeyBinding::new("left", PageUp, context),
        KeyBinding::new("pageup", PageUp, context),
        KeyBinding::new("shift-space", PageUp, context),
        KeyBinding::new("down", LineDown, context),
        KeyBinding::new("up", LineUp, context),
        KeyBinding::new("home", ChapterStart, context),
        KeyBinding::new("end", ChapterEnd, context),
        KeyBinding::new("ctrl-home", BookStart, context),
        KeyBinding::new("ctrl-end", BookEnd, context),
        KeyBinding::new("t", ToggleToc, context),
        KeyBinding::new("alt-left", GoBack, context),
        KeyBinding::new("ctrl-f", OpenSearch, context),
        KeyBinding::new("f3", NextMatch, context),
        KeyBinding::new("shift-f3", PreviousMatch, context),
        KeyBinding::new("ctrl-c", CopySelection, context),
        KeyBinding::new("escape", Back, Some("Search")),
        KeyBinding::new("f3", NextMatch, Some("Search")),
        KeyBinding::new("shift-f3", PreviousMatch, Some("Search")),
    ]);

    macro_rules! styles {
        ($($name:literal),*) => {
            [$(
                include_bytes!(concat!("../assets/fonts/", $name, "-Regular.ttf")) as &'static [u8],
                include_bytes!(concat!("../assets/fonts/", $name, "-Bold.ttf")),
                include_bytes!(concat!("../assets/fonts/", $name, "-Italic.ttf")),
                include_bytes!(concat!("../assets/fonts/", $name, "-BoldItalic.ttf")),
            )*]
        };
    }
    // `style::init` adds IBM Plex Mono Regular for the UI.
    let fonts = styles!(
        "Literata",
        "Inter",
        "Newsreader",
        "SourceSerif4",
        "Alegreya",
        "EBGaramond"
    )
    .into_iter()
    .chain([
        include_bytes!("../assets/fonts/IBMPlexMono-Bold.ttf") as &'static [u8],
        include_bytes!("../assets/fonts/IBMPlexMono-Italic.ttf"),
        include_bytes!("../assets/fonts/IBMPlexMono-BoldItalic.ttf"),
    ]);
    cx.text_system()
        .add_fonts(fonts.map(Cow::Borrowed).collect())
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
    layout: Layout,
}

impl Item {
    fn new(block: Block, spine: usize, chapter_start: bool, layout: Layout) -> Self {
        let text = block.text();
        let mut styles = Vec::new();
        if let BlockKind::Heading { runs, .. }
        | BlockKind::Paragraph { runs }
        | BlockKind::ListItem { runs, .. } = &block.kind
        {
            let mut start = 0;
            for run in runs {
                let end = start + run.text.len();
                if run.bold || run.italic || run.link.is_some() {
                    styles.push((start..end, run.clone()));
                }
                start = end;
            }
        }
        let len = text.chars().count();
        Self {
            block,
            spine,
            len,
            text: text.into(),
            styles,
            chapter_start,
            layout,
        }
    }

    /// `extra` are highlights for search and selection.
    fn styled_text(
        &self,
        theme: &Theme,
        extra: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> StyledText {
        let link = theme.link;
        let styles = self
            .styles
            .iter()
            .map(move |(range, run)| (range.clone(), highlight(run, link)));
        StyledText::new(self.text.clone()).with_highlights(combine_highlights(styles, extra))
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

struct LoadedChapter {
    first: usize,
    len: usize,
    anchors: HashMap<String, usize>,
}

/// A place in the book. The anchor, when the chapter has it, overrides the offset.
struct Target {
    spine: usize,
    anchor: Option<String>,
    offset: usize,
}

impl From<Position> for Target {
    fn from(position: Position) -> Self {
        Self {
            spine: position.spine,
            anchor: None,
            offset: position.offset,
        }
    }
}

/// A scroll that needs the layout of the list first.
enum Adjust {
    /// Scroll to this part of the item height, less the margin, once the item
    /// is measured.
    Part { ix: usize, part: f32, margin: f32 },
    /// Scroll up one screen, after one layout measures the items above the top.
    Above { laid_out: bool },
}

pub struct Reader {
    status: Status,
    book: Option<Arc<Book>>,
    book_id: Option<String>,
    spine_len: usize,
    /// The spine index and anchor of each entry of the table of contents.
    toc: Vec<Option<(usize, Option<String>)>>,
    items: Vec<Item>,
    chapters: Vec<LoadedChapter>,
    list: ListState,
    /// A place to go to when its chapter is loaded: the saved position, or a jump.
    pending: Option<Target>,
    adjust: Option<Adjust>,
    /// Positions before link jumps, for Alt+Left.
    back: Vec<Position>,
    save_task: Option<Task<()>>,
    pictures: HashMap<String, Picture>,
    /// The tones that images are dithered to, dark and light. `None` keeps
    /// the colors of the images.
    tones: Option<Tones>,
    /// Images that must be decoded before the opened book shows.
    waiting: Vec<String>,
    decoded: VecDeque<String>,
    dropped: Vec<Arc<RenderImage>>,
    toc_open: bool,
    search: Search,
    selection: Option<Selection>,
    painted: PaintedText,
    toc_scroll: UniformListScrollHandle,
    hide_bars: bool,
    /// With hidden bars, the bars that the pointer is near.
    top_bar: bool,
    bottom_bar: bool,
    /// The list corrects a scroll past the end in its layout, after the render.
    /// One more frame shows the corrected position in the bars.
    redraw: bool,
    /// The left button went down on the seek bar and is not up yet.
    seeking: bool,
    font: Font,
    font_size: f32,
    mode: Mode,
    pages: Pages,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
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
            this.update(cx, |reader, cx| reader.scrolled(cx)).ok();
        });
        let subscriptions = vec![
            cx.on_app_quit(|reader, cx| {
                reader.save_position(cx);
                async {}
            }),
            cx.observe_global::<Store>(|reader, cx| reader.apply_settings(cx)),
        ];
        let settings = &cx.global::<Store>().library.settings;
        let (font, font_size) = (settings.font, settings.font_size as f32);
        let (mode, hide_bars) = (settings.mode, settings.hide_bars);
        let mut reader = Self {
            status: Status::NoBook,
            book: None,
            book_id: None,
            spine_len: 0,
            toc: Vec::new(),
            items: Vec::new(),
            chapters: Vec::new(),
            list,
            pending: None,
            adjust: None,
            back: Vec::new(),
            save_task: None,
            pictures: HashMap::new(),
            tones: image_tones(cx),
            waiting: Vec::new(),
            decoded: VecDeque::new(),
            dropped: Vec::new(),
            toc_open: false,
            search: Search::default(),
            selection: None,
            painted: PaintedText::default(),
            toc_scroll: UniformListScrollHandle::new(),
            hide_bars,
            top_bar: false,
            bottom_bar: false,
            redraw: false,
            seeking: false,
            font,
            font_size,
            mode,
            pages: Pages::default(),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
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
        let book = &opened.book;
        self.toc = book
            .toc
            .iter()
            .map(|entry| {
                let (spine, fragment) = book.locate(entry.href.as_deref()?)?;
                Some((spine, fragment.map(str::to_string)))
            })
            .collect();
        self.book = Some(opened.book);
        let position = cx.update_global(|store: &mut Store, _| {
            // The open is recorded when the book shows, so that the order of
            // the library does not change while it is still on screen.
            let position = store.library.add(opened.info, Utc::now()).position;
            store.save();
            position
        });
        self.pending = Some(position.into());
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

    /// The list keeps the part of the top block that is scrolled, so the
    /// position stays the same.
    fn apply_settings(&mut self, cx: &mut Context<Self>) {
        let settings = &cx.global::<Store>().library.settings;
        let (font, font_size, mode) = (settings.font, settings.font_size as f32, settings.mode);
        let hide_bars = settings.hide_bars;
        if (font, font_size) != (self.font, self.font_size) {
            self.font = font;
            self.font_size = font_size;
            self.list.remeasure();
            cx.notify();
        }
        if mode != self.mode {
            let position = self.position();
            self.mode = mode;
            if let Some(position) = position {
                self.scroll_to_position(position);
            }
            cx.notify();
        }
        if hide_bars != self.hide_bars {
            self.hide_bars = hide_bars;
            self.top_bar = false;
            self.bottom_bar = false;
            cx.notify();
        }
    }

    /// The height of the bars that take space from the text.
    fn bars_height(&self) -> f32 {
        if self.hide_bars {
            0.
        } else {
            TOP_BAR_HEIGHT + BOTTOM_BAR_HEIGHT
        }
    }

    pub(super) fn line_height(&self) -> f32 {
        self.font_size * LINE_SPACING
    }

    fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        log::error!("{message}");
        self.status = Status::Failed(message);
        cx.notify();
    }

    fn append(&mut self, loaded: Loaded, cx: &mut Context<Self>) {
        let start = self.items.len();
        let spine = self.chapters.len();
        self.chapters.push(LoadedChapter {
            first: start,
            len: loaded.chapter.len,
            anchors: loaded.chapter.anchors,
        });
        let layout = loaded.chapter.layout;
        let blocks = loaded.chapter.blocks.into_iter().enumerate();
        self.items
            .extend(blocks.map(|(i, block)| Item::new(block, spine, i == 0, layout)));
        for (href, picture) in loaded.pictures {
            self.pictures.entry(href).or_insert(picture);
        }
        self.list.splice(start..start, self.items.len() - start);
        if let Some(target) = self.pending.take_if(|t| t.spine < self.chapters.len()) {
            let position = self.target_position(&target);
            self.scroll_to_position(position);
            self.decode_near(position, cx);
            if self.loading() {
                self.wait_for_images(position, cx);
            }
        }
        self.show_when_ready(cx);
        cx.notify();
    }

    /// The book is not ready to show yet.
    pub fn loading(&self) -> bool {
        matches!(self.status, Status::Loading)
    }

    fn wait_for_images(&mut self, position: Position, cx: &mut Context<Self>) {
        self.waiting = self
            .images_near(position)
            .into_iter()
            .filter(|href| {
                self.pictures
                    .get(href)
                    .is_some_and(|picture| matches!(picture.decode, Decode::Running))
            })
            .collect();
        if !self.waiting.is_empty() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(IMAGE_WAIT).await;
                this.update(cx, |reader, cx| {
                    reader.waiting.clear();
                    reader.show_when_ready(cx);
                })
                .ok();
            })
            .detach();
        }
    }

    /// Shows the opened book when the text at its position and the images
    /// there are ready.
    fn show_when_ready(&mut self, cx: &mut Context<Self>) {
        if self.loading()
            && self.pending.is_none()
            && !self.items.is_empty()
            && self.waiting.is_empty()
        {
            self.status = Status::Ready;
            if let Some(id) = self.book_id.clone() {
                cx.update_global(|store: &mut Store, _| {
                    store.library.mark_opened(&id, Utc::now());
                    store.save();
                });
            }
            cx.notify();
        }
    }

    fn loaded(&mut self, cx: &mut Context<Self>) {
        // The target chapter is not in the book.
        self.pending = None;
        if self.items.is_empty() {
            self.fail("The book has no text.".into(), cx);
        } else {
            self.show_when_ready(cx);
        }
    }

    /// Goes to the target now, or when its chapter is loaded.
    fn go_to(&mut self, target: Target, cx: &mut Context<Self>) {
        if target.spine < self.chapters.len() {
            self.scroll_to_target(&target);
        } else if target.spine < self.spine_len {
            self.pending = Some(target);
        }
        self.scrolled(cx);
        cx.notify();
    }

    fn scroll_to_target(&mut self, target: &Target) {
        self.scroll_to_position(self.target_position(target));
    }

    fn target_position(&self, target: &Target) -> Position {
        let anchors = &self.chapters[target.spine].anchors;
        let offset = target
            .anchor
            .as_deref()
            .and_then(|anchor| anchors.get(anchor))
            .copied()
            .unwrap_or(target.offset);
        Position {
            spine: target.spine,
            offset,
        }
    }

    /// The item that holds a position. An empty chapter gives the start of
    /// the next one.
    fn item_at(&self, position: Position) -> Option<usize> {
        let first = self.chapters[position.spine].first;
        let end = self.chapter_end(position.spine);
        (first..end)
            .rev()
            .find(|&ix| self.items[ix].block.offset <= position.offset)
            .or((first < self.items.len()).then_some(first))
    }

    /// Starts to decode the images at a position before a render asks for them.
    fn decode_near(&mut self, position: Position, cx: &mut Context<Self>) {
        for href in self.images_near(position) {
            self.decode(&href, cx);
        }
    }

    fn images_near(&self, position: Position) -> Vec<String> {
        let Some(start) = self.item_at(position) else {
            return Vec::new();
        };
        let end = self.chapter_end(position.spine).min(start + DECODE_AHEAD);
        self.items[start..end]
            .iter()
            .filter_map(|item| match &item.block.kind {
                BlockKind::Image { href, .. } => Some(href.clone()),
                _ => None,
            })
            .collect()
    }

    fn scroll_to_position(&mut self, position: Position) {
        if self.paged() {
            self.show_position(position);
            return;
        }
        let Some(ix) = self.item_at(position) else {
            return;
        };
        let item = &self.items[ix];
        let into = position.offset.saturating_sub(item.block.offset);
        let part = if item.len == 0 {
            0.
        } else {
            (into as f32 / item.len as f32).min(1.)
        };
        self.scroll_to_item(ix);
        self.adjust = (part > 0.).then_some(Adjust::Part {
            ix,
            part,
            margin: 0.,
        });
    }

    fn scroll_to_item(&mut self, ix: usize) {
        self.adjust = None;
        self.list.scroll_to(ListOffset {
            item_ix: ix,
            offset_in_item: px(0.),
        });
    }

    /// The item after the last item of a loaded chapter.
    fn chapter_end(&self, spine: usize) -> usize {
        self.chapters
            .get(spine + 1)
            .map_or(self.items.len(), |chapter| chapter.first)
    }

    fn follow(&mut self, href: &str, cx: &mut Context<Self>) {
        let Some((spine, fragment)) = self.book.as_ref().and_then(|book| book.locate(href)) else {
            return;
        };
        let anchor = fragment.map(str::to_string);
        if let Some(position) = self.position() {
            self.back.push(position);
        }
        self.go_to(
            Target {
                spine,
                anchor,
                offset: 0,
            },
            cx,
        );
    }

    fn go_back(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(position) = self.back.pop() {
            self.go_to(position.into(), cx);
        }
    }

    /// Where an entry of the table of contents goes, once its chapter is loaded.
    fn toc_target(&self, ix: usize) -> Option<Target> {
        let (spine, anchor) = self.toc.get(ix)?.clone()?;
        Some(Target {
            spine,
            anchor,
            offset: 0,
        })
    }

    fn current_toc(&self) -> Option<usize> {
        self.toc_at(self.position()?)
    }

    /// The last entry of the table of contents at or before the position.
    fn toc_at(&self, position: Position) -> Option<usize> {
        let mut current = None;
        for (ix, entry) in self.toc.iter().enumerate() {
            let Some((spine, anchor)) = entry else {
                continue;
            };
            let Some(chapter) = self.chapters.get(*spine) else {
                continue;
            };
            let offset = anchor
                .as_ref()
                .and_then(|anchor| chapter.anchors.get(anchor).copied())
                .unwrap_or(0);
            let start = Position {
                spine: *spine,
                offset,
            };
            if start <= position && current.is_none_or(|(_, best)| start >= best) {
                current = Some((ix, start));
            }
        }
        current.map(|(ix, _)| ix)
    }

    fn open_toc_entry(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(target) = self.toc_target(ix) {
            self.toc_open = false;
            self.go_to(target, cx);
        }
    }

    fn toggle_toc(&mut self, _: &ToggleToc, _: &mut Window, cx: &mut Context<Self>) {
        self.toc_open = !self.toc_open;
        if self.toc_open
            && let Some(ix) = self.current_toc()
        {
            self.toc_scroll.scroll_to_item(ix, ScrollStrategy::Center);
        }
        cx.notify();
    }

    fn back(&mut self, _: &Back, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.open {
            self.close_search(window, cx);
        } else if self.toc_open {
            self.toc_open = false;
            cx.notify();
        } else {
            cx.emit(ReaderEvent::Close);
        }
    }

    fn scroll_by(&mut self, distance: f32, cx: &mut Context<Self>) {
        self.adjust = None;
        self.list.scroll_by(px(distance));
        self.scrolled(cx);
        cx.notify();
    }

    /// One screen, less one line, so the last line stays in view.
    fn screen(&self) -> f32 {
        let height = f32::from(self.list.viewport_bounds().size.height);
        let line = self.line_height();
        (height - line).max(line)
    }

    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.paged() {
            self.turn(1, cx);
        } else {
            self.scroll_by(self.screen(), cx);
        }
    }

    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.paged() {
            self.turn(-1, cx);
        } else {
            self.scroll_by(-self.screen(), cx);
        }
    }

    fn line_down(&mut self, _: &LineDown, _: &mut Window, cx: &mut Context<Self>) {
        if !self.paged() {
            self.scroll_by(self.line_height(), cx);
        }
    }

    fn line_up(&mut self, _: &LineUp, _: &mut Window, cx: &mut Context<Self>) {
        if !self.paged() {
            self.scroll_by(-self.line_height(), cx);
        }
    }

    fn chapter_start(&mut self, _: &ChapterStart, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(position) = self.position() {
            if self.paged() {
                self.show_position(Position {
                    spine: position.spine,
                    offset: 0,
                });
            } else {
                self.scroll_to_item(self.chapters[position.spine].first);
            }
            self.scrolled(cx);
            cx.notify();
        }
    }

    /// Puts the end of the chapter at the bottom of the window.
    fn chapter_end_key(&mut self, _: &ChapterEnd, _: &mut Window, cx: &mut Context<Self>) {
        let Some(position) = self.position() else {
            return;
        };
        let end = self.chapter_end(position.spine);
        if self.paged() {
            self.show_end(position.spine);
        } else if end == self.items.len() {
            self.adjust = None;
            self.list.scroll_to_end();
        } else {
            self.scroll_to_item(end);
            self.adjust = Some(Adjust::Above { laid_out: false });
        }
        self.scrolled(cx);
        cx.notify();
    }

    fn book_start(&mut self, _: &BookStart, _: &mut Window, cx: &mut Context<Self>) {
        if self.paged() {
            self.show_position(Position::default());
        } else {
            self.scroll_to_item(0);
        }
        self.scrolled(cx);
        cx.notify();
    }

    fn book_end(&mut self, _: &BookEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.show_book_end(cx);
    }

    fn show_book_end(&mut self, cx: &mut Context<Self>) {
        if self.paged() {
            if let Some(last) = self.chapters.len().checked_sub(1) {
                self.show_end(last);
            }
        } else {
            self.adjust = None;
            self.list.scroll_to_end();
        }
        self.scrolled(cx);
        cx.notify();
    }

    fn seek_to(&mut self, percent: f32, cx: &mut Context<Self>) {
        if self.chapters.len() < self.spine_len {
            return;
        }
        let total: usize = self.chapters.iter().map(|c| c.len).sum();
        let mut rest = (percent / 100. * total as f32) as usize;
        for (spine, chapter) in self.chapters.iter().enumerate() {
            if rest < chapter.len {
                self.go_to(
                    Position {
                        spine,
                        offset: rest,
                    }
                    .into(),
                    cx,
                );
                return;
            }
            rest -= chapter.len;
        }
        self.show_book_end(cx);
    }

    /// Characters before the position, as a percent of the book. Known when all
    /// chapters are loaded.
    fn percent(&self, position: Position) -> Option<f32> {
        if self.chapters.len() < self.spine_len {
            return None;
        }
        let total: usize = self.chapters.iter().map(|c| c.len).sum();
        let before: usize = self.chapters[..position.spine].iter().map(|c| c.len).sum();
        Some((100. * (before + position.offset) as f32 / total.max(1) as f32).min(100.))
    }

    /// Hides the top bar while you read, and saves the position later.
    fn scrolled(&mut self, cx: &mut Context<Self>) {
        self.top_bar = false;
        self.redraw = true;
        cx.notify();
        self.schedule_save(cx);
    }

    fn mouse_moved(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.hide_bars || self.dragging() {
            return;
        }
        let y = f32::from(event.position.y) - title_bar::height(window);
        let height = content_height(window);
        let bars = (y <= BAR_TRIGGER, y >= height - BAR_TRIGGER);
        if bars != (self.top_bar, self.bottom_bar) {
            (self.top_bar, self.bottom_bar) = bars;
            cx.notify();
        }
    }

    /// The top block, and how far it is scrolled, as a part of its characters.
    fn position(&self) -> Option<Position> {
        if self.paged() {
            let at = self.pages.at;
            let chapter = self
                .chapters
                .get(at.spine)
                .filter(|_| !self.items.is_empty())?;
            return Some(Position {
                spine: at.spine,
                offset: at.offset.min(chapter.len),
            });
        }
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
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            this.update(cx, |reader, cx| reader.save_position(cx)).ok();
        }));
    }

    fn save_position(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        if self.pending.is_some() || self.adjust.is_some() || self.pages.moving() {
            return;
        }
        let (Some(id), Some(position)) = (self.book_id.as_deref(), self.position()) else {
            return;
        };
        let percent = self.percent(position);
        cx.update_global(|store: &mut Store, _| {
            let Some(entry) = store.library.book_mut(id) else {
                return;
            };
            if entry.position == position {
                return;
            }
            entry.position = position;
            if let Some(percent) = percent {
                entry.percent = percent;
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
        let tones = self.tones;
        cx.spawn(async move |this, cx| {
            let image = cx
                .background_spawn(async move { decode_image(&bytes, tones) })
                .await;
            this.update(cx, |reader, cx| reader.decoded(href, image, tones, cx))
                .ok();
        })
        .detach();
    }

    fn decoded(
        &mut self,
        href: String,
        image: Option<Arc<RenderImage>>,
        tones: Option<Tones>,
        cx: &mut Context<Self>,
    ) {
        self.waiting.retain(|waiting| *waiting != href);
        self.show_when_ready(cx);
        let Some(picture) = self.pictures.get_mut(&href) else {
            return;
        };
        if tones != self.tones {
            // The tones changed during the decode.
            picture.decode = Decode::No;
            self.dropped.extend(image);
            cx.notify();
            return;
        }
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

    /// The text stays clear of the search panel.
    fn panel_width(&self) -> f32 {
        if self.search.open {
            search::PANEL_WIDTH
        } else {
            0.
        }
    }

    fn spacing(&self, ix: usize) -> Spacing {
        let size = self.font_size;
        let item = &self.items[ix];
        let gap = size * GAP * item.block.gap.min(MAX_GAP) as f32;
        let paragraph_bottom = match item.layout {
            Layout::Verse => 0.,
            _ => size * 0.75,
        };
        let (top, bottom, line) = match &item.block.kind {
            // A heading does not break across pages.
            BlockKind::Heading { .. } => (size * 0.8 + gap, size * 0.6, None),
            BlockKind::Paragraph { .. } => (gap, paragraph_bottom, Some(size * LINE_SPACING)),
            BlockKind::ListItem { .. } => (0., size * 0.4, Some(size * LINE_SPACING)),
            BlockKind::Pre { .. } => (0., size * 0.75, Some(size * PRE_SCALE * LINE_SPACING)),
            BlockKind::Image { .. } | BlockKind::Error { .. } => (0., size * 0.75, None),
            BlockKind::Rule => (size, size, None),
        };
        Spacing { top, bottom, line }
    }

    /// Returns the element and the href of an image that must be decoded.
    fn render_item(
        &self,
        ix: usize,
        view: &Entity<Reader>,
        window: &Window,
        cx: &App,
    ) -> (AnyElement, Option<String>) {
        let item = &self.items[ix];
        let viewport = window.viewport_size();
        let panel = self.panel_width();
        let width = f32::from(viewport.width) - panel;
        let column = (width - 2. * MARGIN).clamp(100., COLUMN_WIDTH);
        let max_height = content_height(window) * 0.9;
        let (block, decode) = self.render_block(ix, Some(view), column, max_height, cx);

        let mut row = div().w_full().flex().justify_center().pr(px(panel));
        if ix == 0 {
            row = row.pt(px(TEXT_TOP));
        } else if item.chapter_start {
            row = row.pt(px(self.font_size * 3.));
        }
        if ix + 1 == self.items.len() {
            row = row.pb(px(MARGIN * 2.));
        }
        (row.child(block).into_any_element(), decode)
    }

    /// One block at the width of the column. Without a view, the text has no
    /// mouse handlers, for a measurement. Returns the element and the href of
    /// an image that must be decoded.
    fn render_block(
        &self,
        ix: usize,
        view: Option<&Entity<Reader>>,
        column: f32,
        max_height: f32,
        cx: &App,
    ) -> (AnyElement, Option<String>) {
        let item = &self.items[ix];
        let theme = cx.theme();
        let font_size = self.font_size;
        let spacing = self.spacing(ix);
        let text = || match view {
            Some(view) => self.text_element(ix, view, cx),
            None => item
                .styled_text(theme, std::iter::empty())
                .into_any_element(),
        };
        let mut decode = None;

        let mut content = match &item.block.kind {
            BlockKind::Heading { level, .. } => {
                let scale = HEADING_SCALE[(*level as usize).clamp(1, 6) - 1];
                div()
                    .pt(px(spacing.top))
                    .pb(px(spacing.bottom))
                    .text_size(px(font_size * scale))
                    .line_height(relative(HEADING_SPACING))
                    .font_weight(FontWeight::BOLD)
                    .when(item.layout == Layout::Title, |d| d.text_center())
                    .child(text())
            }
            BlockKind::Paragraph { .. } => div()
                .pt(px(spacing.top))
                .pb(px(spacing.bottom))
                .when(item.block.inset, |d| d.pl(px(font_size * INSET)))
                .child(text()),
            BlockKind::ListItem { depth, marker, .. } => {
                let marker = marker.as_ref().map(marker_label).unwrap_or_default();
                div()
                    .flex()
                    .pb(px(spacing.bottom))
                    .pl(px(font_size * 1.5 * (*depth as f32 - 1.)))
                    .child(div().flex_none().w(px(font_size * 1.8)).child(marker))
                    .child(div().flex_1().min_w_0().child(text()))
            }
            BlockKind::Pre { .. } => div()
                .pb(px(spacing.bottom))
                .font_family(MONO)
                .text_size(px(font_size * PRE_SCALE))
                .child(text()),
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
                    .pb(px(spacing.bottom))
                    .child(frame)
            }
            BlockKind::Rule => div()
                .flex()
                .justify_center()
                .pt(px(spacing.top))
                .pb(px(spacing.bottom))
                .child(div().w(px(column * 0.3)).h(px(1.)).bg(theme.border)),
            BlockKind::Error { message } => div()
                .pb(px(spacing.bottom))
                .italic()
                .text_color(theme.danger)
                .child(message.clone()),
        };

        if item.block.quote > 0 {
            content = div()
                .ml(px(font_size * (item.block.quote as f32 - 1.)))
                .mb(px(font_size * QUOTE_GAP))
                .pl(px(font_size))
                .border_l_1()
                .border_color(theme.border)
                .child(content);
        }

        // A measurement has no parent to give it the text style.
        let element = div()
            .w(px(column))
            .font_family(self.font_family())
            .text_size(px(font_size))
            .line_height(relative(LINE_SPACING))
            .child(content)
            .into_any_element();
        (element, decode)
    }

    pub(super) fn font_family(&self) -> &'static str {
        match self.font {
            Font::Serif => SERIF,
            Font::Sans => SANS,
            Font::Newsreader => "Newsreader 16pt",
            Font::SourceSerif => "Source Serif 4",
            Font::Alegreya => "Alegreya",
            Font::Garamond => "EB Garamond",
            Font::Mono => "IBM Plex Mono",
        }
    }
}

/// The height of the window under the title bar.
fn content_height(window: &Window) -> f32 {
    f32::from(window.viewport_size().height) - title_bar::height(window)
}

fn marker_label(marker: &Marker) -> String {
    match marker {
        Marker::Bullet => "•".to_string(),
        Marker::Number(n) => format!("{n}."),
    }
}

/// Space above and below the content of a block, and the line height of its
/// text, if a page can break between its lines.
struct Spacing {
    top: f32,
    bottom: f32,
    line: Option<f32>,
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

type Tones = ([u8; 3], [u8; 3]);

fn image_tones(cx: &App) -> Option<Tones> {
    let dither = cx.global::<Store>().library.settings.dither_images;
    dither.then(|| style::tones(cx))
}

fn decode_image(bytes: &[u8], tones: Option<Tones>) -> Option<Arc<RenderImage>> {
    let image = image::load_from_memory(bytes).ok()?;
    let (width, height) = (image.width(), image.height());
    let mut pixels = match tones {
        Some((dark, light)) => {
            let scale = (COLUMN_WIDTH / width as f32).min(1.) / DITHER_DOT;
            let dots = shrink(image, scale).into_rgba8();
            dither_small(
                &dots,
                style::DITHER_CELL,
                &dither::Adjust::NONE,
                dark,
                light,
            )
        }
        None => {
            let scale = (IMAGE_MAX as f32 / width.max(height) as f32).min(1.);
            shrink(image, scale).into_rgba8()
        }
    };
    // RenderImage is BGRA.
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![Frame::new(pixels)])))
}

fn shrink(image: DynamicImage, scale: f32) -> DynamicImage {
    if scale >= 1. {
        return image;
    }
    let size = |side: u32| ((side as f32 * scale).round() as u32).max(1);
    let (width, height) = (size(image.width()), size(image.height()));
    let mut small = DynamicImage::new(width, height, image.color());
    match Resizer::new().resize(&image, &mut small, None) {
        Ok(()) => small,
        // A pixel format that the resizer does not know.
        Err(_) => image.thumbnail_exact(width, height),
    }
}

impl Reader {
    fn render_top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let library = &cx.global::<Store>().library;
        let edited = self.book_id.as_deref().and_then(|id| library.book(id));
        let title = match (edited, &self.book) {
            (Some(entry), _) => Some(entry.shown_title().to_uppercase()),
            (None, book) => book.as_ref().map(|book| book.title.to_uppercase()),
        };
        let chapter = self
            .current_toc()
            .and_then(|ix| Some(self.book.as_ref()?.toc[ix].label.to_uppercase()));
        div()
            .flex_none()
            .h(px(TOP_BAR_HEIGHT))
            .px(px(MARGIN))
            .flex()
            .items_center()
            .gap_3()
            .text_size(px(11.))
            .bg(theme.background)
            .border_b_1()
            .border_color(theme.border)
            .occlude()
            .child(
                style::button("close-book", "ESC", false, cx)
                    .h(px(20.))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ReaderEvent::Close))),
            )
            .children(title.map(|title| {
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .child(title)
            }))
            .children(chapter.map(|chapter| {
                div()
                    .flex()
                    .min_w_0()
                    .gap_3()
                    .text_color(theme.muted_foreground)
                    .child("//")
                    .child(div().min_w_0().truncate().child(chapter))
            }))
            .child(div().flex_1())
            .child(
                style::button("settings", "SETTINGS", false, cx)
                    .h(px(20.))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx)),
            )
    }

    fn render_bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let position = self.position();
        let percent = position.and_then(|position| self.percent(position));
        let pages = position.and_then(|position| self.page_count(position));
        div()
            .flex_none()
            .h(px(BOTTOM_BAR_HEIGHT))
            .px(px(MARGIN))
            .flex()
            .items_center()
            .gap_4()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .child(self.render_seek(percent, cx))
            .child(
                style::dot(
                    percent.map_or(SharedString::default(), |p| format!("{p:.0}%").into()),
                    16.,
                )
                .w(px(44.))
                .text_right(),
            )
            .children(pages.map(|(page, count)| style::dot(format!("{page} / {count}"), 16.)))
    }

    /// A row of dots, filled up to the percent, with a marker at the percent.
    /// Without a percent, it is empty and does nothing.
    fn render_seek(&self, percent: Option<f32>, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (ink, paper, accent, faint) = (
            theme.foreground,
            theme.background,
            theme.success,
            style::faint(cx),
        );
        let view = cx.entity();
        let paint = move |bounds: Bounds<Pixels>, _, window: &mut Window, _: &mut App| {
            let mid = bounds.center().y;
            let at = bounds.left() + bounds.size.width * percent.unwrap_or(0.) / 100.;
            let mut x = bounds.left() + px(1.5);
            while x < bounds.right() {
                let on = percent.is_some() && x <= at;
                let side = px(if on { 3. } else { 2. });
                let dot = Bounds::new(point(x - side / 2., mid - side / 2.), size(side, side));
                window.paint_quad(fill(dot, if on { ink } else { faint }));
                x += px(SEEK_STEP);
            }
            if percent.is_none() {
                return;
            }
            let line = Bounds::new(point(at, mid - px(5.)), size(px(1.), px(14.)));
            window.paint_quad(fill(line, accent));
            let head = Bounds::new(point(at - px(3.), mid - px(10.)), size(px(7.), px(7.)));
            window.paint_quad(fill(head, paper));
            window.paint_quad(outline(head, accent, BorderStyle::Solid));

            let to_percent =
                move |x: Pixels| ((x - bounds.left()) / bounds.size.width * 100.).clamp(0., 100.);
            window.on_mouse_event({
                let view = view.clone();
                move |event: &MouseDownEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && bounds.contains(&event.position)
                    {
                        view.update(cx, |reader, cx| {
                            reader.seeking = true;
                            reader.seek_to(to_percent(event.position.x), cx);
                        });
                    }
                }
            });
            window.on_mouse_event({
                let view = view.clone();
                move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble && event.dragging() && view.read(cx).seeking {
                        view.update(cx, |reader, cx| {
                            reader.seek_to(to_percent(event.position.x), cx)
                        });
                    }
                }
            });
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    view.update(cx, |reader, _| reader.seeking = false);
                }
            });
        };
        div()
            .flex_1()
            .h(px(22.))
            .when(percent.is_some(), |seek| seek.cursor_pointer())
            .child(canvas(|_, _, _| (), paint).size_full())
    }

    fn render_toc(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let body = if self.toc.is_empty() {
            style::label("THIS BOOK HAS NO TABLE OF CONTENTS.", cx)
                .px(px(18.))
                .into_any_element()
        } else {
            let current = self.current_toc();
            uniform_list(
                "toc",
                self.toc.len(),
                cx.processor(move |reader, range: Range<usize>, _, cx| {
                    let Some(book) = reader.book.clone() else {
                        return Vec::new();
                    };
                    let theme = cx.theme();
                    let (ink, paper, hover) =
                        (theme.foreground, theme.background, theme.list_hover);
                    let faint = style::faint(cx);
                    range
                        .map(|ix| {
                            let entry = &book.toc[ix];
                            let is_current = Some(ix) == current;
                            div()
                                .id(ix)
                                .w_full()
                                .h(px(TOC_ROW_HEIGHT))
                                .pl(px(18. + 16. * entry.depth as f32))
                                .pr(px(18.))
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .text_size(px(12.))
                                .child(style::dot(format!("{:02}", ix + 1), 13.).w(px(18.)))
                                .child(div().min_w_0().truncate().child(entry.label.clone()))
                                .when(is_current, |row| row.bg(ink).text_color(paper))
                                .map(|row| {
                                    if reader.toc[ix].is_some() {
                                        row.cursor_pointer()
                                            .when(!is_current, |row| {
                                                row.hover(move |row| row.bg(hover))
                                            })
                                            .on_click(cx.listener(move |reader, _, _, cx| {
                                                reader.open_toc_entry(ix, cx)
                                            }))
                                    } else {
                                        row.text_color(faint)
                                    }
                                })
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.toc_scroll)
            .flex_1()
            .into_any_element()
        };
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(px(TOC_WIDTH))
            .flex()
            .flex_col()
            .py(px(18.))
            .bg(theme.background)
            .border_r_1()
            .border_color(theme.border)
            .occlude()
            .child(style::side_shadow(true, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(18.))
                    .pb(px(12.))
                    .child(style::tag("CONTENTS", cx))
                    .child(style::rule(cx))
                    .child(style::tag("T", cx)),
            )
            .child(body)
    }
}

impl Render for Reader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.painted.borrow_mut().clear();
        let tones = image_tones(cx);
        if tones != self.tones {
            self.tones = tones;
            self.decoded.clear();
            for picture in self.pictures.values_mut() {
                if let Decode::Done(image) = mem::replace(&mut picture.decode, Decode::No) {
                    self.dropped.push(image);
                }
            }
        }
        for image in self.dropped.drain(..) {
            window.drop_image(image).ok();
        }
        match self.adjust {
            Some(Adjust::Part { ix, part, margin }) => match self.list.bounds_for_item(ix) {
                Some(bounds) => {
                    self.list.scroll_to(ListOffset {
                        item_ix: ix,
                        offset_in_item: (bounds.size.height * part - px(margin)).max(px(0.)),
                    });
                    self.adjust = None;
                }
                None => window.request_animation_frame(),
            },
            Some(Adjust::Above { laid_out: false }) => {
                self.adjust = Some(Adjust::Above { laid_out: true });
                window.request_animation_frame();
            }
            Some(Adjust::Above { laid_out: true }) => {
                let height = self.list.viewport_bounds().size.height;
                self.list.scroll_by(-height);
                self.adjust = None;
            }
            None => {}
        }
        if mem::take(&mut self.redraw) {
            window.request_animation_frame();
        }
        let ready = matches!(self.status, Status::Ready);
        if ready {
            self.estimate_pages(window);
        }
        let pages = if ready && self.paged() {
            self.update_pages(window, cx).map(|geometry| {
                let (element, decode) = self.render_pages(geometry, cx);
                for href in decode {
                    self.decode(&href, cx);
                }
                element
            })
        } else {
            None
        };
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
            Status::Ready if self.paged() => pages.unwrap_or_else(|| div().into_any_element()),
            Status::Ready => {
                let view = cx.entity();
                list(self.list.clone(), move |ix, window, cx| {
                    let (element, decode) = view.read(cx).render_item(ix, &view, window, cx);
                    if let Some(href) = decode {
                        view.update(cx, |reader, cx| reader.decode(&href, cx));
                    }
                    element
                })
                .size_full()
                .into_any_element()
            }
        };
        let (background, foreground) = (theme.background, theme.foreground);
        let (chrome_font, chrome_size) = (theme.font_family.clone(), theme.font_size);
        let chrome = || {
            div()
                .font_family(chrome_font.clone())
                .text_size(chrome_size)
                .line_height(relative(1.4))
        };
        let mut top_bar = ready.then(|| chrome().child(self.render_top_bar(cx)));
        let mut bottom_bar = ready.then(|| chrome().child(self.render_bottom_bar(cx)));
        let mut float_top = None;
        if self.hide_bars {
            // Hidden bars show above the text, at the edges of the window.
            float_top = top_bar
                .take()
                .filter(|_| self.top_bar)
                .map(|bar| bar.absolute().top_0().left_0().right_0());
            bottom_bar = bottom_bar
                .filter(|_| self.bottom_bar || self.seeking)
                .map(|bar| bar.absolute().bottom_0().left_0().right_0());
        }

        // The search box is outside the "Reader" key context, so that the keys
        // of the reader do not take the characters that you type.
        div()
            .id("reader-root")
            .on_mouse_move(cx.listener(Self::mouse_moved))
            .capture_any_mouse_down(cx.listener(|reader, _, _, cx| reader.clear_selection(cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|reader, _, _, cx| reader.end_drag(cx)),
            )
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(background)
            .text_color(foreground)
            .children(top_bar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("reader")
                            .key_context("Reader")
                            .track_focus(&self.focus)
                            .on_action(cx.listener(Self::back))
                            .on_action(cx.listener(Self::toggle_toc))
                            .on_action(cx.listener(Self::go_back))
                            .on_action(cx.listener(Self::open_search))
                            .on_action(cx.listener(Self::copy))
                            .on_action(cx.listener(Self::next_match))
                            .on_action(cx.listener(Self::previous_match))
                            .on_action(cx.listener(Self::page_down))
                            .on_action(cx.listener(Self::page_up))
                            .on_action(cx.listener(Self::line_down))
                            .on_action(cx.listener(Self::line_up))
                            .on_action(cx.listener(Self::chapter_start))
                            .on_action(cx.listener(Self::chapter_end_key))
                            .on_action(cx.listener(Self::book_start))
                            .on_action(cx.listener(Self::book_end))
                            .relative()
                            .size_full()
                            .font_family(self.font_family())
                            .text_size(px(self.font_size))
                            .line_height(relative(LINE_SPACING))
                            .child(body)
                            .when(ready && self.toc_open, |main| {
                                main.child(
                                    chrome()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .size_full()
                                        .child(self.render_toc(cx)),
                                )
                            }),
                    )
                    .when(ready && self.search.open, |main| {
                        main.child(self.render_search(cx))
                    }),
            )
            .children(bottom_bar)
            .children(float_top)
            .children(self.drag_listener(cx))
    }
}

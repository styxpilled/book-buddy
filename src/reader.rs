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
    AnyElement, App, AppContext, BorrowAppContext, Context, Entity, EventEmitter, FocusHandle,
    Focusable, FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveElement, InteractiveText,
    IntoElement, KeyBinding, ListAlignment, ListOffset, ListState, MouseMoveEvent, ParentElement,
    Render, RenderImage, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled,
    StyledText, Subscription, Task, UnderlineStyle, UniformListScrollHandle, Window, actions,
    combine_highlights, div, img, list, prelude::FluentBuilder, px, relative, uniform_list,
};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme, Theme};
use image::imageops::FilterType;
use image::{Frame, ImageReader};

mod search;

use crate::Back;
use crate::scan;
use crate::store::Store;
use search::Search;

const SERIF: &str = "Literata";
const MONO: &str = "Consolas";
const FONT_SIZE: f32 = 18.;
const LINE_HEIGHT: f32 = FONT_SIZE * 1.5;
const HEADING_SCALE: [f32; 6] = [1.8, 1.5, 1.3, 1.15, 1.0, 1.0];
const COLUMN_WIDTH: f32 = 720.;
const MARGIN: f32 = 32.;
/// GPUI keeps a decoded image until it is dropped, so only this many stay decoded.
const DECODED_IMAGES: usize = 16;
/// Longest side of a decoded image, in pixels.
const IMAGE_MAX: u32 = 2048;
const SAVE_DELAY: Duration = Duration::from_secs(1);
const TOP_BAR_HEIGHT: f32 = 40.;
/// The top bar shows when the pointer is this close to the top of the window.
const TOP_EDGE: f32 = 12.;
const BOTTOM_BAR_HEIGHT: f32 = 36.;
const TOC_WIDTH: f32 = 320.;
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
        KeyBinding::new("escape", Back, Some("Search")),
        KeyBinding::new("f3", NextMatch, Some("Search")),
        KeyBinding::new("shift-f3", PreviousMatch, Some("Search")),
    ]);

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
        }
    }

    /// `mark` is a search match to highlight.
    fn styled_text(&self, theme: &Theme, mark: Option<Range<usize>>) -> StyledText {
        let link = theme.link;
        let styles = self
            .styles
            .iter()
            .map(move |(range, run)| (range.clone(), highlight(run, link)));
        let mark = mark.map(|range| {
            let style = HighlightStyle {
                background_color: Some(theme.selection),
                ..Default::default()
            };
            (range, style)
        });
        StyledText::new(self.text.clone()).with_highlights(combine_highlights(styles, mark))
    }

    /// The text, with a click handler on each link.
    fn text(
        &self,
        ix: usize,
        theme: &Theme,
        view: Entity<Reader>,
        mark: Option<Range<usize>>,
    ) -> AnyElement {
        let (ranges, hrefs): (Vec<_>, Vec<_>) = self
            .styles
            .iter()
            .filter_map(|(range, run)| Some((range.clone(), run.link.clone()?)))
            .unzip();
        if ranges.is_empty() {
            return self.styled_text(theme, mark).into_any_element();
        }
        InteractiveText::new(("text", ix), self.styled_text(theme, mark))
            .on_click(ranges, move |link, _, cx| {
                view.update(cx, |reader, cx| reader.follow(&hrefs[link], cx));
            })
            .into_any_element()
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
    decoded: VecDeque<String>,
    dropped: Vec<Arc<RenderImage>>,
    toc_open: bool,
    search: Search,
    toc_scroll: UniformListScrollHandle,
    top_bar: bool,
    /// The list corrects a scroll past the end in its layout, after the render.
    /// One more frame shows the corrected position in the bars.
    redraw: bool,
    seek: Entity<SliderState>,
    /// The seek bar is dragged, so it does not follow the position.
    seeking: bool,
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
        let seek = cx.new(|_| SliderState::new().min(0.).max(100.).step(0.01));
        let subscriptions = vec![
            cx.on_app_quit(|reader, cx| {
                reader.save_position(cx);
                async {}
            }),
            cx.subscribe(&seek, |reader, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    reader.seeking = true;
                    reader.seek_to(value.end(), cx);
                }
                SliderEvent::Release(_) => reader.seeking = false,
            }),
        ];
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
            decoded: VecDeque::new(),
            dropped: Vec::new(),
            toc_open: false,
            search: Search::default(),
            toc_scroll: UniformListScrollHandle::new(),
            top_bar: false,
            redraw: false,
            seek,
            seeking: false,
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
            let position = store.library.open(opened.info, Utc::now()).position;
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
        let blocks = loaded.chapter.blocks.into_iter().enumerate();
        self.items
            .extend(blocks.map(|(i, block)| Item::new(block, spine, i == 0)));
        for (href, picture) in loaded.pictures {
            self.pictures.entry(href).or_insert(picture);
        }
        self.list.splice(start..start, self.items.len() - start);
        if let Some(target) = self.pending.take_if(|t| t.spine < self.chapters.len()) {
            self.scroll_to_target(&target);
        }
        if self.pending.is_none() && !self.items.is_empty() {
            self.status = Status::Ready;
        }
        cx.notify();
    }

    fn loaded(&mut self, cx: &mut Context<Self>) {
        // The target chapter is not in the book.
        self.pending = None;
        if self.items.is_empty() {
            self.fail("The book has no text.".into(), cx);
        } else {
            self.status = Status::Ready;
            cx.notify();
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
        let anchors = &self.chapters[target.spine].anchors;
        let offset = target
            .anchor
            .as_deref()
            .and_then(|anchor| anchors.get(anchor))
            .copied()
            .unwrap_or(target.offset);
        self.scroll_to_position(Position {
            spine: target.spine,
            offset,
        });
    }

    fn scroll_to_position(&mut self, position: Position) {
        let first = self.chapters[position.spine].first;
        let end = self.chapter_end(position.spine);
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
        (height - LINE_HEIGHT).max(LINE_HEIGHT)
    }

    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(self.screen(), cx);
    }

    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(-self.screen(), cx);
    }

    fn line_down(&mut self, _: &LineDown, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(LINE_HEIGHT, cx);
    }

    fn line_up(&mut self, _: &LineUp, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_by(-LINE_HEIGHT, cx);
    }

    fn chapter_start(&mut self, _: &ChapterStart, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(position) = self.position() {
            self.scroll_to_item(self.chapters[position.spine].first);
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
        if end == self.items.len() {
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
        self.scroll_to_item(0);
        self.scrolled(cx);
        cx.notify();
    }

    fn book_end(&mut self, _: &BookEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust = None;
        self.list.scroll_to_end();
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
        self.adjust = None;
        self.list.scroll_to_end();
        self.scrolled(cx);
        cx.notify();
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

    fn mouse_moved(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let y = f32::from(event.position.y);
        let show = if y <= TOP_EDGE {
            true
        } else if y > TOP_BAR_HEIGHT {
            false
        } else {
            self.top_bar
        };
        if show != self.top_bar {
            self.top_bar = show;
            cx.notify();
        }
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
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            this.update(cx, |reader, cx| reader.save_position(cx)).ok();
        }));
    }

    fn save_position(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        if self.pending.is_some() || self.adjust.is_some() {
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
    fn render_item(
        &self,
        ix: usize,
        view: &Entity<Reader>,
        window: &Window,
        cx: &App,
    ) -> (AnyElement, Option<String>) {
        let item = &self.items[ix];
        let mark = self.search.marked(ix);
        let theme = cx.theme();
        let viewport = window.viewport_size();
        // The text stays clear of the search panel.
        let panel = if self.search.open {
            search::PANEL_WIDTH
        } else {
            0.
        };
        let width = f32::from(viewport.width) - panel;
        let column = (width - 2. * MARGIN).clamp(100., COLUMN_WIDTH);
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
                    .child(item.text(ix, theme, view.clone(), mark.clone()))
            }
            BlockKind::Paragraph { .. } => div().pb(px(FONT_SIZE * 0.75)).child(item.text(
                ix,
                theme,
                view.clone(),
                mark.clone(),
            )),
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
                    .child(div().flex_1().min_w_0().child(item.text(
                        ix,
                        theme,
                        view.clone(),
                        mark.clone(),
                    )))
            }
            BlockKind::Pre { .. } => div()
                .pb(px(FONT_SIZE * 0.75))
                .font_family(MONO)
                .text_size(px(FONT_SIZE * 0.85))
                .child(item.styled_text(theme, mark)),
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

        let mut row = div().w_full().flex().justify_center().pr(px(panel));
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

impl Reader {
    fn render_top_bar(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let title = self.book.as_ref().map(|book| book.title.clone());
        let chapter = self
            .current_toc()
            .and_then(|ix| Some(self.book.as_ref()?.toc[ix].label.clone()));
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(TOP_BAR_HEIGHT))
            .px(px(MARGIN))
            .flex()
            .items_center()
            .gap_3()
            .bg(theme.background)
            .border_b_1()
            .border_color(theme.border)
            .occlude()
            .children(title.map(|title| {
                div()
                    .flex_none()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title)
            }))
            .children(chapter.map(|chapter| {
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.muted_foreground)
                    .child(chapter)
            }))
    }

    fn render_bottom_bar(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let percent = self.position().and_then(|position| self.percent(position));
        div()
            .flex_none()
            .h(px(BOTTOM_BAR_HEIGHT))
            .px(px(MARGIN))
            .flex()
            .items_center()
            .gap_4()
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex_1()
                    .child(Slider::new(&self.seek).disabled(percent.is_none())),
            )
            .child(
                div()
                    .w(px(40.))
                    .text_right()
                    .text_color(theme.muted_foreground)
                    .children(percent.map(|percent| format!("{percent:.0}%"))),
            )
    }

    fn render_toc(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let body = if self.toc.is_empty() {
            div()
                .p(px(16.))
                .text_color(theme.muted_foreground)
                .child("This book has no table of contents.")
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
                    let (active, hover, muted) =
                        (theme.list_active, theme.list_hover, theme.muted_foreground);
                    range
                        .map(|ix| {
                            let entry = &book.toc[ix];
                            div()
                                .id(ix)
                                .w_full()
                                .h(px(TOC_ROW_HEIGHT))
                                .pl(px(16. + 16. * entry.depth as f32))
                                .pr(px(16.))
                                .flex()
                                .items_center()
                                .child(div().min_w_0().truncate().child(entry.label.clone()))
                                .when(Some(ix) == current, |row| row.bg(active))
                                .map(|row| {
                                    if reader.toc[ix].is_some() {
                                        row.cursor_pointer()
                                            .hover(move |row| row.bg(hover))
                                            .on_click(cx.listener(move |reader, _, _, cx| {
                                                reader.open_toc_entry(ix, cx)
                                            }))
                                    } else {
                                        row.text_color(muted)
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
            .py_2()
            .bg(theme.background)
            .border_r_1()
            .border_color(theme.border)
            .occlude()
            .child(body)
    }
}

impl Render for Reader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        if !self.seeking
            && let Some(percent) = self.position().and_then(|p| self.percent(p))
            && (self.seek.read(cx).value().end() - percent).abs() > 0.01
        {
            self.seek
                .update(cx, |seek, cx| seek.set_value(percent, window, cx));
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
        let ready = matches!(self.status, Status::Ready);
        let body = match &self.status {
            Status::NoBook => message("Open a book: book-buddy <file.epub>".into()),
            Status::Loading => message("Loading...".into()),
            Status::Failed(error) => message(error.clone().into()),
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

        // The search box is outside the "Reader" key context, so that the keys
        // of the reader do not take the characters that you type.
        div()
            .id("reader-root")
            .on_mouse_move(cx.listener(Self::mouse_moved))
            .size_full()
            .flex()
            .flex_col()
            .bg(background)
            .text_color(foreground)
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
                            .font_family(SERIF)
                            .text_size(px(FONT_SIZE))
                            .line_height(relative(1.5))
                            .child(body)
                            .when(ready && (self.toc_open || self.top_bar), |main| {
                                main.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .size_full()
                                        .font_family(chrome_font.clone())
                                        .text_size(chrome_size)
                                        .line_height(relative(1.4))
                                        .when(self.toc_open, |chrome| {
                                            chrome.child(self.render_toc(cx))
                                        })
                                        .when(self.top_bar, |chrome| {
                                            chrome.child(self.render_top_bar(cx))
                                        }),
                                )
                            }),
                    )
                    .when(ready && self.search.open, |main| {
                        main.child(self.render_search(cx))
                    }),
            )
            .when(ready, |reader| {
                reader.child(
                    div()
                        .font_family(chrome_font)
                        .text_size(chrome_size)
                        .child(self.render_bottom_bar(cx)),
                )
            })
    }
}

use std::collections::HashMap;
use std::time::{Duration, Instant};

use book_buddy::book::BlockKind;
use book_buddy::library::{Font, Mode, Position};
use gpui::{
    AnyElement, AvailableSpace, Context, Hsla, InteractiveElement, IntoElement, ParentElement,
    ScrollDelta, ScrollWheelEvent, Styled, TextRun, Window, div, font, px, size,
};
use gpui_kit::component::ActiveTheme;

use super::{
    COLUMN_WIDTH, HEADING_SCALE, HEADING_SPACING, LINE_SPACING, MARGIN, PRE_SCALE, Reader,
    TEXT_BOTTOM, TEXT_TOP, content_height,
};

/// The pages are made again only when the window size stays the same this long.
const RESIZE_DELAY: Duration = Duration::from_millis(150);
/// Wheel distance in pixels for one page turn.
const WHEEL_TURN: f32 = 60.;

/// A line of usual text, to measure the mean width of a character.
const SAMPLE: &str = "It is a truth universally acknowledged, that a single man in possession of a good fortune, must be in want of a wife.";
/// The part of a line that text fills, as lines wrap between words.
const WRAP: f32 = 0.9;

/// What the pages depend on.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Geometry {
    columns: usize,
    column: f32,
    height: f32,
    panel: f32,
    font: Font,
    font_size: f32,
    hide_bars: bool,
}

impl Geometry {
    /// Only the window size is different. Other changes apply at once.
    fn resized(&self, other: &Geometry) -> bool {
        (self.panel, self.font, self.font_size, self.hide_bars)
            == (other.panel, other.font, other.font_size, other.hide_bars)
    }
}

/// A part of a block: from `top` in the block, `height` pixels.
struct Slice {
    item: usize,
    top: f32,
    height: f32,
}

struct Page {
    /// Character offset in the chapter.
    start: usize,
    slices: Vec<Slice>,
}

struct Measure {
    height: f32,
    /// The end of the content, without the space below it.
    end: f32,
    /// The top of the first line and the line height, if the block can break
    /// between lines.
    lines: Option<(f32, f32)>,
}

#[derive(Default)]
pub(super) struct Pages {
    geometry: Option<Geometry>,
    chapters: HashMap<usize, Vec<Page>>,
    /// The position. The page shown is the one that holds it.
    pub(super) at: Position,
    /// The spine index and page index shown, while the pages stay valid.
    /// Pages can start at the same offset (an image has no text), so the
    /// offset alone does not tell which one is shown.
    page: Option<(usize, usize)>,
    /// Move `at` to the start of the page found for it.
    snap: bool,
    /// Page turns for the next render, in spreads. Negative turns go back.
    turns: isize,
    /// A new window size and when it was first seen.
    resized: Option<(Geometry, Instant)>,
    wheel: f32,
    estimate: Estimate,
}

/// The layout of the whole book on pages, estimated from the number of
/// characters in each block, without a measurement.
#[derive(Default)]
struct Estimate {
    /// The geometry and the number of items that it was made for.
    key: Option<(Geometry, usize)>,
    /// The top and the height of each item, from the start of the book.
    /// Each chapter starts on a new page.
    items: Vec<(f32, f32)>,
    pages: usize,
}

impl Pages {
    /// A page turn waits for the next render.
    pub(super) fn moving(&self) -> bool {
        self.turns != 0 || self.snap
    }
}

impl Reader {
    pub(super) fn paged(&self) -> bool {
        self.mode == Mode::Paginated
    }

    pub(super) fn show_position(&mut self, position: Position) {
        self.pages.at = position;
        self.pages.page = None;
        self.pages.snap = false;
        self.pages.turns = 0;
    }

    /// Shows the last page of a chapter.
    pub(super) fn show_end(&mut self, spine: usize) {
        self.show_position(Position {
            spine,
            offset: usize::MAX,
        });
        self.pages.snap = true;
    }

    pub(super) fn turn(&mut self, turns: isize, cx: &mut Context<Self>) {
        self.pages.turns += turns;
        self.scrolled(cx);
        cx.notify();
    }

    /// The first item on screen.
    pub(super) fn top_item(&self) -> usize {
        if !self.paged() {
            return self.list.logical_scroll_top().item_ix;
        }
        let shown = self
            .pages
            .page
            .and_then(|(spine, index)| self.pages.chapters.get(&spine)?.get(index))
            .and_then(|page| page.slices.first());
        match shown {
            Some(slice) => slice.item,
            None => self
                .chapters
                .get(self.pages.at.spine)
                .map_or(0, |chapter| chapter.first),
        }
    }

    pub(super) fn wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = match event.delta {
            ScrollDelta::Lines(lines) => lines.y * WHEEL_TURN,
            ScrollDelta::Pixels(pixels) => f32::from(pixels.y),
        };
        self.pages.wheel += delta;
        if self.pages.wheel.abs() >= WHEEL_TURN {
            // A positive delta moves the text down, back to earlier pages.
            let turns = if self.pages.wheel > 0. { -1 } else { 1 };
            self.pages.wheel = 0.;
            self.turn(turns, cx);
        }
    }

    fn geometry(&self, window: &Window) -> Geometry {
        let viewport = window.viewport_size();
        let panel = self.panel_width();
        let width = f32::from(viewport.width) - panel;
        let columns = if width >= 2. * COLUMN_WIDTH + 4. * MARGIN {
            2
        } else {
            1
        };
        let column = if columns == 2 {
            COLUMN_WIDTH
        } else {
            (width - 2. * MARGIN).clamp(100., COLUMN_WIDTH)
        };
        let height =
            (content_height(window) - self.bars_height() - TEXT_TOP - TEXT_BOTTOM).max(100.);
        Geometry {
            columns,
            column,
            height,
            panel,
            font: self.font,
            font_size: self.font_size,
            hide_bars: self.hide_bars,
        }
    }

    /// Makes the page estimate again when the geometry or the items change.
    /// Only a complete book has one.
    pub(super) fn estimate_pages(&mut self, window: &Window) {
        if self.chapters.len() < self.spine_len {
            return;
        }
        let geometry = self.geometry(window);
        let key = (geometry, self.items.len());
        if self.pages.estimate.key == Some(key) {
            return;
        }
        let char_width = self.char_width(window);
        let mut items = Vec::with_capacity(self.items.len());
        let mut y = 0.;
        for ix in 0..self.items.len() {
            if self.items[ix].chapter_start {
                y = (y / geometry.height).ceil() * geometry.height;
            }
            let height = self.estimated_height(ix, geometry, char_width);
            items.push((y, height));
            y += height;
        }
        self.pages.estimate = Estimate {
            key: Some(key),
            items,
            pages: ((y / geometry.height).ceil() as usize).max(1),
        };
    }

    /// The page at the position and the pages in the book.
    pub(super) fn page_count(&self, position: Position) -> Option<(usize, usize)> {
        let estimate = &self.pages.estimate;
        let (geometry, len) = estimate.key?;
        if len != self.items.len() {
            return None;
        }
        let ix = self.item_at(position)?;
        let item = &self.items[ix];
        let (top, height) = estimate.items[ix];
        let into = position.offset.saturating_sub(item.block.offset);
        let part = (into as f32 / item.len.max(1) as f32).min(1.);
        let page = ((top + part * height) / geometry.height) as usize + 1;
        Some((page.min(estimate.pages), estimate.pages))
    }

    fn estimated_height(&self, ix: usize, geometry: Geometry, char_width: f32) -> f32 {
        let item = &self.items[ix];
        let size = self.font_size;
        let spacing = self.spacing(ix);
        let text = |scale: f32, line: f32| {
            let per_line = (geometry.column / (char_width * scale) * WRAP).max(1.);
            let lines: f32 = item
                .text
                .split('\n')
                .map(|text| (text.chars().count() as f32 / per_line).ceil().max(1.))
                .sum();
            lines * line
        };
        let content = match &item.block.kind {
            BlockKind::Heading { level, .. } => {
                let scale = HEADING_SCALE[(*level as usize).clamp(1, 6) - 1];
                text(scale, size * scale * HEADING_SPACING)
            }
            BlockKind::Paragraph { .. } | BlockKind::ListItem { .. } => {
                text(1., self.line_height())
            }
            BlockKind::Pre { .. } => text(PRE_SCALE, size * PRE_SCALE * LINE_SPACING),
            BlockKind::Image { href, .. } => match self.pictures.get(href) {
                Some(picture) => {
                    let (w, h) = (picture.width as f32, picture.height as f32);
                    h * (geometry.column / w).min(geometry.height / h).min(1.)
                }
                None => self.line_height(),
            },
            BlockKind::Rule => 1.,
            BlockKind::Error { .. } => self.line_height(),
        };
        spacing.top + content + spacing.bottom + spacing.quote
    }

    fn char_width(&self, window: &Window) -> f32 {
        let run = TextRun {
            len: SAMPLE.len(),
            font: font(self.font_family()),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(SAMPLE.into(), px(self.font_size), &[run], None);
        (f32::from(line.width) / SAMPLE.chars().count() as f32).max(1.)
    }

    /// Makes the pages that the next frame shows and applies the page turns.
    /// Returns the geometry to draw with. After a resize, the old pages stay
    /// until the size is stable.
    pub(super) fn update_pages(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Geometry> {
        let new = self.geometry(window);
        let apply = match (self.pages.geometry, self.pages.resized) {
            (Some(old), _) if old == new => {
                self.pages.resized = None;
                false
            }
            (Some(old), Some((size, since))) if old.resized(&new) && size == new => {
                since.elapsed() >= RESIZE_DELAY
            }
            (Some(old), _) if old.resized(&new) => {
                self.pages.resized = Some((new, Instant::now()));
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(RESIZE_DELAY).await;
                    this.update(cx, |_, cx| cx.notify()).ok();
                })
                .detach();
                false
            }
            _ => true,
        };
        if apply {
            self.pages.geometry = Some(new);
            self.pages.resized = None;
            self.pages.chapters.clear();
            self.pages.page = None;
        }
        let geometry = self.pages.geometry?;

        loop {
            let spine = self.pages.at.spine;
            if spine >= self.chapters.len() {
                return Some(geometry);
            }
            self.paginate(spine, geometry, window, cx);
            let pages = &self.pages.chapters[&spine];
            let index = match self.pages.page {
                Some((shown, index)) if shown == spine => index,
                _ => page_at(pages, self.pages.at.offset),
            };
            let index = index - index % geometry.columns;
            self.pages.page = Some((spine, index));
            if self.pages.snap {
                self.pages.snap = false;
                self.pages.at.offset = pages[index].start;
            }

            let step = geometry.columns;
            if self.pages.turns > 0 {
                self.pages.turns -= 1;
                if index + step < pages.len() {
                    self.pages.page = Some((spine, index + step));
                    self.pages.at.offset = pages[index + step].start;
                } else if spine + 1 < self.chapters.len() {
                    self.pages.at = Position {
                        spine: spine + 1,
                        offset: 0,
                    };
                    self.pages.page = None;
                } else {
                    self.pages.turns = 0;
                }
            } else if self.pages.turns < 0 {
                self.pages.turns += 1;
                if index >= step {
                    self.pages.page = Some((spine, index - step));
                    self.pages.at.offset = pages[index - step].start;
                } else if spine > 0 {
                    self.pages.at = Position {
                        spine: spine - 1,
                        offset: usize::MAX,
                    };
                    self.pages.page = None;
                    self.pages.snap = true;
                } else {
                    self.pages.turns = 0;
                }
            } else {
                return Some(geometry);
            }
        }
    }

    /// Puts the blocks of a chapter on pages. A page breaks a text block
    /// between two lines. A block that cannot break goes to the next page.
    /// A block taller than an empty page is cut. A heading stays on the page
    /// of the first line after it.
    fn paginate(
        &mut self,
        spine: usize,
        geometry: Geometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pages.chapters.contains_key(&spine) {
            return;
        }
        let height = geometry.height;
        let mut pages = vec![Page {
            start: 0,
            slices: Vec::new(),
        }];
        let first = self.chapters[spine].first;
        let measures: Vec<Measure> = (first..self.chapter_end(spine))
            .map(|ix| self.measure(ix, geometry, window, cx))
            .collect();
        let mut y = 0.;
        for (i, measure) in measures.iter().enumerate() {
            let ix = first + i;
            if self.is_heading(ix)
                && let Some(page) = pages.last()
                && !page.slices.is_empty()
                && y + self.heading_group(ix, &measures[i..]) > height + 0.5
            {
                pages.push(Page {
                    start: self.items[ix].block.offset,
                    slices: Vec::new(),
                });
                y = 0.;
            }
            let mut from = 0.;
            loop {
                let room = height - y;
                let page = pages.last_mut().expect("there is always a page");
                if measure.end - from <= room + 0.5 {
                    page.slices.push(Slice {
                        item: ix,
                        top: from,
                        height: (measure.height - from).min(room),
                    });
                    y += measure.height - from;
                    break;
                }
                if let Some((top, line)) = measure.lines {
                    let cut = top + ((room + from - top) / line).floor() * line;
                    if cut > from + 0.5 {
                        page.slices.push(Slice {
                            item: ix,
                            top: from,
                            height: cut - from,
                        });
                        from = cut;
                    }
                }
                if page.slices.is_empty() {
                    page.slices.push(Slice {
                        item: ix,
                        top: from,
                        height: room,
                    });
                    y = height;
                    break;
                }
                pages.push(Page {
                    start: self.offset_in(ix, measure, from),
                    slices: Vec::new(),
                });
                y = 0.;
            }
        }
        self.pages.chapters.insert(spine, pages);
    }

    fn is_heading(&self, ix: usize) -> bool {
        matches!(self.items[ix].block.kind, BlockKind::Heading { .. })
    }

    /// The height of the headings from item `start`, and the first line of
    /// the block after them. `measures` start at the same item.
    fn heading_group(&self, start: usize, measures: &[Measure]) -> f32 {
        let mut height = 0.;
        for (i, measure) in measures.iter().enumerate() {
            if !self.is_heading(start + i) {
                return height + measure.lines.map_or(measure.end, |(top, line)| top + line);
            }
            height += measure.height;
        }
        height
    }

    fn measure(
        &self,
        ix: usize,
        geometry: Geometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Measure {
        let (mut element, _) = self.render_block(ix, None, geometry.column, geometry.height, cx);
        let space = size(
            AvailableSpace::Definite(px(geometry.column)),
            AvailableSpace::MinContent,
        );
        let height = f32::from(element.layout_as_root(space, window, cx).height);
        let spacing = self.spacing(ix);
        let end = (height - spacing.bottom - spacing.quote).max(0.);
        // GPUI rounds the line height, so it comes from the measured height.
        let lines = spacing.line.map(|guess| {
            let count = ((end - spacing.top) / guess).round().max(1.);
            (spacing.top, (end - spacing.top) / count)
        });
        Measure { height, end, lines }
    }

    /// The character offset in the chapter at `from` pixels in a block.
    fn offset_in(&self, ix: usize, measure: &Measure, from: f32) -> usize {
        let item = &self.items[ix];
        let into = match measure.lines {
            Some((top, _)) if from > top && measure.end > top => {
                let part = ((from - top) / (measure.end - top)).min(1.);
                (part * item.len as f32).round() as usize
            }
            _ => 0,
        };
        item.block.offset + into
    }

    /// The pages on screen, and the hrefs of images that must be decoded.
    pub(super) fn render_pages(
        &self,
        geometry: Geometry,
        cx: &mut Context<Self>,
    ) -> (AnyElement, Vec<String>) {
        let view = cx.entity();
        let pages = self
            .pages
            .page
            .and_then(|(spine, index)| Some((self.pages.chapters.get(&spine)?, index)));
        let mut decode = Vec::new();
        let mut columns = Vec::new();
        for side in 0..geometry.columns {
            let mut column = div()
                .flex_none()
                .w(px(geometry.column))
                .h(px(geometry.height))
                .overflow_hidden();
            if let Some(page) = pages.and_then(|(pages, index)| pages.get(index + side)) {
                for slice in &page.slices {
                    let (block, href) = self.render_block(
                        slice.item,
                        Some(&view),
                        geometry.column,
                        geometry.height,
                        cx,
                    );
                    decode.extend(href);
                    // One block can be on both pages of a spread.
                    let id = (if side == 0 { "left" } else { "right" }, slice.item);
                    column = column.child(
                        div()
                            .id(id)
                            .relative()
                            .w_full()
                            .h(px(slice.height))
                            .overflow_hidden()
                            .child(div().absolute().left_0().top(px(-slice.top)).child(block)),
                    );
                }
            }
            if side > 0 {
                columns.push(
                    div()
                        .flex_none()
                        .w(px(1.))
                        .h(px(geometry.height))
                        .bg(cx.theme().border),
                );
            }
            columns.push(column);
        }
        let element = div()
            .id("pages")
            .size_full()
            .flex()
            .justify_center()
            .gap(px(MARGIN - 0.5))
            .pt(px(TEXT_TOP))
            .pr(px(geometry.panel))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .children(columns)
            .into_any_element();
        (element, decode)
    }
}

/// The page that holds the offset. Of pages with the same start, the first.
fn page_at(pages: &[Page], offset: usize) -> usize {
    let last = pages
        .partition_point(|p| p.start <= offset)
        .saturating_sub(1);
    let start = pages[last].start;
    pages.partition_point(|p| p.start < start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(starts: &[usize]) -> Vec<Page> {
        starts
            .iter()
            .map(|&start| Page {
                start,
                slices: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn page_at_offset() {
        // An image page (no text) starts at the same offset as the page after it.
        let pages = pages(&[0, 100, 100, 250]);
        assert_eq!(page_at(&pages, 0), 0);
        assert_eq!(page_at(&pages, 99), 0);
        assert_eq!(page_at(&pages, 100), 1);
        assert_eq!(page_at(&pages, 249), 1);
        assert_eq!(page_at(&pages, usize::MAX), 3);
    }
}

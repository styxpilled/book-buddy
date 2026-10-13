use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use book_buddy::book::BlockKind;
use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, Entity,
    HighlightStyle, InteractiveElement, InteractiveText, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, Pixels, Point, Styled, TextLayout, Window, canvas, div, point,
    prelude::FluentBuilder, px,
};
use gpui_kit::component::ActiveTheme;

use super::{CopySelection, Reader, marker_label};

/// With the pointer this close to the top or bottom of the list, a drag scrolls it.
const SCROLL_ZONE: f32 = 40.;
const SCROLL_TICK: Duration = Duration::from_millis(16);
/// Scroll per tick, as a part of the distance into the zone.
const SCROLL_RATE: f32 = 0.25;
const SCROLL_MAX: f32 = 40.;

/// An item index and a byte index into the item text.
type Caret = (usize, usize);

pub(super) struct Selection {
    anchor: Caret,
    head: Caret,
    /// The left button is down after a click on text.
    dragging: bool,
    /// After a double-click, the first word. A drag then extends by words.
    word: Option<(usize, Range<usize>)>,
    pointer: Point<Pixels>,
    scrolling: bool,
}

impl Selection {
    fn ends(&self) -> (Caret, Caret) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }

    /// The selected part of the text of an item.
    fn range_in(&self, item: usize, len: usize) -> Option<Range<usize>> {
        let (start, end) = self.ends();
        if item < start.0 || item > end.0 {
            return None;
        }
        let from = if item == start.0 { start.1 } else { 0 };
        let to = if item == end.0 { end.1 } else { len };
        Some(from..to)
    }
}

/// Text that was painted in the last frame. Only painted text can find the
/// index at a point.
pub(super) struct Painted {
    item: usize,
    /// The part of the text bounds inside the clip.
    visible: Bounds<Pixels>,
    layout: TextLayout,
}

pub(super) type PaintedText = Rc<RefCell<Vec<Painted>>>;

impl Reader {
    /// The text of an item, with search and selection highlights, link clicks
    /// and selection with the mouse.
    pub(super) fn text_element(&self, ix: usize, view: &Entity<Reader>, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let item = &self.items[ix];
        let search = self.search.marked(ix).map(|range| {
            let style = HighlightStyle {
                color: Some(theme.background),
                background_color: Some(theme.foreground),
                ..Default::default()
            };
            (range, style)
        });
        let selection = self.selection.as_ref();
        let selected = selection
            .and_then(|s| s.range_in(ix, item.text.len()))
            .map(|range| {
                let style = HighlightStyle {
                    background_color: Some(theme.selection),
                    ..Default::default()
                };
                (range, style)
            });
        let text = item.styled_text(theme, search.into_iter().chain(selected));
        let layout = text.layout().clone();

        let links: Vec<(Range<usize>, String)> = item
            .styles
            .iter()
            .filter_map(|(range, run)| Some((range.clone(), run.link.clone()?)))
            .collect();
        let link_ranges: Vec<_> = links.iter().map(|(range, _)| range.clone()).collect();
        let text = if links.is_empty() {
            text.into_any_element()
        } else {
            let view = view.clone();
            InteractiveText::new(("text", ix), text)
                .on_click(link_ranges.clone(), move |link, _, cx| {
                    view.update(cx, |reader, cx| reader.follow(&links[link].1, cx));
                })
                .into_any_element()
        };

        div()
            .relative()
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(MouseButton::Left, {
                let view = view.clone();
                let layout = layout.clone();
                move |event: &MouseDownEvent, _, cx| {
                    let index = layout
                        .index_for_position(event.position)
                        .unwrap_or_else(|closest| closest);
                    if link_ranges.iter().any(|range| range.contains(&index)) {
                        return;
                    }
                    view.update(cx, |reader, cx| reader.select_start((ix, index), event, cx));
                }
            })
            .child(text)
            .when(self.dragging(), |element| {
                let painted = self.painted.clone();
                element.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            let visible = layout.bounds().intersect(&window.content_mask().bounds);
                            if !visible.is_empty() {
                                painted.borrow_mut().push(Painted {
                                    item: ix,
                                    visible,
                                    layout,
                                });
                            }
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
            .into_any_element()
    }

    /// During a drag, moves anywhere in the window extend the selection.
    pub(super) fn drag_listener(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.dragging() {
            return None;
        }
        let view = cx.entity();
        let element = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        view.update(cx, |reader, cx| reader.select_drag(event, cx));
                    }
                });
            },
        )
        .absolute()
        .size_0()
        .into_any_element();
        Some(element)
    }

    fn select_start(&mut self, caret: Caret, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let (item, index) = caret;
        let word = (event.click_count == 2).then(|| word_at(&self.items[item].text, index));
        let (anchor, head) = match &word {
            Some(word) => ((item, word.start), (item, word.end)),
            None => (caret, caret),
        };
        self.selection = Some(Selection {
            anchor,
            head,
            dragging: event.click_count <= 2,
            word: word.map(|word| (item, word)),
            pointer: event.position,
            scrolling: false,
        });
        cx.notify();
    }

    fn select_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(selection) = &mut self.selection else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            selection.dragging = false;
            cx.notify();
            return;
        }
        selection.pointer = event.position;
        self.extend_to(event.position, cx);
        self.start_scroll(cx);
    }

    fn extend_to(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(caret) = self.caret_at(point) else {
            return;
        };
        let Some(selection) = &mut self.selection else {
            return;
        };
        let (anchor, head) = match &selection.word {
            Some((item, first)) => {
                let word = word_at(&self.items[caret.0].text, caret.1);
                if (caret.0, word.start) < (*item, first.start) {
                    ((*item, first.end), (caret.0, word.start))
                } else {
                    let end = (caret.0, word.end).max((*item, first.end));
                    ((*item, first.start), end)
                }
            }
            None => (selection.anchor, caret),
        };
        if (anchor, head) != (selection.anchor, selection.head) {
            selection.anchor = anchor;
            selection.head = head;
            cx.notify();
        }
    }

    /// The caret at a point of the window. Off the text, the caret in the
    /// nearest painted text.
    fn caret_at(&self, at: Point<Pixels>) -> Option<Caret> {
        let painted = self.painted.borrow();
        let nearest = painted
            .iter()
            .min_by(|a, b| distance(&a.visible, at).total_cmp(&distance(&b.visible, at)))?;
        // On the bottom edge, the index is 0.
        let max = nearest.visible.bottom_right() - point(px(1.), px(1.));
        let inside = at.clamp(&nearest.visible.origin, &max);
        let index = nearest
            .layout
            .index_for_position(inside)
            .unwrap_or_else(|closest| closest);
        Some((nearest.item, index))
    }

    /// Scrolls the list while the pointer is near its top or bottom.
    fn start_scroll(&mut self, cx: &mut Context<Self>) {
        let list = self.list.viewport_bounds();
        let paged = self.paged();
        let Some(selection) = &mut self.selection else {
            return;
        };
        if selection.scrolling || paged || edge_speed(&list, selection.pointer) == 0. {
            return;
        }
        selection.scrolling = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SCROLL_TICK).await;
                let more = this.update(cx, |reader, cx| reader.scroll_tick(cx));
                if !more.unwrap_or(false) {
                    break;
                }
            }
        })
        .detach();
    }

    fn scroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let list = self.list.viewport_bounds();
        let paged = self.paged();
        let Some(selection) = &mut self.selection else {
            return false;
        };
        let pointer = selection.pointer;
        let speed = if selection.dragging && !paged {
            edge_speed(&list, pointer)
        } else {
            0.
        };
        if speed == 0. {
            selection.scrolling = false;
            return false;
        }
        // The text painted in the last frame is at the last scroll position.
        self.extend_to(pointer, cx);
        self.scroll_by(speed, cx);
        true
    }

    /// A press anywhere clears the selection. A press on text then starts a new one.
    pub(super) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn end_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(selection) = &mut self.selection
            && selection.dragging
        {
            selection.dragging = false;
            cx.notify();
        }
    }

    pub(super) fn dragging(&self) -> bool {
        self.selection.as_ref().is_some_and(|s| s.dragging)
    }

    /// The selected text of each block, with a blank line between blocks. A
    /// list item gets its marker when the selection has its start and more
    /// than one block.
    pub(super) fn copy(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = &self.selection else {
            return;
        };
        let (start, end) = selection.ends();
        let parts: Vec<String> = (start.0..=end.0)
            .filter_map(|ix| {
                let item = &self.items[ix];
                let range = selection.range_in(ix, item.text.len())?;
                if range.is_empty() {
                    return None;
                }
                let marker = match &item.block.kind {
                    BlockKind::ListItem {
                        marker: Some(marker),
                        ..
                    } if range.start == 0 && start.0 != end.0 => marker_label(marker) + " ",
                    _ => String::new(),
                };
                Some(marker + &item.text[range])
            })
            .collect();
        if !parts.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(parts.join("\n\n")));
        }
    }
}

fn distance(bounds: &Bounds<Pixels>, point: Point<Pixels>) -> f32 {
    let dx = (bounds.left() - point.x)
        .max(point.x - bounds.right())
        .max(px(0.));
    let dy = (bounds.top() - point.y)
        .max(point.y - bounds.bottom())
        .max(px(0.));
    f32::from(dx).hypot(f32::from(dy))
}

/// Scroll per tick for a pointer near the top (negative) or the bottom of the list.
fn edge_speed(list: &Bounds<Pixels>, pointer: Point<Pixels>) -> f32 {
    let above = f32::from(list.top() - pointer.y) + SCROLL_ZONE;
    let below = f32::from(pointer.y - list.bottom()) + SCROLL_ZONE;
    if above > 0. {
        -(above * SCROLL_RATE).min(SCROLL_MAX)
    } else if below > 0. {
        (below * SCROLL_RATE).min(SCROLL_MAX)
    } else {
        0.
    }
}

/// The word at a byte index: letters, digits and apostrophes. Any other char
/// is a word of its own.
fn word_at(text: &str, index: usize) -> Range<usize> {
    let is_word = |c: char| c.is_alphanumeric() || matches!(c, '\'' | '’' | '_');
    let Some(c) = text[index.min(text.len())..].chars().next() else {
        return index..index;
    };
    if !is_word(c) {
        return index..index + c.len_utf8();
    }
    let start = text[..index]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(index, |(i, _)| i);
    let end = text[index..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(text.len(), |(i, _)| index + i);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words() {
        let text = "Debt florist's mush, café!";
        assert_eq!(&text[word_at(text, 0)], "Debt");
        assert_eq!(&text[word_at(text, 7)], "florist's");
        assert_eq!(&text[word_at(text, 13)], "florist's");
        assert_eq!(&text[word_at(text, 19)], ",");
        assert_eq!(&text[word_at(text, 21)], "café");
        assert_eq!(word_at(text, text.len()), text.len()..text.len());
    }

    #[test]
    fn range_across_items() {
        let selection = Selection {
            anchor: (5, 3),
            head: (2, 4),
            dragging: false,
            word: None,
            pointer: Point::default(),
            scrolling: false,
        };
        assert_eq!(selection.range_in(1, 10), None);
        assert_eq!(selection.range_in(2, 10), Some(4..10));
        assert_eq!(selection.range_in(3, 10), Some(0..10));
        assert_eq!(selection.range_in(5, 10), Some(0..3));
        assert_eq!(selection.range_in(6, 10), None);
    }
}

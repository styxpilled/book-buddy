use std::ops::Range;

use gpui::{
    AnyElement, App, ClipboardItem, Context, CursorStyle, DispatchPhase, Entity, HighlightStyle,
    InteractiveElement, InteractiveText, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, Styled, TextLayout, Window, canvas, div, prelude::FluentBuilder,
};
use gpui_kit::component::ActiveTheme;

use super::{CopySelection, Reader};

/// A selection inside the text of one item. Indices are bytes.
pub(super) struct Selection {
    item: usize,
    anchor: usize,
    head: usize,
    /// The left button is down after a click in the item.
    dragging: bool,
    /// After a double-click, the first word. A drag then extends by words.
    word: Option<Range<usize>>,
}

impl Selection {
    fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

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
        let selection = self.selection.as_ref().filter(|s| s.item == ix);
        let selected = selection.map(|s| {
            let style = HighlightStyle {
                background_color: Some(theme.selection),
                ..Default::default()
            };
            (s.range(), style)
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

        let dragging = selection.is_some_and(|s| s.dragging);
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
                    view.update(cx, |reader, cx| {
                        reader.select_start(ix, index, event.click_count, cx)
                    });
                }
            })
            .child(text)
            .when(dragging, |element| {
                // Moves outside the item count too, so the listener is on the
                // window, and only while this frame of the text is painted.
                let view = view.clone();
                element.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble {
                                    view.update(cx, |reader, cx| {
                                        reader.select_drag(event, &layout, cx)
                                    });
                                }
                            });
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
            .into_any_element()
    }

    fn select_start(&mut self, item: usize, index: usize, clicks: usize, cx: &mut Context<Self>) {
        let word = (clicks == 2).then(|| word_at(&self.items[item].text, index));
        let (anchor, head) = match &word {
            Some(word) => (word.start, word.end),
            None => (index, index),
        };
        self.selection = Some(Selection {
            item,
            anchor,
            head,
            dragging: clicks <= 2,
            word,
        });
        cx.notify();
    }

    fn select_drag(&mut self, event: &MouseMoveEvent, layout: &TextLayout, cx: &mut Context<Self>) {
        let Some(selection) = &mut self.selection else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            selection.dragging = false;
            cx.notify();
            return;
        }
        let index = layout
            .index_for_position(event.position)
            .unwrap_or_else(|closest| closest);
        let (anchor, head) = match &selection.word {
            Some(first) => {
                let word = word_at(&self.items[selection.item].text, index);
                if word.start < first.start {
                    (first.end, word.start)
                } else {
                    (first.start, word.end.max(first.end))
                }
            }
            None => (selection.anchor, index),
        };
        if (anchor, head) != (selection.anchor, selection.head) {
            selection.anchor = anchor;
            selection.head = head;
            cx.notify();
        }
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

    pub(super) fn copy(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = &self.selection else {
            return;
        };
        let range = selection.range();
        if !range.is_empty() {
            let text = &self.items[selection.item].text[range];
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
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
}

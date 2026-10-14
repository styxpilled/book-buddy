use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use book_buddy::library::Position;
use book_buddy::search::find;
use gpui::{
    AppContext, Context, Entity, FontWeight, HighlightStyle, InteractiveElement, IntoElement,
    ParentElement, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, StyledText,
    Subscription, Task, UniformListScrollHandle, Window, div, prelude::FluentBuilder, px, relative,
    uniform_list,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use crate::Back;
use crate::ui::style;

use super::{Adjust, NextMatch, OpenSearch, PreviousMatch, Reader, SANS};

pub(super) const PANEL_WIDTH: f32 = 340.;
const ROW_HEIGHT: f32 = 52.;
const MAX_RESULTS: usize = 10_000;
const DEBOUNCE: Duration = Duration::from_millis(150);
/// Characters of context before and after a match in the results.
const BEFORE: usize = 30;
const AFTER: usize = 80;

#[derive(Default)]
pub(super) struct Search {
    pub(super) open: bool,
    input: Option<Entity<InputState>>,
    query: String,
    results: Vec<Match>,
    truncated: bool,
    /// The result that is highlighted in the text.
    current: Option<usize>,
    task: Option<Task<()>>,
    scroll: UniformListScrollHandle,
    _subscription: Option<Subscription>,
}

struct Match {
    item: usize,
    /// Bytes in the item text.
    range: Range<usize>,
}

impl Search {
    /// The highlighted match, if it is in this item.
    pub(super) fn marked(&self, item: usize) -> Option<Range<usize>> {
        let found = &self.results[self.current?];
        (found.item == item).then(|| found.range.clone())
    }
}

impl Reader {
    pub(super) fn open_search(
        &mut self,
        _: &OpenSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = match &self.search.input {
            Some(input) => input.clone(),
            None => {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("text"));
                self.search._subscription = Some(cx.subscribe_in(
                    &input,
                    window,
                    |reader, input, event: &InputEvent, _, cx| match event {
                        InputEvent::Change => {
                            let query = input.read(cx).value().to_string();
                            reader.start_search(query, cx);
                        }
                        InputEvent::PressEnter { shift, .. } => {
                            reader.step_match(if *shift { -1 } else { 1 }, cx)
                        }
                        _ => {}
                    },
                ));
                self.search.input = Some(input.clone());
                input
            }
        };
        if !self.search.open {
            self.search.open = true;
            self.list.remeasure();
        }
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = false;
        self.search.current = None;
        self.list.remeasure();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn start_search(&mut self, query: String, cx: &mut Context<Self>) {
        // The input also sends a change when Enter is pressed.
        if query == self.search.query {
            return;
        }
        self.search.query = query.clone();
        self.search.current = None;
        if query.trim().is_empty() {
            self.search.task = None;
            self.search.results.clear();
            self.search.truncated = false;
            cx.notify();
            return;
        }
        let texts: Arc<Vec<SharedString>> =
            Arc::new(self.items.iter().map(|item| item.text.clone()).collect());
        self.search.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let (results, truncated) = cx
                .background_spawn(async move { search(&texts, &query) })
                .await;
            this.update(cx, |reader, cx| {
                reader.search.results = results;
                reader.search.current = None;
                reader.search.truncated = truncated;
                reader.search.scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            })
            .ok();
        }));
    }

    pub(super) fn next_match(&mut self, _: &NextMatch, _: &mut Window, cx: &mut Context<Self>) {
        self.step_match(1, cx);
    }

    pub(super) fn previous_match(
        &mut self,
        _: &PreviousMatch,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_match(-1, cx);
    }

    /// The next or previous result. With no result shown, it starts from the
    /// top of the window.
    fn step_match(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.search.results.len();
        if count == 0 {
            return;
        }
        let ix = match self.search.current {
            Some(current) => (current as isize + step).rem_euclid(count as isize) as usize,
            None => {
                let top = self.top_item();
                let after = self.search.results.partition_point(|m| m.item < top);
                if step > 0 {
                    after % count
                } else {
                    (after + count - 1) % count
                }
            }
        };
        self.show_match(ix, cx);
    }

    fn show_match(&mut self, ix: usize, cx: &mut Context<Self>) {
        let found = &self.search.results[ix];
        let item = &self.items[found.item];
        let into = item.text[..found.range.start].chars().count();
        let position = Position {
            spine: item.spine,
            offset: item.block.offset + into,
        };
        self.search.current = Some(ix);
        self.search
            .scroll
            .scroll_to_item(ix, ScrollStrategy::Center);
        self.scroll_to_position(position);
        // A few lines above the match stay in view.
        let line = self.line_height();
        if let Some(Adjust::Part { margin, .. }) = &mut self.adjust {
            *margin = 3. * line;
        }
        self.scrolled(cx);
        cx.notify();
    }

    pub(super) fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let count = self.search.results.len();
        let current = self.search.current;
        let number = match current {
            Some(ix) => format!("{:02}/{count:02}", ix + 1),
            None => format!("{count:02}"),
        };
        let number = if self.search.truncated {
            format!("{number}+")
        } else {
            number
        };
        let label = match count {
            0 => "NO MATCHES",
            1 => "MATCH",
            _ => "MATCHES",
        };
        let query_empty = self
            .search
            .input
            .as_ref()
            .is_none_or(|input| input.read(cx).value().trim().is_empty());
        let list =
            uniform_list(
                "search-results",
                count,
                cx.processor(move |reader, range: Range<usize>, _, cx| {
                    let theme = cx.theme();
                    let (ink, paper, hover, muted) = (
                        theme.foreground,
                        theme.background,
                        theme.list_hover,
                        theme.muted_foreground,
                    );
                    let hairline = style::hairline(cx);
                    range
                        .map(|ix| {
                            let found = &reader.search.results[ix];
                            let item = &reader.items[found.item];
                            let chapter = reader
                                .toc_at(Position {
                                    spine: item.spine,
                                    offset: item.block.offset,
                                })
                                .and_then(|toc| Some(reader.book.as_ref()?.toc[toc].label.clone()))
                                .unwrap_or_else(|| format!("Part {}", item.spine + 1));
                            let (text, mark) = snippet(&item.text, found.range.clone());
                            let bold = HighlightStyle {
                                font_weight: Some(FontWeight::BOLD),
                                ..Default::default()
                            };
                            let is_current = Some(ix) == current;
                            div()
                                .id(ix)
                                .w_full()
                                .h(px(ROW_HEIGHT))
                                .px(px(16.))
                                .flex()
                                .flex_col()
                                .justify_center()
                                .border_b_1()
                                .border_color(hairline)
                                .cursor_pointer()
                                .when(!is_current, |row| row.hover(move |row| row.bg(hover)))
                                .when(is_current, |row| row.bg(ink).text_color(paper))
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(if is_current {
                                            paper.opacity(0.6)
                                        } else {
                                            muted
                                        })
                                        .truncate()
                                        .child(chapter.to_uppercase()),
                                )
                                .child(
                                    div().font_family(SANS).text_size(px(13.)).truncate().child(
                                        StyledText::new(text).with_highlights([(mark, bold)]),
                                    ),
                                )
                                .on_click(
                                    cx.listener(move |reader, _, _, cx| reader.show_match(ix, cx)),
                                )
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.search.scroll)
            .flex_1();

        div()
            .key_context("Search")
            .on_action(cx.listener(|reader, _: &Back, window, cx| reader.close_search(window, cx)))
            .on_action(cx.listener(Self::next_match))
            .on_action(cx.listener(Self::previous_match))
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(PANEL_WIDTH))
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .line_height(relative(1.4))
            .flex()
            .flex_col()
            .bg(theme.background)
            .border_l_1()
            .border_color(theme.border)
            .occlude()
            .child(style::side_shadow(false, cx))
            .children(self.search.input.as_ref().map(|input| {
                div()
                    .mx(px(16.))
                    .mt(px(18.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(style::tag("QUERY", cx))
                    .child(
                        div()
                            .flex_1()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(Input::new(input).appearance(false).cleanable(true)),
                    )
            }))
            .when(!query_empty, |panel| {
                panel.child(
                    div()
                        .px(px(16.))
                        .pt(px(14.))
                        .pb(px(10.))
                        .flex()
                        .items_end()
                        .gap(px(8.))
                        .border_b_1()
                        .border_color(theme.border)
                        .child(style::dot(number, 28.))
                        .child(style::label(label, cx)),
                )
            })
            .child(list)
    }
}

fn search(texts: &[SharedString], query: &str) -> (Vec<Match>, bool) {
    let mut results = Vec::new();
    for (item, text) in texts.iter().enumerate() {
        for range in find(text, query) {
            if results.len() == MAX_RESULTS {
                return (results, true);
            }
            results.push(Match { item, range });
        }
    }
    (results, false)
}

/// Some text around a match, on one line, and the range of the match in it.
fn snippet(text: &str, range: Range<usize>) -> (String, Range<usize>) {
    let start = text[..range.start]
        .char_indices()
        .rev()
        .nth(BEFORE - 1)
        .map_or(0, |(i, _)| i);
    let end = text[range.end..]
        .char_indices()
        .nth(AFTER)
        .map_or(text.len(), |(i, _)| range.end + i);
    let prefix = if start > 0 { "…" } else { "" };
    let snippet = format!("{prefix}{}", &text[start..end]).replace('\n', " ");
    let shift = prefix.len() + range.start - start;
    (snippet, shift..shift + range.len())
}

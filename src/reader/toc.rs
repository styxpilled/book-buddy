use std::ops::Range;

use book_buddy::library::Position;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, ScrollStrategy,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, uniform_list,
};
use gpui_kit::component::ActiveTheme;

use super::{Reader, Target, ToggleToc};
use crate::ui::style;

const TOC_WIDTH: f32 = 330.;
const TOC_ROW_HEIGHT: f32 = 30.;

impl Reader {
    /// Where an entry of the table of contents goes, once its chapter is loaded.
    fn toc_target(&self, ix: usize) -> Option<Target> {
        let (spine, anchor) = self.toc.get(ix)?.clone()?;
        Some(Target {
            spine,
            anchor,
            offset: 0,
        })
    }

    pub(super) fn current_toc(&self) -> Option<usize> {
        self.toc_at(self.position()?)
    }

    /// The last entry of the table of contents at or before the position.
    pub(super) fn toc_at(&self, position: Position) -> Option<usize> {
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

    pub(super) fn toggle_toc(&mut self, _: &ToggleToc, _: &mut Window, cx: &mut Context<Self>) {
        self.toc_open = !self.toc_open;
        if self.toc_open
            && let Some(ix) = self.current_toc()
        {
            self.toc_scroll.scroll_to_item(ix, ScrollStrategy::Center);
        }
        cx.notify();
    }

    pub(super) fn render_toc(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

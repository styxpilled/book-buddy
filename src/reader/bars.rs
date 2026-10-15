use gpui::{
    App, BorderStyle, Bounds, Context, DispatchPhase, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, fill, outline, point,
    prelude::FluentBuilder, px, size,
};
use gpui_kit::component::ActiveTheme;

use super::{BOTTOM_BAR_HEIGHT, MARGIN, Reader, ReaderEvent, TOP_BAR_HEIGHT};
use crate::settings::OpenSettings;
use crate::store::Store;
use crate::ui::style;

/// Distance between the dots of the seek bar.
const SEEK_STEP: f32 = 6.;

impl Reader {
    pub(super) fn render_top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub(super) fn render_bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                            reader.record_jump(cx);
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
}

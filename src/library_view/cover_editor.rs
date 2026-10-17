//! A screen to change the generated cover of a book without a cover image.

use std::sync::Arc;

use book_buddy::cover::{self, Align, Background, CoverFont, OUTLINES, SIZES, TextCover};
use gpui::{
    App, AppContext, BorrowAppContext, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, RenderImage, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, img, prelude::FluentBuilder, px,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};

use super::backdrop::backdrop;
use crate::Back;
use crate::scan;
use crate::store::Store;
use crate::ui::render_image;
use crate::ui::style;

/// Logical pixels of one dot in the preview: twice the size in the library.
const PREVIEW_DOT: f32 = 4.;
const LABEL_WIDTH: f32 = 90.;

pub struct CloseCoverEditor;

pub struct CoverEditor {
    id: String,
    title: SharedString,
    author: String,
    cover: TextCover,
    text: Entity<TextareaState>,
    size: Entity<SliderState>,
    /// The image, and the pixels of a dot and the cover it was made from.
    preview: Option<(Arc<RenderImage>, u32, TextCover)>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<CloseCoverEditor> for CoverEditor {}

impl Focusable for CoverEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CoverEditor {
    /// `None` if the library has no book with this ID.
    pub fn open(id: &str, window: &mut Window, cx: &mut App) -> Option<Entity<Self>> {
        let book = cx.global::<Store>().library.book(id)?.clone();
        Some(cx.new(|cx| {
            let cover = book.text_cover();
            let title = book.shown_title().to_string();
            let text = cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(2, 2)
                    .placeholder(title.clone())
                    .default_value(cover.text.clone())
            });
            let size = cx.new(|_| {
                SliderState::new()
                    .min(*SIZES.start() as f32)
                    .max(*SIZES.end() as f32)
                    .step(1.)
                    .default_value(cover.size as f32)
            });
            let subscriptions = vec![
                cx.subscribe(&text, |this: &mut Self, text, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.cover.text = text.read(cx).value().to_string();
                        cx.notify();
                    }
                }),
                cx.subscribe(&size, |this: &mut Self, _, event: &SliderEvent, cx| {
                    let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
                    this.cover.size = value.end().round() as u32;
                    this.cover.auto_size = false;
                    cx.notify();
                }),
                cx.on_release(|this, cx| {
                    if let Some((image, ..)) = this.preview.take() {
                        cx.drop_image(image, None);
                    }
                }),
            ];
            Self {
                id: book.id.clone(),
                title: title.into(),
                author: book.shown_author().to_string(),
                cover,
                text,
                size,
                preview: None,
                focus: cx.focus_handle(),
                _subscriptions: subscriptions,
            }
        }))
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cover = TextCover::for_book(&self.id);
        let size = self.cover.size as f32;
        self.text
            .update(cx, |text, cx| text.set_value("", window, cx));
        self.size
            .update(cx, |slider, cx| slider.set_value(size, window, cx));
        cx.notify();
    }

    fn shuffle(&mut self, cx: &mut Context<Self>) {
        self.cover.seed = self
            .cover
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let cover = self.cover.clone();
        cx.update_global(|store: &mut Store, _| {
            if let Some(book) = store.library.book_mut(&self.id) {
                book.text_cover = (cover != TextCover::for_book(&self.id)).then_some(cover);
                scan::update_text_cover(&self.id, store);
                store.save();
            }
        });
        cx.emit(CloseCoverEditor);
    }

    /// Makes the preview again if the cover or the scale of the window
    /// changed. A dot is a whole number of device pixels, so its edges are sharp.
    fn update_preview(&mut self, window: &mut Window, cx: &App) -> Arc<RenderImage> {
        let cell = (PREVIEW_DOT * window.scale_factor()).round() as u32;
        if let Some((image, old_cell, old_cover)) = &self.preview
            && (*old_cell, old_cover) == (cell, &self.cover)
        {
            return image.clone();
        }
        let (dark, light) = style::tones(cx);
        let cover = cover::render(&self.cover, &self.title, &self.author, cell);
        let image = render_image(cover::colorize(&cover, dark, light));
        if let Some((old, ..)) = self
            .preview
            .replace((image.clone(), cell, self.cover.clone()))
        {
            window.drop_image(old).ok();
        }
        image
    }

    /// A row of buttons, one for each value. The button of the current value is filled.
    fn choices<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        options: impl IntoIterator<Item = (T, &'static str)>,
        current: T,
        pick: fn(&mut TextCover, T),
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .children(options.into_iter().enumerate().map(|(ix, (value, label))| {
                style::button((id, ix), label, value == current, cx).on_click(cx.listener(
                    move |this, _, _, cx| {
                        pick(&mut this.cover, value);
                        cx.notify();
                    },
                ))
            }))
    }
}

fn row(label: &'static str, control: impl IntoElement, cx: &App) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .child(style::label(label, cx).w(px(LABEL_WIDTH)).pt(px(5.)))
        .child(div().flex_1().min_w_0().child(control))
}

impl Render for CoverEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.update_preview(window, cx);
        let backdrop = backdrop(window, cx);
        let cover = self.cover.clone();

        let fonts = self.choices(
            "font",
            CoverFont::ALL.map(|font| (font, font.label())),
            cover.font,
            |cover, font| cover.font = font,
            cx,
        );
        let styles = self
            .choices(
                "bold",
                [(true, "BOLD")],
                cover.bold,
                |cover, _| cover.bold = !cover.bold,
                cx,
            )
            .child(self.choices(
                "italic",
                [(true, "ITALIC")],
                cover.italic,
                |cover, _| cover.italic = !cover.italic,
                cx,
            ))
            .when(!cover.font.has_styles(), |row| row.opacity(style::DISABLED));
        let size = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(self.choices(
                "auto-size",
                [(true, "AUTO")],
                cover.auto_size,
                |cover, _| cover.auto_size = !cover.auto_size,
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(cover.auto_size, |slider| slider.opacity(style::DISABLED))
                    .child(div().flex_1().child(Slider::new(&self.size)))
                    .child(
                        style::dot(cover.size.to_string(), 16.)
                            .w(px(28.))
                            .text_right(),
                    ),
            );
        let padding = self.choices(
            "padding",
            [(true, "ON"), (false, "OFF")],
            cover.padding,
            |cover, padding| cover.padding = padding,
            cx,
        );
        let outline = self.choices(
            "outline",
            OUTLINES.map(|width| (width, ["NONE", "1 DOT", "2 DOTS"][width as usize])),
            cover.outline,
            |cover, width| cover.outline = width,
            cx,
        );
        let colors = self.choices(
            "colors",
            [(false, "DARK ON LIGHT"), (true, "LIGHT ON DARK")],
            cover.inverted,
            |cover, inverted| cover.inverted = inverted,
            cx,
        );
        let align = self.choices(
            "align",
            [(Align::Left, "LEFT"), (Align::Center, "CENTER")],
            cover.align,
            |cover, align| cover.align = align,
            cx,
        );
        let author = self.choices(
            "author",
            [(true, "SHOW"), (false, "HIDE")],
            cover.author,
            |cover, author| cover.author = author,
            cx,
        );
        let backgrounds = self.choices(
            "background",
            Background::ALL.map(|background| (background, background.label())),
            cover.background,
            |cover, background| cover.background = background,
            cx,
        );
        let text = div()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Textarea::new(&self.text).appearance(false));

        let buttons = div()
            .flex()
            .gap(px(10.))
            .pt(px(10.))
            .child(
                style::button("shuffle", "SHUFFLE", false, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.shuffle(cx))),
            )
            .child(
                style::button("reset", "RESET", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
            )
            .child(div().flex_1())
            .child(
                style::button("back", "ESC", false, cx)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(CloseCoverEditor))),
            )
            .child(
                style::button("save", "SAVE", true, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            );

        let theme = cx.theme();
        div()
            .key_context("CoverEditor")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Back, _, cx| cx.emit(CloseCoverEditor)))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(backdrop)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(20.))
                    .px(px(32.))
                    .pt(px(18.))
                    .child(style::tag_inverted("EDIT COVER", cx))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(12.))
                            .truncate()
                            .child(self.title.clone()),
                    )
                    .child(style::rule(cx)),
            )
            .child(
                div()
                    .id("cover-editor")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(48.))
                    .p(px(32.))
                    .child(
                        div()
                            .flex_none()
                            // The border is outside the image.
                            .w(px(cover::WIDTH as f32 * PREVIEW_DOT + 2.))
                            .h(px(cover::HEIGHT as f32 * PREVIEW_DOT + 2.))
                            .border_1()
                            .border_color(theme.border)
                            .child(img(preview).size_full()),
                    )
                    .child(
                        div()
                            .w(px(400.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(row("TEXT =", text, cx))
                            .child(row("FONT =", fonts, cx))
                            .child(row("STYLE =", styles, cx))
                            .child(row("SIZE =", size, cx))
                            .child(row("OUTLINE =", outline, cx))
                            .child(row("ALIGN =", align, cx))
                            .child(row("PADDING =", padding, cx))
                            .child(row("AUTHOR =", author, cx))
                            .child(row("BACKGROUND =", backgrounds, cx))
                            .child(row("COLORS =", colors, cx))
                            .child(buttons),
                    ),
            )
    }
}

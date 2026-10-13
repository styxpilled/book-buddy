//! A screen to change the tones of one cover before it is dithered.

use std::sync::Arc;

use book_buddy::dither::{ADJUST_RANGE, Adjust, Method, dither_small, dots};
use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, RenderImage, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, img, prelude::FluentBuilder, px,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use image::{Frame, RgbaImage};

use crate::Back;
use crate::store::{Store, cover_path};
use crate::style;

/// Logical pixels of one dot in the preview: twice the size in the library.
const PREVIEW_DOT: f32 = 4.;
const LABEL_WIDTH: f32 = 90.;
/// The label and the lowest value of each slider, in the order of `value_mut`.
const SLIDERS: [(&str, i32); 12] = [
    ("BLACK", 0),
    ("WHITE", 0),
    ("MIDTONES", -ADJUST_RANGE),
    ("CONTRAST", -ADJUST_RANGE),
    ("RED", -ADJUST_RANGE),
    ("ORANGE", -ADJUST_RANGE),
    ("YELLOW", -ADJUST_RANGE),
    ("GREEN", -ADJUST_RANGE),
    ("AQUA", -ADJUST_RANGE),
    ("BLUE", -ADJUST_RANGE),
    ("PURPLE", -ADJUST_RANGE),
    ("MAGENTA", -ADJUST_RANGE),
];
/// The sliders before this one are for tones, the others for hues.
const FIRST_HUE: usize = 4;
const METHODS: [(Method, &str); 3] = [
    (Method::Bayer, "BAYER"),
    (Method::Atkinson, "ATKINSON"),
    (Method::FloydSteinberg, "FLOYD-STEINBERG"),
];

pub struct CloseCoverAdjust;

pub struct CoverAdjust {
    id: String,
    title: SharedString,
    /// The cover with one pixel for each dot, as in the library.
    dots: RgbaImage,
    /// The cover in color, shown while the compare button is held.
    original: Arc<RenderImage>,
    comparing: bool,
    adjust: Adjust,
    sliders: Vec<Entity<SliderState>>,
    /// The image, and the pixels of a dot and the adjustment it was made with.
    preview: Option<(Arc<RenderImage>, u32, Adjust)>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<CloseCoverAdjust> for CoverAdjust {}

impl Focusable for CoverAdjust {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CoverAdjust {
    /// `None` if the book has no cover.
    pub fn open(id: &str, cx: &mut App) -> Option<Entity<Self>> {
        let store = cx.global::<Store>();
        let book = store.library.book(id)?;
        let cover = image::open(cover_path(store.covers(), id)).ok()?;
        let (title, adjust) = (book.shown_title().to_string().into(), book.cover_adjust);
        let cover = cover.into_rgba8();
        let dots = dots(&cover, style::DITHER_CELL);
        let original = render_image(cover);
        Some(cx.new(|cx| Self::new(id.to_string(), title, dots, original, adjust, cx)))
    }

    fn new(
        id: String,
        title: SharedString,
        dots: RgbaImage,
        original: Arc<RenderImage>,
        mut adjust: Adjust,
        cx: &mut Context<Self>,
    ) -> Self {
        let sliders: Vec<_> = SLIDERS
            .iter()
            .enumerate()
            .map(|(ix, &(_, min))| {
                let value = *value_mut(&mut adjust, ix);
                cx.new(|_| {
                    SliderState::new()
                        .min(min as f32)
                        .max(ADJUST_RANGE as f32)
                        .default_value(value as f32)
                })
            })
            .collect();
        let mut subscriptions: Vec<_> = sliders
            .iter()
            .enumerate()
            .map(|(ix, slider)| {
                cx.subscribe(slider, move |this, _, event: &SliderEvent, cx| {
                    let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
                    *value_mut(&mut this.adjust, ix) = value.end().round() as i32;
                    cx.notify();
                })
            })
            .collect();
        subscriptions.push(cx.on_release(|this, cx| {
            if let Some((image, ..)) = this.preview.take() {
                cx.drop_image(image, None);
            }
            cx.drop_image(this.original.clone(), None);
        }));

        Self {
            id,
            title,
            dots,
            original,
            comparing: false,
            adjust,
            sliders,
            preview: None,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.adjust = Adjust::NONE;
        self.update_sliders(window, cx);
    }

    fn auto_levels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        (self.adjust.black, self.adjust.white) = self.adjust.auto_levels(&self.dots);
        self.update_sliders(window, cx);
    }

    /// Moves the sliders to the values of `adjust`.
    fn update_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (ix, slider) in self.sliders.iter().enumerate() {
            let value = *value_mut(&mut self.adjust, ix) as f32;
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
        cx.notify();
    }

    fn set_comparing(&mut self, comparing: bool, cx: &mut Context<Self>) {
        if self.comparing != comparing {
            self.comparing = comparing;
            cx.notify();
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let adjust = self.adjust;
        cx.update_global(|store: &mut Store, _| {
            if let Some(book) = store.library.book_mut(&self.id) {
                book.cover_adjust = adjust;
                store.save();
            }
        });
        cx.emit(CloseCoverAdjust);
    }

    /// Makes the preview again if the adjustment or the scale of the window
    /// changed. A dot is a whole number of device pixels, so its edges are sharp.
    fn update_preview(&mut self, window: &mut Window, cx: &App) -> Arc<RenderImage> {
        let cell = (PREVIEW_DOT * window.scale_factor()).round() as u32;
        let adjust = self.adjust;
        if let Some((image, old_cell, old_adjust)) = &self.preview
            && (*old_cell, *old_adjust) == (cell, adjust)
        {
            return image.clone();
        }
        let (dark, light) = style::tones(cx);
        let image = render_image(dither_small(&self.dots, cell, &adjust, dark, light));
        if let Some((old, ..)) = self.preview.replace((image.clone(), cell, adjust)) {
            window.drop_image(old).ok();
        }
        image
    }
}

/// The value of slider `ix`, in the order of `SLIDERS`.
fn value_mut(adjust: &mut Adjust, ix: usize) -> &mut i32 {
    match ix {
        0 => &mut adjust.black,
        1 => &mut adjust.white,
        2 => &mut adjust.midtones,
        3 => &mut adjust.contrast,
        ix => &mut adjust.hues[ix - FIRST_HUE],
    }
}

fn render_image(mut pixels: RgbaImage) -> Arc<RenderImage> {
    // RenderImage is BGRA.
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![Frame::new(pixels)]))
}

fn signed(value: i32) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

impl Render for CoverAdjust {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.update_preview(window, cx);
        let shown = if self.comparing {
            self.original.clone()
        } else {
            preview
        };
        let theme = cx.theme();
        let (width, height) = self.dots.dimensions();

        let mut adjust = self.adjust;
        let sliders =
            SLIDERS
                .iter()
                .zip(&self.sliders)
                .enumerate()
                .map(|(ix, (&(label, min), slider))| {
                    let value = *value_mut(&mut adjust, ix);
                    let value = if min < 0 {
                        signed(value)
                    } else {
                        value.to_string()
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .when(ix == FIRST_HUE, |row| {
                            row.mt(px(6.))
                                .pt(px(14.))
                                .border_t_1()
                                .border_color(style::hairline(cx))
                        })
                        .child(style::label(format!("{label} ="), cx).w(px(LABEL_WIDTH)))
                        .child(div().flex_1().child(Slider::new(slider)))
                        .child(style::dot(value, 16.).w(px(40.)).text_right())
                });

        let methods = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .mb(px(6.))
            .child(style::label("METHOD =", cx).w(px(LABEL_WIDTH)))
            .child(div().flex().children(METHODS.iter().enumerate().map(
                |(ix, &(method, label))| {
                    style::button(("method", ix), label, method == self.adjust.method, cx)
                        // The borders of neighbours overlap.
                        .when(ix > 0, |button| button.ml(px(-1.)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.adjust.method = method;
                            cx.notify();
                        }))
                },
            )));

        let buttons = div()
            .flex()
            .gap(px(10.))
            .pt(px(10.))
            .child(
                style::button("reset", "RESET", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
            )
            .child(
                style::button("auto", "AUTO", false, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.auto_levels(window, cx))),
            )
            .child(
                style::button("compare", "COMPARE", self.comparing, cx)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.set_comparing(true, cx)),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.set_comparing(false, cx)),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.set_comparing(false, cx)),
                    ),
            )
            .child(div().flex_1())
            .child(
                style::button("back", "ESC", false, cx)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(CloseCoverAdjust))),
            )
            .child(
                style::button("save", "SAVE", true, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            );

        div()
            .key_context("CoverAdjust")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Back, _, cx| cx.emit(CloseCoverAdjust)))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(style::grid(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(20.))
                    .px(px(32.))
                    .pt(px(18.))
                    .child(style::tag_inverted("ADJUST COVER", cx))
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
                    .id("cover-adjust")
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
                            .w(px(width as f32 * PREVIEW_DOT + 2.))
                            .h(px(height as f32 * PREVIEW_DOT + 2.))
                            .border_1()
                            .border_color(theme.border)
                            .child(img(shown).size_full()),
                    )
                    .child(
                        div()
                            .w(px(400.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(methods)
                            .children(sliders)
                            .child(buttons),
                    ),
            )
    }
}

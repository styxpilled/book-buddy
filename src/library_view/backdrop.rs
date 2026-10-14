//! The dithered gradient behind the library: soft shapes of light and dark
//! with ordered dithering to two tones of low contrast.

use std::sync::Arc;

use book_buddy::dither::{Adjust, dither_small};
use gpui::{
    App, AppContext, Global, IntoElement, ParentElement, RenderImage, Styled, Window, div, img, px,
};
use gpui_kit::component::ActiveTheme;
use image::{Rgba, RgbaImage};

use crate::ui::render_image;
use crate::ui::style;

/// Logical pixels of one dot, as on the covers.
const DOT: f32 = 2.;
/// Share of the foreground in the color of the dots.
const CONTRAST: f32 = 0.12;
/// Share of dots in the darkest part of a shape.
const MAX_SHARE: f32 = 0.75;
/// The image grows in steps of this many dots, a multiple of the Bayer map.
const STEP: u32 = 128;

#[derive(Clone, Copy, PartialEq)]
struct Look {
    paper: [u8; 3],
    dots: [u8; 3],
    cell: u32,
    scale: f32,
}

#[derive(Clone, Copy, PartialEq)]
struct Key {
    look: Look,
    /// Size in dots.
    width: u32,
    height: u32,
}

/// One image for all windows. It grows with the window and never shrinks, so
/// a resize makes a new image only when the window becomes larger than before.
#[derive(Default)]
struct Backdrop {
    image: Option<(Arc<RenderImage>, Key)>,
    pending: Option<Key>,
    /// Replaced images, to remove from the sprite atlas of the window.
    old: Vec<Arc<RenderImage>>,
}

impl Global for Backdrop {}

/// The backdrop, behind the content of its parent. Until the first image is
/// made, the parent shows its own background.
pub fn backdrop(window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
    let scale = window.scale_factor();
    let cell = (DOT * scale).round().max(1.) as u32;
    let viewport = window.viewport_size();
    let dots = |side: f32| ((side * scale / cell as f32).ceil() as u32).next_multiple_of(STEP);
    let (width, height) = (
        dots(f32::from(viewport.width)),
        dots(f32::from(viewport.height)),
    );
    let (dark, light) = style::tones(cx);
    let (paper, ink) = if cx.theme().is_dark() {
        (dark, light)
    } else {
        (light, dark)
    };
    let look = Look {
        paper,
        dots: [0, 1, 2].map(|i| mix(paper[i], ink[i], CONTRAST)),
        cell,
        scale,
    };

    let backdrop = cx.default_global::<Backdrop>();
    for image in backdrop.old.drain(..) {
        window.drop_image(image).ok();
    }
    let current = backdrop.image.clone().filter(|(_, key)| key.look == look);
    let covered = current
        .as_ref()
        .is_some_and(|(_, key)| key.width >= width && key.height >= height);
    if !covered {
        let (width, height) = match &current {
            Some((_, key)) => (key.width.max(width), key.height.max(height)),
            None => (width, height),
        };
        let key = Key {
            look,
            width,
            height,
        };
        if backdrop.pending != Some(key) {
            backdrop.pending = Some(key);
            make(key, cx);
        }
    }

    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .overflow_hidden()
        .children(current.map(|(image, key)| {
            let side = |dots: u32| px((dots * key.look.cell) as f32 / key.look.scale);
            img(image)
                .absolute()
                .top_0()
                .left_0()
                .w(side(key.width))
                .h(side(key.height))
        }))
}

fn make(key: Key, cx: &mut App) {
    cx.spawn(async move |cx| {
        let image = cx.background_spawn(async move { render(key) }).await;
        cx.update(|cx| {
            let backdrop = cx.global_mut::<Backdrop>();
            if backdrop.pending != Some(key) {
                return;
            }
            backdrop.pending = None;
            if let Some((old, _)) = backdrop.image.replace((image, key)) {
                backdrop.old.push(old);
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

fn render(key: Key) -> Arc<RenderImage> {
    let dot = key.look.cell as f32 / key.look.scale;
    let small = RgbaImage::from_fn(key.width, key.height, |x, y| {
        let share = shapes((x as f32 + 0.5) * dot, (y as f32 + 0.5) * dot) * MAX_SHARE;
        let tone = ((1. - share) * 255.).round() as u8;
        Rgba([tone, tone, tone, 255])
    });
    let pixels = dither_small(
        &small,
        key.look.cell,
        &Adjust::NONE,
        key.look.dots,
        key.look.paper,
    );
    render_image(pixels)
}

/// The darkness at a point in logical pixels, 0 to 1. Warped waves give soft
/// shapes that do not repeat, with empty parts between them.
fn shapes(x: f32, y: f32) -> f32 {
    let (u, v) = (x / 200., y / 170.);
    let (u, v) = (
        u + 0.6 * (v * 0.9 + 1.3).sin(),
        v + 0.6 * (u * 0.7 + 0.4).sin(),
    );
    let a = (u * 1.1 + 0.5).sin() * (v * 0.8 - 1.2).cos();
    let b = (u * 0.5 - v * 0.9 + 2.).sin();
    smoothstep(0.4, 0.95, 0.5 + 0.3 * a + 0.2 * b)
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn mix(from: u8, to: u8, share: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * share).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_have_light_and_dark_parts() {
        let values: Vec<f32> = (0..200)
            .flat_map(|y| (0..200).map(move |x| shapes(x as f32 * 20., y as f32 * 20.)))
            .collect();
        assert!(values.iter().all(|v| (0. ..=1.).contains(v)));
        let empty = values.iter().filter(|&&v| v == 0.).count();
        assert!(empty > values.len() / 10 && empty < values.len() * 9 / 10);
    }
}

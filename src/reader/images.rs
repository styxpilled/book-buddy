use std::mem;
use std::sync::Arc;
use std::time::Duration;

use book_buddy::book::BlockKind;
use book_buddy::dither::{self, dither_small};
use book_buddy::library::Position;
use fast_image_resize::Resizer;
use gpui::{App, AppContext, Context, RenderImage, Window};
use image::DynamicImage;

use super::{COLUMN_WIDTH, Reader};
use crate::store::Store;
use crate::ui::render_image;
use crate::ui::style;

/// GPUI keeps a decoded image until it is dropped, so only this many stay decoded.
const DECODED_IMAGES: usize = 16;
/// Longest side of a decoded image, in pixels.
const IMAGE_MAX: u32 = 2048;
/// Width of one dot of a dithered image, in pixels of the widest column.
const DITHER_DOT: f32 = 2.;
/// Blocks after the position of an opened book whose images decode at once.
const DECODE_AHEAD: usize = 8;
/// The longest time that an opened book waits for these images.
const IMAGE_WAIT: Duration = Duration::from_millis(300);

pub(super) struct Picture {
    pub(super) bytes: Arc<Vec<u8>>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) decode: Decode,
}

pub(super) enum Decode {
    No,
    Running,
    Done(Arc<RenderImage>),
    Failed,
}

impl Picture {
    /// Sets the picture to not decoded. Returns the image if it was decoded.
    pub(super) fn reset(&mut self) -> Option<Arc<RenderImage>> {
        match mem::replace(&mut self.decode, Decode::No) {
            Decode::Done(image) => Some(image),
            _ => None,
        }
    }
}

pub(super) type Tones = ([u8; 3], [u8; 3]);

pub(super) fn image_tones(cx: &App) -> Option<Tones> {
    let dither = cx.global::<Store>().library.settings.dither_images;
    dither.then(|| style::tones(cx))
}

fn decode_image(bytes: &[u8], tones: Option<Tones>) -> Option<Arc<RenderImage>> {
    let image = image::load_from_memory(bytes).ok()?;
    let (width, height) = (image.width(), image.height());
    let pixels = match tones {
        Some((dark, light)) => {
            let scale = (COLUMN_WIDTH / width as f32).min(1.) / DITHER_DOT;
            let dots = shrink(image, scale).into_rgba8();
            dither_small(
                &dots,
                style::DITHER_CELL,
                &dither::Adjust::NONE,
                dark,
                light,
            )
        }
        None => {
            let scale = (IMAGE_MAX as f32 / width.max(height) as f32).min(1.);
            shrink(image, scale).into_rgba8()
        }
    };
    Some(render_image(pixels))
}

fn shrink(image: DynamicImage, scale: f32) -> DynamicImage {
    if scale >= 1. {
        return image;
    }
    let size = |side: u32| ((side as f32 * scale).round() as u32).max(1);
    let (width, height) = (size(image.width()), size(image.height()));
    let mut small = DynamicImage::new(width, height, image.color());
    match Resizer::new().resize(&image, &mut small, None) {
        Ok(()) => small,
        // A pixel format that the resizer does not know.
        Err(_) => image.thumbnail_exact(width, height),
    }
}

impl Reader {
    pub(super) fn wait_for_images(&mut self, position: Position, cx: &mut Context<Self>) {
        self.waiting = self
            .images_near(position)
            .into_iter()
            .filter(|href| {
                self.pictures
                    .get(href)
                    .is_some_and(|picture| matches!(picture.decode, Decode::Running))
            })
            .collect();
        if !self.waiting.is_empty() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(IMAGE_WAIT).await;
                this.update(cx, |reader, cx| {
                    reader.waiting.clear();
                    reader.show_when_ready(cx);
                })
                .ok();
            })
            .detach();
        }
    }

    /// Starts to decode the images at a position before a render asks for them.
    pub(super) fn decode_near(&mut self, position: Position, cx: &mut Context<Self>) {
        for href in self.images_near(position) {
            self.decode(&href, cx);
        }
    }

    fn images_near(&self, position: Position) -> Vec<String> {
        let Some(start) = self.item_at(position) else {
            return Vec::new();
        };
        let end = self.chapter_end(position.spine).min(start + DECODE_AHEAD);
        self.items[start..end]
            .iter()
            .filter_map(|item| match &item.block.kind {
                BlockKind::Image { href, .. } => Some(href.clone()),
                _ => None,
            })
            .collect()
    }

    pub(super) fn decode(&mut self, href: &str, cx: &mut Context<Self>) {
        let Some(picture) = self.pictures.get_mut(href) else {
            return;
        };
        if !matches!(picture.decode, Decode::No) {
            return;
        }
        picture.decode = Decode::Running;
        let bytes = picture.bytes.clone();
        let href = href.to_string();
        let tones = self.tones;
        cx.spawn(async move |this, cx| {
            let image = cx
                .background_spawn(async move { decode_image(&bytes, tones) })
                .await;
            this.update(cx, |reader, cx| reader.decoded(href, image, tones, cx))
                .ok();
        })
        .detach();
    }

    fn decoded(
        &mut self,
        href: String,
        image: Option<Arc<RenderImage>>,
        tones: Option<Tones>,
        cx: &mut Context<Self>,
    ) {
        self.waiting.retain(|waiting| *waiting != href);
        self.show_when_ready(cx);
        let Some(picture) = self.pictures.get_mut(&href) else {
            return;
        };
        if tones != self.tones {
            // The tones changed during the decode.
            picture.decode = Decode::No;
            self.dropped.extend(image);
            cx.notify();
            return;
        }
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
            if let Some(image) = self.pictures.get_mut(&old).and_then(Picture::reset) {
                self.dropped.push(image);
            }
        }
        cx.notify();
    }

    pub(super) fn drop_images(&mut self, window: &mut Window) {
        for image in self.dropped.drain(..) {
            window.drop_image(image).ok();
        }
    }
}

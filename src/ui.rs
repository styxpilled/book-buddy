use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};

pub(crate) mod autoscroll;
pub(crate) mod menu;
pub(crate) mod style;
pub(crate) mod title_bar;

pub(crate) fn render_image(mut pixels: RgbaImage) -> Arc<RenderImage> {
    // RenderImage is BGRA.
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![Frame::new(pixels)]))
}

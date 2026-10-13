use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

const BAYER: [[u8; 8]; 8] = bayer();

/// The 8x8 Bayer threshold map, values 0 to 63.
const fn bayer() -> [[u8; 8]; 8] {
    let mut map = [[0u8; 8]; 8];
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let mut value = 0;
            let mut bit = 0;
            while bit < 3 {
                let xb = (x >> bit) & 1;
                let yb = (y >> bit) & 1;
                value = (value << 2) | ((xb ^ yb) << 1) | yb;
                bit += 1;
            }
            map[y][x] = value as u8;
            x += 1;
        }
        y += 1;
    }
    map
}

/// Ordered dithering to two tones. Each dot is a square of `cell` pixels.
/// Transparent parts count as light.
pub fn dither(image: &RgbaImage, cell: u32, dark: [u8; 3], light: [u8; 3]) -> RgbaImage {
    let cell = cell.max(1);
    let (width, height) = (
        (image.width() / cell).max(1),
        (image.height() / cell).max(1),
    );
    let small = imageops::resize(image, width, height, FilterType::Triangle);
    let mut out = RgbaImage::new(width * cell, height * cell);
    for (x, y, pixel) in small.enumerate_pixels() {
        let [r, g, b, a] = pixel.0.map(|c| c as f32 / 255.);
        let luma = (0.2126 * r + 0.7152 * g + 0.0722 * b) * a + (1. - a);
        let threshold = (BAYER[(y % 8) as usize][(x % 8) as usize] as f32 + 0.5) / 64.;
        let [r, g, b] = if luma > threshold { light } else { dark };
        for dy in 0..cell {
            for dx in 0..cell {
                out.put_pixel(x * cell + dx, y * cell + dy, Rgba([r, g, b, 255]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DARK: [u8; 3] = [0, 0, 0];
    const LIGHT: [u8; 3] = [255, 255, 255];

    fn dark_share(gray: u8) -> f32 {
        let image = RgbaImage::from_pixel(64, 64, Rgba([gray, gray, gray, 255]));
        let out = dither(&image, 2, DARK, LIGHT);
        let dark = out.pixels().filter(|p| p.0[0] == 0).count();
        dark as f32 / (out.width() * out.height()) as f32
    }

    #[test]
    fn map_holds_each_value_once() {
        let mut seen = [false; 64];
        for row in BAYER {
            for value in row {
                seen[value as usize] = true;
            }
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn tones_follow_brightness() {
        assert_eq!(dark_share(0), 1.);
        assert_eq!(dark_share(255), 0.);
        assert!((dark_share(128) - 0.5).abs() < 0.05);
    }

    #[test]
    fn cells_are_square_blocks() {
        let image = RgbaImage::from_pixel(10, 7, Rgba([128, 128, 128, 255]));
        let out = dither(&image, 3, DARK, LIGHT);
        assert_eq!(out.dimensions(), (9, 6));
        for y in 0..6 {
            for x in 0..9 {
                assert_eq!(out.get_pixel(x, y), out.get_pixel(x / 3 * 3, y / 3 * 3));
            }
        }
    }

    #[test]
    fn transparent_is_light() {
        let image = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        let out = dither(&image, 1, DARK, LIGHT);
        assert!(out.pixels().all(|p| p.0[0] == 255));
    }
}

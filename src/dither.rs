use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

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

pub const ADJUST_RANGE: i32 = 100;

/// The centers of the hues of `Adjust::hues`, in degrees: red, orange,
/// yellow, green, aqua, blue, purple, magenta.
const HUE_CENTERS: [f32; 8] = [0., 30., 60., 120., 180., 240., 270., 300.];

/// Changes to the tones of an image before it is dithered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjust {
    /// The input tone that becomes full dark, from 0 to `ADJUST_RANGE`.
    pub black: i32,
    /// The input tone that becomes full light, from 0 to `ADJUST_RANGE`.
    pub white: i32,
    /// The values below go from `-ADJUST_RANGE` to `ADJUST_RANGE`, and 0 makes
    /// no change. At the ends, a value multiplies or divides by 4: the gamma,
    /// the distance from mid gray, or the luma of a saturated hue.
    pub midtones: i32,
    pub contrast: i32,
    /// In the order of `HUE_CENTERS`.
    pub hues: [i32; 8],
    pub method: Method,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    /// Ordered dithering: a regular pattern.
    #[default]
    Bayer,
    /// Error diffusion that drops a quarter of the error: clean highlights
    /// and sharp edges.
    Atkinson,
    /// Error diffusion that keeps all of the error: the most tonal detail.
    FloydSteinberg,
}

impl Default for Adjust {
    fn default() -> Self {
        Self::NONE
    }
}

impl Adjust {
    pub const NONE: Adjust = Adjust {
        black: 0,
        white: ADJUST_RANGE,
        midtones: 0,
        contrast: 0,
        hues: [0; 8],
        method: Method::Bayer,
    };

    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }

    /// The tone of a color, from 0 to 1.
    fn tone(&self) -> impl Fn([f32; 3]) -> f32 {
        let factor = |value| 4f32.powf(value as f32 / ADJUST_RANGE as f32);
        let luma_of = self.luma();
        let black = self.black as f32 / ADJUST_RANGE as f32;
        let white = self.white as f32 / ADJUST_RANGE as f32;
        let span = (white - black).max(0.01);
        let gamma = factor(-self.midtones);
        let contrast = factor(self.contrast);
        move |rgb| {
            let leveled = ((luma_of(rgb) - black) / span).clamp(0., 1.).powf(gamma);
            ((leveled - 0.5) * contrast + 0.5).clamp(0., 1.)
        }
    }

    /// The luma of a color after the hue sliders, before the levels.
    fn luma(&self) -> impl Fn([f32; 3]) -> f32 {
        let hues = (self.hues != [0; 8]).then_some(self.hues);
        move |rgb| {
            let [r, g, b] = rgb;
            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            match hues {
                Some(hues) => {
                    let (hue, chroma) = hue_and_chroma(rgb);
                    luma * 4f32.powf(hue_value(&hues, hue) / ADJUST_RANGE as f32 * chroma)
                }
                None => luma,
            }
        }
    }

    /// Black and white points at the darkest and lightest tones of an image.
    /// The darkest and lightest 1% of the pixels are ignored, so that a few
    /// specks do not decide. Transparent pixels are ignored too.
    pub fn auto_levels(&self, image: &RgbaImage) -> (i32, i32) {
        let luma_of = self.luma();
        let mut lumas: Vec<f32> = image
            .pixels()
            .filter(|pixel| pixel.0[3] >= 128)
            .map(|pixel| {
                let [r, g, b, _] = pixel.0.map(|c| c as f32 / 255.);
                luma_of([r, g, b])
            })
            .collect();
        if lumas.is_empty() {
            return (Self::NONE.black, Self::NONE.white);
        }
        lumas.sort_by(f32::total_cmp);
        let at = |share: f32| lumas[((lumas.len() - 1) as f32 * share).round() as usize];
        let range = ADJUST_RANGE as f32;
        let black = ((at(0.01) * range).floor() as i32).clamp(0, ADJUST_RANGE - 1);
        let white = ((at(0.99) * range).ceil() as i32).clamp(black + 1, ADJUST_RANGE);
        (black, white)
    }
}

/// The hue in degrees, and the chroma from 0 to 1.
fn hue_and_chroma([r, g, b]: [f32; 3]) -> (f32, f32) {
    let max = r.max(g).max(b);
    let chroma = max - r.min(g).min(b);
    if chroma == 0. {
        return (0., 0.);
    }
    let sector = if max == r {
        ((g - b) / chroma).rem_euclid(6.)
    } else if max == g {
        (b - r) / chroma + 2.
    } else {
        (r - g) / chroma + 4.
    };
    (sector * 60., chroma)
}

/// The slider value at a hue, linear between the two nearest centers.
fn hue_value(values: &[i32; 8], hue: f32) -> f32 {
    let next = HUE_CENTERS.iter().position(|&c| c > hue).unwrap_or(8);
    let ix = next - 1;
    let from = HUE_CENTERS[ix];
    let to = HUE_CENTERS.get(next).copied().unwrap_or(360.);
    let t = (hue - from) / (to - from);
    values[ix] as f32 * (1. - t) + values[next % 8] as f32 * t
}

/// Dithering to two tones. Each dot is a square of `cell` pixels.
/// Transparent parts count as light.
pub fn dither(
    image: &RgbaImage,
    cell: u32,
    adjust: &Adjust,
    dark: [u8; 3],
    light: [u8; 3],
) -> RgbaImage {
    let small = dots(image, cell);
    dither_small(&small, cell, adjust, dark, light)
}

/// The image made smaller to one pixel for each dot of `cell` pixels.
pub fn dots(image: &RgbaImage, cell: u32) -> RgbaImage {
    let cell = cell.max(1);
    let (width, height) = (
        (image.width() / cell).max(1),
        (image.height() / cell).max(1),
    );
    imageops::resize(image, width, height, FilterType::Triangle)
}

/// Dithering of an image that has one pixel for each dot.
pub fn dither_small(
    small: &RgbaImage,
    cell: u32,
    adjust: &Adjust,
    dark: [u8; 3],
    light: [u8; 3],
) -> RgbaImage {
    let cell = cell.max(1);
    let tone_of = adjust.tone();
    let mut tones: Vec<f32> = small
        .pixels()
        .map(|pixel| {
            let [r, g, b, a] = pixel.0.map(|c| c as f32 / 255.);
            tone_of([r, g, b]) * a + (1. - a)
        })
        .collect();
    let (width, height) = (small.width() as usize, small.height() as usize);
    let light_dots = match adjust.method {
        Method::Bayer => bayer_dots(&tones, width),
        Method::Atkinson => diffuse(&mut tones, width, height, ATKINSON),
        Method::FloydSteinberg => diffuse(&mut tones, width, height, FLOYD_STEINBERG),
    };
    let mut out = RgbaImage::new(small.width() * cell, small.height() * cell);
    for (ix, is_light) in light_dots.into_iter().enumerate() {
        let (x, y) = ((ix % width) as u32, (ix / width) as u32);
        let [r, g, b] = if is_light { light } else { dark };
        for dy in 0..cell {
            for dx in 0..cell {
                out.put_pixel(x * cell + dx, y * cell + dy, Rgba([r, g, b, 255]));
            }
        }
    }
    out
}

fn bayer_dots(tones: &[f32], width: usize) -> Vec<bool> {
    tones
        .iter()
        .enumerate()
        .map(|(ix, &tone)| {
            let (x, y) = (ix % width, ix / width);
            tone > (BAYER[y % 8][x % 8] as f32 + 0.5) / 64.
        })
        .collect()
}

/// Where the error of a dot goes: the offset to x and y, and the share.
type Kernel = &'static [(isize, usize, f32)];

const ATKINSON: Kernel = &[
    (1, 0, 1. / 8.),
    (2, 0, 1. / 8.),
    (-1, 1, 1. / 8.),
    (0, 1, 1. / 8.),
    (1, 1, 1. / 8.),
    (0, 2, 1. / 8.),
];

const FLOYD_STEINBERG: Kernel = &[
    (1, 0, 7. / 16.),
    (-1, 1, 3. / 16.),
    (0, 1, 5. / 16.),
    (1, 1, 1. / 16.),
];

/// Error diffusion. It changes `tones`.
fn diffuse(tones: &mut [f32], width: usize, height: usize, kernel: Kernel) -> Vec<bool> {
    let mut dots = Vec::with_capacity(tones.len());
    for y in 0..height {
        for x in 0..width {
            let tone = tones[y * width + x];
            let is_light = tone > 0.5;
            let error = tone - if is_light { 1. } else { 0. };
            for &(dx, dy, share) in kernel {
                let (nx, ny) = (x as isize + dx, y + dy);
                if nx >= 0 && (nx as usize) < width && ny < height {
                    tones[ny * width + nx as usize] += error * share;
                }
            }
            dots.push(is_light);
        }
    }
    dots
}

#[cfg(test)]
mod tests {
    use super::*;

    const DARK: [u8; 3] = [0, 0, 0];
    const LIGHT: [u8; 3] = [255, 255, 255];
    /// `dither_small` output for each method, adjustment and cell in
    /// `output_is_unchanged`, recorded at 2eded73.
    const OUTPUT_HASHES: [u64; 24] = [
        0xd2e952461d4c4268,
        0x2b779e17df5c6075,
        0x55f38971f4ae8440,
        0xf5c395f05ede8b65,
        0x1e76459170e3614a,
        0xab6d8006832e8425,
        0x64b525f39f1351aa,
        0x7d6a932615e7f725,
        0x927af4af49ede132,
        0x4be8ef2fc833ba45,
        0xd1a93ad5590c6352,
        0xd3bd0ea384d303a5,
        0xc06fd45684402d3a,
        0x93dd6d34af227485,
        0x6883d505988b0222,
        0x84a82c2b056f67a5,
        0x1f3c7a1a8055f188,
        0xffc53efebd068c75,
        0xbad52150825c5fb0,
        0xd3ec193f1bb64165,
        0x156f277292247afa,
        0xe176fe125f8ef9a5,
        0x68dad190212036aa,
        0xc99b78d50e75b25,
    ];

    fn dark_share(gray: u8) -> f32 {
        adjusted_dark_share([gray; 3], Adjust::NONE)
    }

    fn adjusted_dark_share([r, g, b]: [u8; 3], adjust: Adjust) -> f32 {
        let image = RgbaImage::from_pixel(64, 64, Rgba([r, g, b, 255]));
        let out = dither(&image, 2, &adjust, DARK, LIGHT);
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
        let out = dither(&image, 3, &Adjust::NONE, DARK, LIGHT);
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
        let out = dither(&image, 1, &Adjust::NONE, DARK, LIGHT);
        assert!(out.pixels().all(|p| p.0[0] == 255));
    }

    #[test]
    fn adjust_changes_the_tones() {
        let share = adjusted_dark_share;
        let none = Adjust::NONE;
        let gray = [128; 3];
        let base = share(gray, none);
        assert!(
            share(
                gray,
                Adjust {
                    midtones: 50,
                    ..none
                }
            ) < base
        );
        assert!(share(gray, Adjust { black: 30, ..none }) > base);
        assert!(share(gray, Adjust { white: 70, ..none }) < base);
        assert_eq!(share([60; 3], Adjust { black: 30, ..none }), 1.);
        assert_eq!(share([200; 3], Adjust { white: 70, ..none }), 0.);

        let dark_gray = [80; 3];
        let more_contrast = Adjust {
            contrast: 100,
            ..none
        };
        assert!(share(dark_gray, more_contrast) > share(dark_gray, none));
    }

    #[test]
    fn auto_levels_find_the_range() {
        let mut image = RgbaImage::from_fn(100, 100, |x, _| {
            let gray = (50 + x) as u8;
            Rgba([gray, gray, gray, 255])
        });
        image.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        image.put_pixel(1, 0, Rgba([255, 255, 255, 0]));
        assert_eq!(Adjust::NONE.auto_levels(&image), (20, 59));

        let flat = RgbaImage::from_pixel(4, 4, Rgba([255; 4]));
        assert_eq!(Adjust::NONE.auto_levels(&flat), (99, 100));
    }

    #[test]
    fn hue_sliders_change_only_their_colors() {
        let share = adjusted_dark_share;
        let none = Adjust::NONE;
        let mut lighter_red = none;
        lighter_red.hues[0] = 100;
        let red = [200, 0, 0];
        assert!(share(red, lighter_red) < share(red, none));
        let blue = [0, 0, 200];
        assert_eq!(share(blue, lighter_red), share(blue, none));
        let gray = [128; 3];
        assert_eq!(share(gray, lighter_red), share(gray, none));
    }

    #[test]
    fn hue_values_between_centers() {
        let mut values = [0; 8];
        values[0] = 100;
        values[1] = 50;
        assert_eq!(hue_value(&values, 0.), 100.);
        assert_eq!(hue_value(&values, 15.), 75.);
        assert_eq!(hue_value(&values, 330.), 50.);
        assert_eq!(hue_and_chroma([0., 0., 1.]), (240., 1.));
        assert_eq!(hue_and_chroma([1., 0., 1.]), (300., 1.));
    }

    #[test]
    fn diffusion_follows_brightness() {
        for method in [Method::Atkinson, Method::FloydSteinberg] {
            let adjust = Adjust {
                method,
                ..Adjust::NONE
            };
            assert_eq!(adjusted_dark_share([0; 3], adjust), 1.);
            assert_eq!(adjusted_dark_share([255; 3], adjust), 0.);
            assert!((adjusted_dark_share([128; 3], adjust) - 0.5).abs() < 0.05);
        }
    }

    #[test]
    fn output_is_unchanged() {
        let image = RgbaImage::from_fn(37, 23, |x, y| {
            let alpha = if (x + y) % 9 == 0 { 0 } else { 255 };
            Rgba([(x * 7) as u8, (y * 11) as u8, ((x + y) * 5) as u8, alpha])
        });
        let adjusted = Adjust {
            black: 10,
            white: 90,
            midtones: 30,
            contrast: -20,
            hues: [40, 0, 0, -30, 0, 0, 0, 0],
            ..Adjust::NONE
        };
        let mut hashes = Vec::new();
        for method in [Method::Bayer, Method::Atkinson, Method::FloydSteinberg] {
            for base in [Adjust::NONE, adjusted] {
                for cell in 1..=4 {
                    let adjust = Adjust { method, ..base };
                    let out = dither_small(&image, cell, &adjust, [10, 20, 30], [200, 210, 220]);
                    assert_eq!(out.dimensions(), (37 * cell, 23 * cell));
                    let hash = out.as_raw().iter().fold(0xcbf29ce484222325u64, |hash, &b| {
                        (hash ^ b as u64).wrapping_mul(0x100000001b3)
                    });
                    hashes.push(hash);
                }
            }
        }
        assert_eq!(hashes, OUTPUT_HASHES, "{hashes:#x?}");
    }
}

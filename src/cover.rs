//! Covers made from text, for books without a cover image. The background is
//! lightly dithered to two tones, and the text is solid dots.

use std::f32::consts::TAU;
use std::ops::RangeInclusive;

use image::{Rgba, RgbaImage};
use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
use rustybuzz::{Face, UnicodeBuffer};
use serde::{Deserialize, Serialize};
use tiny_skia::{FillRule, Mask, Path, PathBuilder, Transform};

use crate::dither::{Adjust, dither_small};
use crate::fonts;

/// The size in dots.
pub const WIDTH: u32 = 80;
pub const HEIGHT: u32 = 120;
/// The em sizes in dots that the user can pick for the text.
pub const SIZES: RangeInclusive<u32> = 8..=32;
/// The widths in dots that the user can pick for the outline.
pub const OUTLINES: [u32; 3] = [0, 1, 2];

/// Space between the text and the edge, when the cover has padding.
const MARGIN: f32 = 6.;
const FRAME_MARGIN: f32 = 7.;
/// The frame line is this many dots inside the edge.
const FRAME_INSET: u32 = 3;
const LINE_HEIGHT: f32 = 1.1;
const AUTO_MAX_SIZE: f32 = 20.;
const MIN_SIZE: f32 = 6.;
const AUTHOR_SIZE: f32 = 9.;
const AUTHOR_MIN_SIZE: f32 = 7.;
const AUTHOR_LINES: f32 = 2.;
const AUTHOR_GAP: f32 = 4.;
/// The band is this many dots taller than the text at each side.
const BAND_PAD: f32 = 4.;
/// A dot with at least this coverage is part of the text.
const COVERAGE: u8 = 128;
const BLACK: [u8; 3] = [0; 3];
const WHITE: [u8; 3] = [255; 3];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextCover {
    /// Lines of the text. Empty uses the title.
    pub text: String,
    pub font: CoverFont,
    pub bold: bool,
    pub italic: bool,
    /// The em size in dots, when `auto_size` is off.
    pub size: u32,
    /// The text has the largest size at which it fits.
    pub auto_size: bool,
    pub align: Align,
    pub author: bool,
    pub background: Background,
    /// Light text and pattern on a dark background.
    pub inverted: bool,
    /// Space between the text and the edges of the cover.
    pub padding: bool,
    /// The width in dots of a line around the text, in the opposite tone.
    pub outline: u32,
    /// Changes the shapes of the background.
    pub seed: u32,
}

impl Default for TextCover {
    fn default() -> Self {
        Self {
            text: String::new(),
            font: CoverFont::Garamond,
            bold: true,
            italic: false,
            size: 16,
            auto_size: true,
            align: Align::Center,
            author: true,
            background: Background::Gradient,
            inverted: false,
            padding: true,
            outline: 0,
            seed: 0,
        }
    }
}

impl TextCover {
    /// The defaults, with a seed from the book ID.
    pub fn for_book(id: &str) -> Self {
        Self {
            seed: seed(id),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverFont {
    Literata,
    Newsreader,
    SourceSerif,
    Alegreya,
    Garamond,
    Inter,
    PlexMono,
    /// It has no bold and no italic.
    DotGothic,
}

impl CoverFont {
    pub const ALL: [CoverFont; 8] = [
        CoverFont::Literata,
        CoverFont::Newsreader,
        CoverFont::SourceSerif,
        CoverFont::Alegreya,
        CoverFont::Garamond,
        CoverFont::Inter,
        CoverFont::PlexMono,
        CoverFont::DotGothic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CoverFont::Literata => "LITERATA",
            CoverFont::Newsreader => "NEWSREADER",
            CoverFont::SourceSerif => "SOURCE SERIF",
            CoverFont::Alegreya => "ALEGREYA",
            CoverFont::Garamond => "EB GARAMOND",
            CoverFont::Inter => "INTER",
            CoverFont::PlexMono => "PLEX MONO",
            CoverFont::DotGothic => "DOTGOTHIC16",
        }
    }

    pub fn has_styles(self) -> bool {
        self != CoverFont::DotGothic
    }

    fn file(self, bold: bool, italic: bool) -> &'static [u8] {
        let family = match self {
            CoverFont::Literata => &fonts::LITERATA,
            CoverFont::Newsreader => &fonts::NEWSREADER,
            CoverFont::SourceSerif => &fonts::SOURCE_SERIF,
            CoverFont::Alegreya => &fonts::ALEGREYA,
            CoverFont::Garamond => &fonts::GARAMOND,
            CoverFont::Inter => &fonts::INTER,
            CoverFont::PlexMono => &fonts::PLEX_MONO,
            CoverFont::DotGothic => return fonts::DOT_GOTHIC,
        };
        family.style(bold, italic)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    Left,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Background {
    /// Darker along a direction.
    Gradient,
    /// Soft shapes.
    Noise,
    /// A band behind the title.
    Band,
    /// A line near the edge.
    Frame,
}

impl Background {
    pub const ALL: [Background; 4] = [
        Background::Gradient,
        Background::Noise,
        Background::Band,
        Background::Frame,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Background::Gradient => "GRADIENT",
            Background::Noise => "NOISE",
            Background::Band => "BAND",
            Background::Frame => "FRAME",
        }
    }
}

/// The cover, `cell` pixels for each dot. Black is the dark tone and white
/// the light tone.
pub fn render(cover: &TextCover, title: &str, author: &str, cell: u32) -> RgbaImage {
    let cell = cell.max(1);
    let (path, band) = layout(cover, title, author);
    let mut dots = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let tone = ((1. - background(cover, x, y, band)) * 255.).round() as u8;
        Rgba([tone, tone, tone, 255])
    });
    if let Some(path) = &path {
        let text = text_dots(path);
        let near = near_text(&text, cover.outline);
        for (ix, pixel) in dots.pixels_mut().enumerate() {
            if text[ix] {
                pixel.0 = [0, 0, 0, 255];
            } else if near[ix] {
                pixel.0 = [255; 4];
            }
        }
    }
    let mut image = dither_small(&dots, cell, &Adjust::NONE, BLACK, WHITE);
    if cover.inverted {
        for pixel in image.pixels_mut() {
            for c in &mut pixel.0[..3] {
                *c = 255 - *c;
            }
        }
    }
    image
}

/// A cover from `render` in two colors.
pub fn colorize(image: &RgbaImage, dark: [u8; 3], light: [u8; 3]) -> RgbaImage {
    let mut out = image.clone();
    for pixel in out.pixels_mut() {
        let t = pixel.0[0] as u32;
        for (i, c) in pixel.0[..3].iter_mut().enumerate() {
            *c = ((dark[i] as u32 * (255 - t) + light[i] as u32 * t + 127) / 255) as u8;
        }
        pixel.0[3] = 255;
    }
    out
}

/// The outline of all text in dots, and the top and bottom of the band.
fn layout(cover: &TextCover, title: &str, author: &str) -> (Option<Path>, (f32, f32)) {
    let face = Face::from_slice(cover.font.file(cover.bold, cover.italic), 0)
        .expect("bundled fonts must parse");
    let margin = match (cover.padding, cover.background) {
        (false, _) => 0.,
        (true, Background::Frame) => FRAME_MARGIN,
        (true, _) => MARGIN,
    };
    let width = WIDTH as f32 - 2. * margin;
    let mut bottom = HEIGHT as f32 - margin;
    let mut path = PathBuilder::new();

    let author = author.trim();
    if cover.author && !author.is_empty() {
        let height = AUTHOR_LINES * LINE_HEIGHT * AUTHOR_SIZE;
        let block = fit(&face, author, width, height, AUTHOR_SIZE, AUTHOR_MIN_SIZE);
        let top = bottom - block.height(&face);
        block.draw(&face, &mut path, margin, top, width, cover.align);
        bottom = top - AUTHOR_GAP;
    }

    let text = if cover.text.trim().is_empty() {
        title.trim()
    } else {
        cover.text.trim_end()
    };
    let height = bottom - margin;
    let block = if cover.auto_size {
        fit(&face, text, width, height, AUTO_MAX_SIZE, MIN_SIZE)
    } else {
        wrap(&face, text, width, cover.size as f32)
    };
    let top = margin + (height - block.height(&face)).max(0.) / 2.;
    block.draw(&face, &mut path, margin, top, width, cover.align);
    let band = (top - BAND_PAD, top + block.height(&face) + BAND_PAD);
    (path.finish(), band)
}

/// The dots that the text covers.
fn text_dots(path: &Path) -> Vec<bool> {
    let mut mask = Mask::new(WIDTH, HEIGHT).expect("the cover has a size");
    mask.fill_path(path, FillRule::Winding, true, Transform::identity());
    mask.data().iter().map(|&c| c >= COVERAGE).collect()
}

/// The dots that are not text and are at most `radius` dots from text.
fn near_text(text: &[bool], radius: u32) -> Vec<bool> {
    let r = radius as i32;
    let (width, height) = (WIDTH as i32, HEIGHT as i32);
    let is_text = |x: i32, y: i32| {
        (0..width).contains(&x) && (0..height).contains(&y) && text[(y * width + x) as usize]
    };
    (0..text.len() as i32)
        .map(|ix| {
            let (x, y) = (ix % width, ix / width);
            r > 0
                && !text[ix as usize]
                && (-r..=r).any(|dy| {
                    (-r..=r).any(|dx| dx * dx + dy * dy <= r * r + r && is_text(x + dx, y + dy))
                })
        })
        .collect()
}

/// Lines of text at one size.
struct Block {
    size: f32,
    lines: Vec<Line>,
}

/// Glyphs and their positions in font units.
struct Line {
    glyphs: Vec<(GlyphId, f32, f32)>,
    width: f32,
}

impl Block {
    fn scale(&self, face: &Face) -> f32 {
        self.size / face.units_per_em() as f32
    }

    /// From the top of the first line to the bottom of the last.
    fn height(&self, face: &Face) -> f32 {
        let lines = self.lines.len().max(1) as f32;
        let extent = (face.ascender() - face.descender()) as f32 * self.scale(face);
        (lines - 1.) * self.size * LINE_HEIGHT + extent
    }

    fn draw(
        &self,
        face: &Face,
        path: &mut PathBuilder,
        left: f32,
        top: f32,
        width: f32,
        align: Align,
    ) {
        let scale = self.scale(face);
        let mut baseline = top + face.ascender() as f32 * scale;
        for line in &self.lines {
            let x = match align {
                Align::Left => left,
                Align::Center => left + (width - line.width * scale) / 2.,
            };
            for &(glyph, gx, gy) in &line.glyphs {
                let mut outline = Outline {
                    path,
                    x: x + gx * scale,
                    y: baseline - gy * scale,
                    scale,
                };
                face.outline_glyph(glyph, &mut outline);
            }
            baseline += self.size * LINE_HEIGHT;
        }
    }
}

/// The largest size from `max` down to `min` at which the words fit `width`
/// and the lines fit `height`. At `min` the text can overflow.
fn fit(face: &Face, text: &str, width: f32, height: f32, max: f32, min: f32) -> Block {
    let mut size = max;
    loop {
        let block = wrap(face, text, width, size);
        let fits = block.height(face) <= height
            && block
                .lines
                .iter()
                .all(|line| line.width * block.scale(face) <= width);
        if fits || size <= min {
            return block;
        }
        size = (size - 0.5).max(min);
    }
}

/// Keeps the line breaks of `text` and wraps long lines at spaces.
fn wrap(face: &Face, text: &str, width: f32, size: f32) -> Block {
    let max = width / size * face.units_per_em() as f32;
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let next = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if !current.is_empty() && shape(face, &next).width > max {
                lines.push(shape(face, &current));
                current = word.to_string();
            } else {
                current = next;
            }
        }
        lines.push(shape(face, &current));
    }
    Block { size, lines }
}

fn shape(face: &Face, text: &str) -> Line {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    let shaped = rustybuzz::shape(face, &[], buffer);
    let mut x = 0.;
    let mut glyphs = Vec::with_capacity(shaped.len());
    for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        glyphs.push((
            GlyphId(info.glyph_id as u16),
            x + pos.x_offset as f32,
            pos.y_offset as f32,
        ));
        x += pos.x_advance as f32;
    }
    Line { glyphs, width: x }
}

/// Adds a glyph outline to a path, in dots. Font units go up, dots go down.
struct Outline<'a> {
    path: &'a mut PathBuilder,
    x: f32,
    y: f32,
    scale: f32,
}

impl Outline<'_> {
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y - y * self.scale)
    }
}

impl OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.at(x, y);
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.at(x, y);
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let ((x1, y1), (x, y)) = (self.at(x1, y1), self.at(x, y));
        self.path.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let ((x1, y1), (x2, y2), (x, y)) = (self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        self.path.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

/// The share of dark dots in the background at a dot, from 0 to 1. `band` is
/// the top and bottom of the band in dots.
fn background(cover: &TextCover, x: u32, y: u32, band: (f32, f32)) -> f32 {
    let seed = cover.seed;
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    match cover.background {
        Background::Gradient => {
            let angle = random(seed, 0) * TAU;
            let (dx, dy) = (angle.cos(), angle.sin());
            let extent = dx.abs() * WIDTH as f32 + dy.abs() * HEIGHT as f32;
            let along = (fx - WIDTH as f32 / 2.) * dx + (fy - HEIGHT as f32 / 2.) * dy;
            0.02 + 0.26 * (along / extent + 0.5).clamp(0., 1.)
        }
        Background::Noise => {
            let n =
                0.65 * noise(seed, fx / 22., fy / 22.) + 0.35 * noise(seed ^ 1, fx / 9., fy / 9.);
            0.02 + 0.28 * smoothstep(0.35, 0.75, n)
        }
        Background::Band => {
            if (band.0..band.1).contains(&fy) {
                0.22
            } else {
                0.03
            }
        }
        Background::Frame => {
            let (right, bottom) = (WIDTH - 1 - FRAME_INSET, HEIGHT - 1 - FRAME_INSET);
            let inside = (FRAME_INSET..=right).contains(&x) && (FRAME_INSET..=bottom).contains(&y);
            let on_line = x == FRAME_INSET || x == right || y == FRAME_INSET || y == bottom;
            if inside && on_line { 1. } else { 0.05 }
        }
    }
}

/// Smooth value noise, from 0 to 1.
fn noise(seed: u32, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (smoothstep(0., 1., x - x0), smoothstep(0., 1., y - y0));
    let corner = |dx: i32, dy: i32| {
        let (cx, cy) = (x0 as i32 + dx, y0 as i32 + dy);
        random(
            seed,
            (cx as u32).wrapping_mul(0x9e37) ^ (cy as u32).wrapping_mul(0x85eb_ca6b),
        )
    };
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * tx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * tx;
    top + (bottom - top) * ty
}

/// A number from 0 to 1 that depends only on `seed` and `key`.
fn random(seed: u32, key: u32) -> f32 {
    let mut h = seed ^ key.wrapping_mul(0x27d4_eb2d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// FNV-1a of the ID.
fn seed(id: &str) -> u32 {
    id.bytes().fold(0x811c_9dc5, |hash, b| {
        (hash ^ b as u32).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cover() -> TextCover {
        TextCover {
            author: false,
            ..TextCover::for_book("fnv1a:0123456789abcdef")
        }
    }

    fn text(cover: &TextCover, title: &str, author: &str) -> Vec<bool> {
        text_dots(&layout(cover, title, author).0.unwrap())
    }

    /// Rows of dots with text, with empty rows between them.
    fn text_rows(text: &[bool]) -> usize {
        let rows: Vec<bool> = text
            .chunks(WIDTH as usize)
            .map(|row| row.contains(&true))
            .collect();
        rows.windows(2).filter(|w| w[1] && !w[0]).count() + rows[0] as usize
    }

    fn tones(image: &RgbaImage) -> Vec<u8> {
        image.pixels().map(|p| p.0[0]).collect()
    }

    #[test]
    fn text_dots_are_dark_and_the_background_is_light() {
        for background in Background::ALL {
            let cover = TextCover {
                background,
                ..cover()
            };
            let text = text(&cover, "Middlemarch", "");
            assert!(text.iter().filter(|&&t| t).count() > 50, "{background:?}");
            let tones = tones(&render(&cover, "Middlemarch", "", 1));
            let others: Vec<usize> = (0..text.len()).filter(|&ix| !text[ix]).collect();
            assert!(
                (0..text.len())
                    .filter(|&ix| text[ix])
                    .all(|ix| tones[ix] == 0)
            );
            let dark = others.iter().filter(|&&ix| tones[ix] == 0).count();
            assert!(dark * 10 < others.len() * 3, "{background:?}");
        }
    }

    #[test]
    fn long_text_stays_inside_the_margins() {
        let title = "The Life and Strange Surprizing Adventures of Robinson Crusoe, of York, \
                     Mariner: Who Lived Eight and Twenty Years, All Alone";
        let text = text(&TextCover::default(), title, "Daniel Defoe");
        let margin = MARGIN as u32 - 1;
        for ix in (0..text.len()).filter(|&ix| text[ix]) {
            let (x, y) = (ix as u32 % WIDTH, ix as u32 / WIDTH);
            assert!((margin..WIDTH - margin).contains(&x), "{x}");
            assert!((margin..HEIGHT - margin).contains(&y), "{y}");
        }
    }

    #[test]
    fn every_size_changes_the_text() {
        let title = "The Picture of Dorian Gray";
        let at = |size| {
            let cover = TextCover {
                size,
                auto_size: false,
                ..cover()
            };
            text(&cover, title, "")
        };
        for size in *SIZES.start()..*SIZES.end() {
            assert_ne!(at(size), at(size + 1), "{size}");
        }
    }

    #[test]
    fn line_breaks_stay() {
        let cover = |text: &str| TextCover {
            text: text.into(),
            size: 12,
            auto_size: false,
            ..cover()
        };
        assert_eq!(text_rows(&text(&cover("EMMA"), "", "")), 1);
        assert_eq!(text_rows(&text(&cover("EM\n\nMA"), "", "")), 2);
    }

    #[test]
    fn empty_text_is_the_title() {
        let mut cover = TextCover::default();
        let title = render(&cover, "Emma", "Jane Austen", 2);
        cover.text = "Emma".into();
        assert_eq!(render(&cover, "Other", "Jane Austen", 2), title);
        cover.author = false;
        assert_ne!(render(&cover, "Other", "Jane Austen", 2), title);
    }

    #[test]
    fn the_seed_changes_only_the_background() {
        let cover = cover();
        let other = TextCover {
            seed: cover.seed + 1,
            ..cover.clone()
        };
        let a = render(&cover, "Emma", "", 1);
        assert_ne!(a, render(&other, "Emma", "", 1));
        assert_eq!(text(&cover, "Emma", ""), text(&other, "Emma", ""));
        assert_eq!(render(&cover, "Emma", "", 1), a);
    }

    #[test]
    fn outline_dots_are_light() {
        let cover = TextCover {
            outline: 1,
            ..cover()
        };
        let text = text(&cover, "Emma", "");
        let tones = tones(&render(&cover, "Emma", "", 1));
        let near = near_text(&text, 1);
        assert!(near.contains(&true));
        assert!(
            (0..near.len())
                .filter(|&ix| near[ix])
                .all(|ix| tones[ix] == 255)
        );
        let count = |near: Vec<bool>| near.into_iter().filter(|&n| n).count();
        assert!(count(near_text(&text, 2)) > count(near));
    }

    #[test]
    fn inverted_swaps_the_tones() {
        let cover = TextCover {
            outline: 1,
            ..cover()
        };
        let inverted = TextCover {
            inverted: true,
            ..cover.clone()
        };
        let a = tones(&render(&cover, "Emma", "", 3));
        let b = tones(&render(&inverted, "Emma", "", 3));
        assert!(a.iter().zip(&b).all(|(a, b)| *a == 255 - *b));
        assert!(a.iter().all(|&t| t == 0 || t == 255));
    }

    #[test]
    fn without_padding_text_goes_to_the_edges() {
        let left = |padding| {
            let cover = TextCover {
                padding,
                align: Align::Left,
                ..cover()
            };
            let text = text(&cover, "Middlemarch", "");
            (0..text.len())
                .filter(|&ix| text[ix])
                .map(|ix| ix as u32 % WIDTH)
                .min()
                .unwrap()
        };
        assert!(left(true) >= MARGIN as u32 - 1);
        assert!(left(false) < 2);
    }

    #[test]
    fn colorize_maps_black_and_white() {
        let image = RgbaImage::from_fn(3, 1, |x, _| Rgba([[0, 255, 128][x as usize]; 4]));
        let out = colorize(&image, [10, 20, 30], [200, 210, 220]);
        assert_eq!(out.get_pixel(0, 0).0, [10, 20, 30, 255]);
        assert_eq!(out.get_pixel(1, 0).0, [200, 210, 220, 255]);
        assert_eq!(out.get_pixel(2, 0).0, [105, 115, 125, 255]);
    }

    #[test]
    fn every_font_and_style_renders() {
        for font in CoverFont::ALL {
            for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
                let cover = TextCover {
                    font,
                    bold,
                    italic,
                    ..TextCover::default()
                };
                let text = text(&cover, "Persuasion", "Jane Austen");
                assert!(text.contains(&true), "{font:?}");
            }
        }
    }
}

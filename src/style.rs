//! The monochrome look: the UI fonts, the theme and the small parts that the
//! views share.

use std::borrow::Cow;
use std::rc::Rc;

use gpui::{
    App, Div, ElementId, Global, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    SharedString, Stateful, Styled, canvas, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit::component::{ActiveTheme, Theme, ThemeMode, ThemeSet};

/// The dot-matrix font for numbers.
pub const DOT: &str = "DotGothic16";
/// Pixels of a cover thumbnail in one dot of the dithering.
pub const DITHER_CELL: u32 = 4;

pub fn init(cx: &mut App) {
    let fonts: [&'static [u8]; 3] = [
        include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
        include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
        include_bytes!("../assets/fonts/DotGothic16-Regular.ttf"),
    ];
    cx.text_system()
        .add_fonts(fonts.into_iter().map(Cow::Borrowed).collect())
        .expect("bundled fonts must load");

    let mode = Theme::global(cx).mode;
    set_theme(mode, Tint::DRAWN, cx);
}

/// Colors with less saturation than this are neutral and take the tint of the
/// settings. The others, as success and danger, keep their color.
const NEUTRAL: f32 = 0.25;

/// A change to the neutral colors of the theme.
#[derive(Clone, Copy, PartialEq)]
pub struct Tint {
    /// Degrees, or `None` for the hue of each color as drawn.
    pub hue: Option<u16>,
    /// Percent of the saturation of each color as drawn.
    pub saturation: u16,
}

impl Tint {
    pub const DRAWN: Tint = Tint {
        hue: None,
        saturation: 100,
    };
}

/// Installs the theme with `tint` on its neutral colors.
pub fn set_theme(mode: ThemeMode, tint: Tint, cx: &mut App) {
    let mut json: serde_json::Value =
        serde_json::from_str(include_str!("../assets/themes/monochrome.json"))
            .expect("bundled theme must parse");
    if let (true, Some(themes)) = (tint != Tint::DRAWN, json["themes"].as_array_mut()) {
        for theme in themes {
            let Some(colors) = theme["colors"].as_object_mut() else {
                continue;
            };
            for value in colors.values_mut() {
                if let Some(color) = value.as_str().and_then(|c| tinted(c, tint)) {
                    *value = color.into();
                }
            }
        }
    }
    let set: ThemeSet = serde_json::from_value(json).expect("bundled theme must parse");
    let theme = Theme::global_mut(cx);
    for config in set.themes {
        if config.mode.is_dark() {
            theme.dark_theme = Rc::new(config);
        } else {
            theme.light_theme = Rc::new(config);
        }
    }
    Theme::change(mode, None, cx);
}

/// A `#rrggbb` or `#rrggbbaa` color with `tint`, if it is neutral.
fn tinted(hex: &str, tint: Tint) -> Option<String> {
    let digits = hex.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    let (rgb, alpha) = match digits.len() {
        6 => (value, None),
        8 => (value >> 8, Some(value & 0xff)),
        _ => return None,
    };
    let mut color: Hsla = gpui::rgb(rgb).into();
    if color.s >= NEUTRAL {
        return None;
    }
    if let Some(hue) = tint.hue {
        color.h = hue as f32 / 360.;
    }
    color.s = (color.s * tint.saturation as f32 / 100.).min(1.);
    let [r, g, b] = self::rgb(color);
    Some(match alpha {
        Some(a) => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
        None => format!("#{r:02x}{g:02x}{b:02x}"),
    })
}

/// The tones that images are dithered to. A new hue does not change them
/// until `redither`, because dithering every image again is slow.
struct DitherTones(([u8; 3], [u8; 3]));

impl Global for DitherTones {}

/// The two tones of a dithered image, dark first, as RGB.
pub fn tones(cx: &App) -> ([u8; 3], [u8; 3]) {
    cx.try_global::<DitherTones>()
        .map(|tones| tones.0)
        .unwrap_or_else(|| theme_tones(cx))
}

/// Dithers the images to the tones of the theme.
pub fn redither(cx: &mut App) {
    cx.set_global(DitherTones(theme_tones(cx)));
    cx.refresh_windows();
}

/// The two tones of the theme, dark first, as RGB.
pub fn theme_tones(cx: &App) -> ([u8; 3], [u8; 3]) {
    let theme = cx.theme();
    let (ink, paper) = (rgb(theme.foreground), rgb(theme.background));
    if theme.is_dark() {
        (paper, ink)
    } else {
        (ink, paper)
    }
}

pub fn rgb(color: Hsla) -> [u8; 3] {
    let color = color.to_rgb();
    [color.r, color.g, color.b].map(|c| (c * 255.).round() as u8)
}

/// Quiet lines and marks.
pub fn faint(cx: &App) -> Hsla {
    cx.theme().muted_foreground.opacity(0.55)
}

/// The thin line between rows of a list.
pub fn hairline(cx: &App) -> Hsla {
    cx.theme().foreground.opacity(0.1)
}

/// A small boxed label.
pub fn tag(text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .h(px(18.))
        .px(px(5.))
        .flex()
        .items_center()
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .text_size(px(10.))
        .line_height(px(16.))
        .whitespace_nowrap()
        .child(text.into())
}

pub fn tag_inverted(text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    tag(text, cx)
        .bg(theme.foreground)
        .text_color(theme.background)
}

/// A boxed button. A filled button has the colors the other way round, for
/// the main action or the current choice.
pub fn button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    filled: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let (bg, fg, hover) = if filled {
        (theme.foreground, theme.background, theme.primary_hover)
    } else {
        (theme.background, theme.foreground, theme.list_hover)
    };
    div()
        .id(id)
        .flex_none()
        .h(px(24.))
        .px(px(8.))
        .flex()
        .items_center()
        .border_1()
        .border_color(theme.border)
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.))
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(move |button| button.bg(hover))
        .child(text.into())
}

/// A horizontal line that takes the free width.
pub fn rule(cx: &App) -> Div {
    div().flex_1().h(px(1.)).bg(cx.theme().border)
}

/// A muted label in capitals.
pub fn label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(11.))
        .text_color(cx.theme().muted_foreground)
        .whitespace_nowrap()
        .child(text.into())
}

/// Text in the dot-matrix font.
pub fn dot(text: impl Into<SharedString>, size: f32) -> Div {
    div()
        .flex_none()
        .font_family(DOT)
        .text_size(px(size))
        .line_height(relative(1.))
        .child(text.into())
}

/// Ten small squares, filled to the percent.
pub fn progress(percent: f32, cx: &App) -> Div {
    let filled = (percent / 10.).round() as usize;
    let (on, off) = (cx.theme().foreground, faint(cx));
    div()
        .flex()
        .gap(px(2.))
        .children((0..10).map(|i| div().size(px(3.)).bg(if i < filled { on } else { off })))
}

/// Two diagonals across the parent.
pub fn cross(color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let lines = [
                (bounds.origin, bounds.bottom_right()),
                (bounds.top_right(), bounds.bottom_left()),
            ];
            for (from, to) in lines {
                let mut path = PathBuilder::stroke(px(1.));
                path.move_to(from);
                path.line_to(to);
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// How far the box behind a panel or menu shows, as its shadow.
pub const SHADOW: f32 = 4.;

/// The part of the shadow box that shows at the side of a panel that is as
/// tall as its parent. On the right side of the panel, or else the left.
pub fn side_shadow(right: bool, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(px(SHADOW))
        .bg(theme.background)
        .border_color(theme.border)
        .map(|strip| {
            if right {
                strip.right(px(-SHADOW)).border_r_1()
            } else {
                strip.left(px(-SHADOW)).border_l_1()
            }
        })
}

/// Corner marks just outside a box, like the crop marks of a print.
pub fn crop_marks(cx: &App) -> impl Iterator<Item = Div> {
    let color = cx.theme().foreground;
    let mark = move || div().absolute().size(px(14.)).border_color(color);
    let offset = px(-8.);
    [
        mark().top(offset).left(offset).border_t_1().border_l_1(),
        mark().top(offset).right(offset).border_t_1().border_r_1(),
        mark().bottom(offset).left(offset).border_b_1().border_l_1(),
        mark()
            .bottom(offset)
            .right(offset)
            .border_b_1()
            .border_r_1(),
    ]
    .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_changes_only_neutral_colors() {
        let tint = |hue, saturation| Tint { hue, saturation };
        let channels =
            |hex: String| [1, 3, 5].map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap());
        let [r, _, b] = channels(tinted("#dddbd4", tint(Some(220), 100)).unwrap());
        assert!(b > r);
        let [r, g, b] = channels(tinted("#dddbd4", tint(None, 0)).unwrap());
        assert!(r == g && g == b);
        assert!(
            tinted("#1b1b1a40", tint(Some(220), 100))
                .unwrap()
                .ends_with("40")
        );
        assert_eq!(tinted("#a33a2c", tint(Some(220), 100)), None);
    }
}

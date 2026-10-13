//! The monochrome look: the UI fonts, the theme and the small parts that the
//! views share.

use std::borrow::Cow;
use std::rc::Rc;

use gpui::{
    App, Bounds, Div, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    SharedString, Stateful, Styled, canvas, div, fill, point, px, relative, size,
};
use gpui_kit::component::{ActiveTheme, Theme, ThemeSet};

/// The dot-matrix font for numbers.
pub const DOT: &str = "DotGothic16";
const GRID: f32 = 12.;
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

    let set: ThemeSet = serde_json::from_str(include_str!("../assets/themes/monochrome.json"))
        .expect("bundled theme must parse");
    let theme = Theme::global_mut(cx);
    for config in set.themes {
        if config.mode.is_dark() {
            theme.dark_theme = Rc::new(config);
        } else {
            theme.light_theme = Rc::new(config);
        }
    }
    let mode = theme.mode;
    Theme::change(mode, None, cx);
}

/// The two tones of a dithered image, dark first, as RGB.
pub fn tones(cx: &App) -> ([u8; 3], [u8; 3]) {
    let theme = cx.theme();
    let (ink, paper) = (rgb(theme.foreground), rgb(theme.background));
    if theme.is_dark() {
        (paper, ink)
    } else {
        (ink, paper)
    }
}

fn rgb(color: Hsla) -> [u8; 3] {
    let color = color.to_rgb();
    [color.r, color.g, color.b].map(|c| (c * 255.).round() as u8)
}

/// Lines between the lines of the grid, and other quiet parts.
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

/// The faint grid of the screen, behind the content of its parent.
pub fn grid(cx: &App) -> impl IntoElement {
    let color = cx.theme().foreground.opacity(0.07);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let mut x = bounds.left();
            while x < bounds.right() {
                let line = Bounds::new(point(x, bounds.top()), size(px(1.), bounds.size.height));
                window.paint_quad(fill(line, color));
                x += px(GRID);
            }
            let mut y = bounds.top();
            while y < bounds.bottom() {
                let line = Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.)));
                window.paint_quad(fill(line, color));
                y += px(GRID);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
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

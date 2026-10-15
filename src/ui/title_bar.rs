//! The title bar. On Windows and macOS the app draws it in the theme colors,
//! in place of the system title bar. macOS keeps its window buttons on it.
//! Other systems keep their own.

use gpui::Window;

const HEIGHT: f32 = 30.;

/// The height of the title bar that the app draws. A fullscreen window has none.
pub fn height(window: &Window) -> f32 {
    if cfg!(any(target_os = "windows", target_os = "macos")) && !window.is_fullscreen() {
        HEIGHT
    } else {
        0.
    }
}

#[cfg(target_os = "windows")]
pub fn render(window: &Window, cx: &gpui::App) -> Option<impl gpui::IntoElement + use<>> {
    use gpui::{InteractiveElement, ParentElement, Styled, WindowControlArea, div, px, relative};
    use gpui_kit::component::ActiveTheme;

    if window.is_fullscreen() {
        return None;
    }
    let theme = cx.theme();
    let (ink, paper, hover) = (theme.foreground, theme.background, theme.list_hover);
    let control = |id: &'static str, area: WindowControlArea| {
        div()
            .id(id)
            .flex_none()
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .border_l_1()
            .border_color(theme.border)
            .window_control_area(area)
    };
    let mark = |w: f32, h: f32| div().w(px(w)).h(px(h)).border_1().border_color(ink);
    let close = control("close", WindowControlArea::Close)
        .text_size(px(14.))
        .hover(move |close| close.bg(ink).text_color(paper))
        .child("×");
    let bar = div()
        .flex_none()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .bg(paper)
        .text_color(ink)
        .border_b_1()
        .border_color(theme.border)
        .font_family(theme.font_family.clone())
        .line_height(relative(1.))
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .pl(px(16.))
                .text_size(px(11.))
                .window_control_area(WindowControlArea::Drag)
                .child("[ BOOK-BUDDY ]"),
        )
        .child(
            control("minimize", WindowControlArea::Min)
                .hover(move |button| button.bg(hover))
                .child(mark(8., 1.)),
        )
        .child(
            control("maximize", WindowControlArea::Max)
                .hover(move |button| button.bg(hover))
                .child(mark(7., 7.)),
        )
        .child(close);
    Some(bar)
}

/// The left part is free for the window buttons of macOS.
#[cfg(target_os = "macos")]
pub fn render(window: &Window, cx: &gpui::App) -> Option<impl gpui::IntoElement + use<>> {
    use gpui::{InteractiveElement, MouseButton, ParentElement, Styled, div, px, relative};
    use gpui_kit::component::ActiveTheme;

    if window.is_fullscreen() {
        return None;
    }
    let theme = cx.theme();
    let bar = div()
        .id("title-bar")
        .flex_none()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.background)
        .text_color(theme.foreground)
        .border_b_1()
        .border_color(theme.border)
        .font_family(theme.font_family.clone())
        .text_size(px(11.))
        .line_height(relative(1.))
        .on_mouse_down(MouseButton::Left, |event, window, _| {
            if event.click_count == 2 {
                window.titlebar_double_click();
            } else {
                window.start_window_move();
            }
        })
        .child("[ BOOK BUDDY ]");
    Some(bar)
}

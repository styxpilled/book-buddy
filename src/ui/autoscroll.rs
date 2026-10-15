//! Scroll with the middle button, and the scrollbar of the views that scroll.

use std::time::{Duration, Instant};

use gpui::{
    Context, CursorStyle, Div, InteractiveElement, MouseButton, Pixels, Point, Styled, Window, div,
    px,
};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarHandle, ScrollbarMode};

/// The distance from the anchor where the pointer does not scroll.
const DEAD_ZONE: f32 = 12.;
/// The pixels per second, for each pixel of the pointer out of the dead zone.
const SPEED: f32 = 9.;
/// A longer frame does not make a longer step.
const MAX_STEP: Duration = Duration::from_millis(50);

/// The speed follows the vertical distance of the pointer from the anchor.
pub struct Autoscroll {
    anchor: Point<Pixels>,
    /// The pointer went out of the dead zone, so the release of the button
    /// stops the scroll. Without a move, the next click stops it.
    moved: bool,
    last_frame: Instant,
}

impl Autoscroll {
    pub fn new(anchor: Point<Pixels>) -> Self {
        Self {
            anchor,
            moved: false,
            last_frame: Instant::now(),
        }
    }

    /// The distance to scroll down for the time since the last frame. Call it
    /// in each render. It asks for the next frame.
    pub fn step(&mut self, window: &Window) -> Pixels {
        window.request_animation_frame();
        let now = Instant::now();
        let elapsed = (now - self.last_frame).min(MAX_STEP);
        self.last_frame = now;
        let distance = f32::from(window.mouse_position().y - self.anchor.y);
        let out = (distance.abs() - DEAD_ZONE).max(0.);
        if out > 0. {
            self.moved = true;
        }
        px(distance.signum() * out * SPEED * elapsed.as_secs_f32())
    }
}

/// Over the view while the autoscroll is on. It takes all clicks, so a click
/// only stops the scroll. `slot` is where the view keeps its autoscroll.
pub fn overlay<V: 'static>(
    slot: fn(&mut V) -> &mut Option<Autoscroll>,
    cx: &mut Context<V>,
) -> Div {
    let overlay = div()
        .absolute()
        .inset_0()
        .occlude()
        .cursor(CursorStyle::ResizeUpDown)
        .on_mouse_up(
            MouseButton::Middle,
            cx.listener(move |view, _, _, cx| {
                let autoscroll = slot(view);
                if autoscroll.as_ref().is_some_and(|a| a.moved) {
                    *autoscroll = None;
                    cx.notify();
                }
            }),
        );
    let buttons = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];
    buttons.into_iter().fold(overlay, |overlay, button| {
        overlay.on_mouse_down(
            button,
            cx.listener(move |view, _, _, cx| {
                *slot(view) = None;
                cx.notify();
            }),
        )
    })
}

pub fn scrollbar<H: ScrollbarHandle + Clone>(handle: &H) -> Scrollbar {
    Scrollbar::vertical(handle)
        .mode(ScrollbarMode::Always)
        .styles(|styles| styles.thumb(|thumb| thumb.radius(px(0.))))
}

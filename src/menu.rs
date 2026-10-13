//! A popup menu in the monochrome look: a box with a 1 px line and a second
//! box behind it as a shadow.

use std::rc::Rc;

use gpui::{
    App, Context, Div, ElementId, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, Render,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window, anchored, deferred, div,
    point, prelude::FluentBuilder, px,
};
use gpui_kit::component::ActiveTheme;

use crate::style;

const ITEM_HEIGHT: f32 = 26.;

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub struct MenuItem {
    label: SharedString,
    checked: bool,
    disabled: bool,
    action: Action,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            checked: false,
            disabled: false,
            action: Rc::new(action),
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The menu wants to close: an item was chosen, or the user clicked outside
/// or pressed Escape.
pub struct Dismiss;

pub struct Menu {
    items: Vec<MenuItem>,
    selected: Option<usize>,
    focus: FocusHandle,
}

impl EventEmitter<Dismiss> for Menu {}

impl Focusable for Menu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Menu {
    /// A new menu takes the keyboard focus.
    pub fn new(items: Vec<MenuItem>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            items,
            selected: None,
            focus,
        }
    }

    fn choose(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(ix).filter(|item| !item.disabled) else {
            return;
        };
        let action = item.action.clone();
        cx.emit(Dismiss);
        action(window, cx);
    }

    /// Selects the next item that is not disabled, down or up.
    fn step(&mut self, down: bool, cx: &mut Context<Self>) {
        let count = self.items.len();
        let mut ix = self.selected;
        for _ in 0..count {
            let next = match (ix, down) {
                (None, true) => 0,
                (None, false) => count - 1,
                (Some(ix), true) => (ix + 1) % count,
                (Some(ix), false) => (ix + count - 1) % count,
            };
            ix = Some(next);
            if !self.items[next].disabled {
                self.selected = ix;
                cx.notify();
                return;
            }
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "down" => self.step(true, cx),
            "up" => self.step(false, cx),
            "enter" | "space" => {
                if let Some(ix) = self.selected {
                    self.choose(ix, window, cx);
                }
            }
            "escape" => cx.emit(Dismiss),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (paper, ink, hover) = (theme.popover, theme.border, theme.list_hover);
        let marks = self.items.iter().any(|item| item.checked);

        let rows: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                div()
                    .id(ix)
                    .h(px(ITEM_HEIGHT))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .whitespace_nowrap()
                    .when(self.selected == Some(ix), |row| row.bg(hover))
                    .when(marks, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .size(px(6.))
                                .when(item.checked, |mark| mark.bg(ink)),
                        )
                    })
                    .child(item.label.clone())
                    .map(|row| {
                        if item.disabled {
                            row.opacity(0.45)
                        } else {
                            row.cursor_pointer()
                                .on_hover(cx.listener(move |menu, hovered: &bool, _, cx| {
                                    if *hovered {
                                        menu.selected = Some(ix);
                                        cx.notify();
                                    }
                                }))
                                .on_click(cx.listener(move |menu, _, window, cx| {
                                    menu.choose(ix, window, cx)
                                }))
                        }
                    })
            })
            .collect();

        div()
            .id("menu")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .relative()
            .min_w(px(160.))
            .text_size(px(11.))
            .text_color(theme.popover_foreground)
            .child(
                div()
                    .absolute()
                    .top(px(style::SHADOW))
                    .left(px(style::SHADOW))
                    .size_full()
                    .bg(paper)
                    .border_1()
                    .border_color(ink),
            )
            .child(
                div()
                    .relative()
                    .occlude()
                    .py(px(4.))
                    .bg(paper)
                    .border_1()
                    .border_color(ink)
                    .children(rows),
            )
    }
}

/// A layer that takes all mouse input from the content under it. A press of
/// any button on the layer itself runs `on_press`.
pub fn shield(
    id: impl Into<ElementId>,
    on_press: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let on_press: Action = Rc::new(on_press);
    let handler = move |_: &MouseDownEvent, window: &mut Window, cx: &mut App| on_press(window, cx);
    div()
        .id(id)
        .occlude()
        .on_mouse_down(MouseButton::Left, handler.clone())
        .on_mouse_down(MouseButton::Right, handler.clone())
        .on_mouse_down(MouseButton::Middle, handler)
}

/// The menu above all other content, at `position` in the window, or else
/// where it is laid out. Under it, a layer over the whole window takes the
/// clicks outside the menu, so they do nothing else.
pub fn popup(
    menu: &Entity<Menu>,
    position: Option<Point<Pixels>>,
    window: &Window,
) -> impl IntoElement {
    let size = window.viewport_size();
    let dismiss = menu.downgrade();
    let layer = shield("menu-outside", move |_, cx| {
        dismiss.update(cx, |_, cx| cx.emit(Dismiss)).ok();
    })
    .w(size.width)
    .h(size.height);
    div()
        .child(deferred(anchored().position(point(px(0.), px(0.))).child(layer)).with_priority(1))
        .child(
            deferred(
                anchored()
                    .when_some(position, |anchored, position| anchored.position(position))
                    .snap_to_window_with_margin(px(8.))
                    .child(menu.clone()),
            )
            .with_priority(2),
        )
}

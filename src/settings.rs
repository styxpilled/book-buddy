use book_buddy::library::{FONT_SIZES, Font, Mode, Settings, Theme};
use gpui::{
    AnyElement, App, AppContext, BorrowAppContext, Context, Div, Entity, EventEmitter, FocusHandle,
    Focusable, Global, InteractiveElement, IntoElement, KeyBinding, ParentElement,
    PathPromptOptions, Render, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Subscription, Window, actions, div, prelude::FluentBuilder, px,
};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme, ThemeMode};

use crate::Back;
use crate::scan;
use crate::store::Store;
use crate::ui::menu::{self, Dismiss, Menu, MenuItem};
use crate::ui::style::{self, Tint};

const PANEL_WIDTH: f32 = 500.;
const LABEL_WIDTH: f32 = 110.;
/// The hue of the theme as drawn, for the slider when the setting is `None`.
const DRAWN_HUE: u16 = 47;
const MAX_SATURATION: u16 = 400;

actions!(settings, [OpenSettings, FontLarger, FontSmaller]);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-,", OpenSettings, None),
        KeyBinding::new("ctrl-=", FontLarger, None),
        KeyBinding::new("ctrl--", FontSmaller, None),
        KeyBinding::new("escape", Back, Some("Settings")),
    ]);
    cx.on_action(|_: &FontLarger, cx| change(cx, |s| s.font_size += 1));
    cx.on_action(|_: &FontSmaller, cx| change(cx, |s| s.font_size -= 1));
    apply_theme(cx);
}

/// Changes the settings and saves them. Views that use them observe the store.
pub fn change(cx: &mut App, f: impl FnOnce(&mut Settings)) {
    let changed = cx.update_global(|store: &mut Store, _| {
        let settings = &mut store.library.settings;
        let old = settings.clone();
        f(settings);
        settings.font_size = settings
            .font_size
            .clamp(*FONT_SIZES.start(), *FONT_SIZES.end());
        let changed = *settings != old;
        if changed {
            store.save();
        }
        changed
    });
    if changed {
        apply_theme(cx);
        cx.refresh_windows();
    }
}

/// The theme mode and tint that are installed.
#[derive(Default, PartialEq)]
struct AppliedTheme(Option<(ThemeMode, Tint)>);

impl Global for AppliedTheme {}

fn apply_theme(cx: &mut App) {
    let settings = &cx.global::<Store>().library.settings;
    let mode = match settings.theme {
        Theme::Light => ThemeMode::Light,
        Theme::Dark => ThemeMode::Dark,
    };
    let wanted = (mode, tint(settings));
    let applied = cx.default_global::<AppliedTheme>().0;
    if applied != Some(wanted) {
        style::set_theme(wanted.0, wanted.1, cx);
        cx.set_global(AppliedTheme(Some(wanted)));
        // Tones of the other mode would not show on this background.
        if applied.map(|(mode, _)| mode) != Some(mode) {
            style::redither(cx);
        }
    }
}

fn tint(settings: &Settings) -> Tint {
    Tint {
        hue: settings.hue,
        saturation: settings.saturation,
    }
}

/// Shows a tint while a slider moves, without saving it.
fn preview_tint(tint: Tint, cx: &mut App) {
    let mode = cx.theme().mode;
    if cx.default_global::<AppliedTheme>().0 != Some((mode, tint)) {
        style::set_theme(mode, tint, cx);
        cx.set_global(AppliedTheme(Some((mode, tint))));
        cx.refresh_windows();
    }
}

pub struct SettingsPanel {
    focus: FocusHandle,
    hue: Entity<SliderState>,
    saturation: Entity<SliderState>,
    /// The tint of the sliders while one moves.
    shown_tint: Option<Tint>,
    font_menu: Option<(Entity<Menu>, Subscription)>,
    /// CLEAR LIBRARY was clicked once and waits for a second click.
    confirm_clear: bool,
    /// `.epub` files open in this exe.
    #[cfg(target_os = "windows")]
    associated: bool,
    _subscriptions: Vec<Subscription>,
}

pub struct CloseSettings;

impl EventEmitter<CloseSettings> for SettingsPanel {}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tint = tint(&cx.global::<Store>().library.settings);
        let slider = |max: u16, step: u16, value: u16, cx: &mut Context<Self>| {
            cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(max as f32)
                    .step(step as f32)
                    .default_value(value as f32)
            })
        };
        let hue = slider(359, 1, tint.hue.unwrap_or(DRAWN_HUE), cx);
        let saturation = slider(MAX_SATURATION, 5, tint.saturation, cx);
        let subscriptions = vec![
            cx.observe_global::<Store>(|_, cx| cx.notify()),
            cx.subscribe(&hue, |panel, _, event: &SliderEvent, cx| {
                panel.slid(event, |tint, hue| tint.hue = Some(hue), cx)
            }),
            cx.subscribe(&saturation, |panel, _, event: &SliderEvent, cx| {
                panel.slid(event, |tint, saturation| tint.saturation = saturation, cx)
            }),
        ];
        Self {
            focus: cx.focus_handle(),
            hue,
            saturation,
            shown_tint: None,
            font_menu: None,
            confirm_clear: false,
            #[cfg(target_os = "windows")]
            associated: crate::platform::association::registered(),
            _subscriptions: subscriptions,
        }
    }

    /// A slider shows its tint while it moves, and saves it when released.
    fn slid(&mut self, event: &SliderEvent, set: fn(&mut Tint, u16), cx: &mut Context<Self>) {
        let mut shown = self
            .shown_tint
            .unwrap_or_else(|| tint(&cx.global::<Store>().library.settings));
        let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
        set(&mut shown, value.end().round() as u16);
        if let SliderEvent::Change(_) = event {
            self.shown_tint = Some(shown);
            preview_tint(shown, cx);
        } else {
            self.shown_tint = None;
            change(cx, |s| {
                s.hue = shown.hue;
                s.saturation = shown.saturation;
            });
        }
    }

    fn drawn_tint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        change(cx, |s| {
            s.hue = Tint::DRAWN.hue;
            s.saturation = Tint::DRAWN.saturation;
        });
        self.hue.update(cx, |slider, cx| {
            slider.set_value(DRAWN_HUE as f32, window, cx)
        });
        self.saturation.update(cx, |slider, cx| {
            slider.set_value(Tint::DRAWN.saturation as f32, window, cx)
        });
    }

    fn close(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(CloseSettings);
    }

    fn open_font_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = cx.global::<Store>().library.settings.font;
        let items = Font::ALL
            .into_iter()
            .map(|font| {
                MenuItem::new(font_label(font), move |_, cx| change(cx, |s| s.font = font))
                    .checked(font == current)
            })
            .collect();
        let menu = cx.new(|cx| Menu::new(items, window, cx));
        let dismiss = cx.subscribe_in(&menu, window, |panel, _, _: &Dismiss, window, cx| {
            panel.font_menu = None;
            panel.focus.focus(window, cx);
            cx.notify();
        });
        self.font_menu = Some((menu, dismiss));
        cx.notify();
    }

    /// The button that makes `.epub` files open in this exe.
    #[cfg(target_os = "windows")]
    fn association(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (text, filled) = if self.associated {
            ("REGISTERED", true)
        } else {
            ("REGISTER", false)
        };
        let button = style::button("associate", text, filled, cx).on_click(cx.listener(
            |panel, _, _, cx| {
                if let Err(e) = crate::platform::association::register() {
                    log::error!("{e:#}");
                }
                panel.associated = crate::platform::association::registered();
                cx.notify();
            },
        ));
        Some(row("EPUB FILES =", button, cx).into_any_element())
    }

    #[cfg(not(target_os = "windows"))]
    fn association(&self, _: &mut Context<Self>) -> Option<AnyElement> {
        None
    }

    /// Clears the library on the second click.
    fn clear_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let text = if self.confirm_clear {
            "CLICK AGAIN TO CLEAR"
        } else {
            "CLEAR LIBRARY"
        };
        style::button("clear-library", text, self.confirm_clear, cx).on_click(cx.listener(
            |panel, _, _, cx| {
                if panel.confirm_clear {
                    cx.update_global(|store: &mut Store, _| store.clear_library());
                }
                panel.confirm_clear = !panel.confirm_clear;
                cx.notify();
            },
        ))
    }

    fn add_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Watch folder".into()),
        });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                cx.update(|cx| {
                    cx.update_global(|store: &mut Store, _| {
                        for path in paths {
                            if !store.library.folders.contains(&path) {
                                store.library.folders.push(path);
                            }
                        }
                        store.save();
                    });
                    scan::start(cx);
                });
            }
        })
        .detach();
    }
}

fn font_label(font: Font) -> &'static str {
    match font {
        Font::Serif => "LITERATA",
        Font::Sans => "INTER",
        Font::Newsreader => "NEWSREADER",
        Font::SourceSerif => "SOURCE SERIF",
        Font::Alegreya => "ALEGREYA",
        Font::Garamond => "EB GARAMOND",
        Font::Mono => "PLEX MONO",
    }
}

/// One button for each choice. The button of the current value is filled.
fn choice<T: Copy + PartialEq + 'static>(
    id: &'static str,
    current: T,
    options: &[(T, &'static str, bool)],
    set: fn(&mut Settings, T),
    cx: &App,
) -> impl IntoElement {
    div().flex().children(
        options
            .iter()
            .enumerate()
            .map(move |(ix, &(value, label, enabled))| {
                let button = style::button((id, ix), label, value == current, cx)
                    // The borders of neighbours overlap.
                    .when(ix > 0, |button| button.ml(px(-1.)));
                enabled_button(button, enabled, move |cx| change(cx, |s| set(s, value)))
            }),
    )
}

/// A disabled button is faded and does nothing.
fn enabled_button(
    button: Stateful<Div>,
    enabled: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> Stateful<Div> {
    if enabled {
        button.on_click(move |_, _, cx| on_click(cx))
    } else {
        button.opacity(0.45).cursor_default()
    }
}

/// The dot font has no `%`, so a unit is in the label font.
fn slider_row(
    slider: &Entity<SliderState>,
    value: String,
    unit: &'static str,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().flex_1().child(Slider::new(slider)))
        .child(
            div()
                .w(px(52.))
                .flex()
                .items_baseline()
                .justify_end()
                .gap(px(2.))
                .child(style::dot(value, 16.))
                .child(style::label(unit, cx)),
        )
}

fn row(label: &'static str, control: impl IntoElement, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(style::label(label, cx).flex_none().w(px(LABEL_WIDTH)))
        .child(control)
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let association = self.association(cx);
        let clear = self.clear_button(cx);
        let theme = cx.theme();
        let library = &cx.global::<Store>().library;
        let settings = library.settings.clone();
        let folders = library.folders.clone();
        let shown = self.shown_tint.unwrap_or_else(|| tint(&settings));

        let size = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(enabled_button(
                style::button("smaller", "-", false, cx),
                settings.font_size > *FONT_SIZES.start(),
                |cx| change(cx, |s| s.font_size -= 1),
            ))
            .child(
                style::dot(settings.font_size.to_string(), 20.)
                    .w(px(28.))
                    .text_center(),
            )
            .child(enabled_button(
                style::button("larger", "+", false, cx),
                settings.font_size < *FONT_SIZES.end(),
                |cx| change(cx, |s| s.font_size += 1),
            ));

        let font = div()
            .relative()
            .child(
                style::button(
                    "font",
                    format!("{} ▾", font_label(settings.font)),
                    false,
                    cx,
                )
                .on_click(cx.listener(|panel, _, window, cx| panel.open_font_menu(window, cx))),
            )
            .children(self.font_menu.as_ref().map(|(menu, _)| {
                div()
                    .absolute()
                    .top_full()
                    .left_0()
                    .mt(px(4.))
                    .child(menu::popup(menu, None, window))
            }));

        let hairline = style::hairline(cx);
        let folder_rows = folders.into_iter().enumerate().map(|(ix, folder)| {
            let shown: SharedString = folder.display().to_string().into();
            div()
                .h(px(30.))
                .flex()
                .items_center()
                .gap(px(10.))
                .border_b_1()
                .border_color(hairline)
                .child(style::dot(format!("{:02}", ix + 1), 12.).text_color(theme.muted_foreground))
                .child(div().flex_1().min_w_0().truncate().child(shown))
                .child(
                    style::button(("remove-folder", ix), "REMOVE", false, cx)
                        .h(px(20.))
                        .on_click(move |_, _, cx| {
                            cx.update_global(|store: &mut Store, _| {
                                store.library.folders.retain(|f| *f != folder);
                                store.save();
                            });
                        }),
                )
        });
        let folders = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(style::label("WATCHED FOLDERS", cx))
            .children(folder_rows)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .pt(px(4.))
                    .child(
                        style::button("add-folder", "+ ADD FOLDER", false, cx)
                            .on_click(cx.listener(|panel, _, _, cx| panel.add_folder(cx))),
                    )
                    .child(clear),
            );

        let panel = div()
            .id("settings-panel")
            .relative()
            .w(px(PANEL_WIDTH))
            .max_w_full()
            .flex()
            .flex_col()
            .gap(px(14.))
            .pt(px(22.))
            .px(px(24.))
            .pb(px(20.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .occlude()
            .children(style::crop_marks(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(style::tag_inverted("SETTINGS", cx))
                    .child(style::rule(cx))
                    .child(
                        style::button("close-settings", "ESC", false, cx)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(CloseSettings))),
                    ),
            )
            .child(row(
                "THEME =",
                choice(
                    "theme",
                    settings.theme,
                    &[(Theme::Light, "LIGHT", true), (Theme::Dark, "DARK", true)],
                    |s, v| s.theme = v,
                    cx,
                ),
                cx,
            ))
            .child(row(
                "HUE =",
                slider_row(
                    &self.hue,
                    shown.hue.unwrap_or(DRAWN_HUE).to_string(),
                    "",
                    cx,
                ),
                cx,
            ))
            .child(row(
                "SATURATION =",
                slider_row(&self.saturation, shown.saturation.to_string(), "%", cx),
                cx,
            ))
            .child(row(
                "",
                div()
                    .flex()
                    .gap(px(10.))
                    .child(
                        style::button("drawn-tint", "DEFAULT", shown == Tint::DRAWN, cx).on_click(
                            cx.listener(|panel, _, window, cx| panel.drawn_tint(window, cx)),
                        ),
                    )
                    .child(enabled_button(
                        style::button("redither", "RE-DITHER", false, cx),
                        style::tones(cx) != style::theme_tones(cx),
                        style::redither,
                    )),
                cx,
            ))
            .child(row("FONT =", font, cx))
            .child(row("FONT SIZE =", size, cx))
            .child(row(
                "MODE =",
                choice(
                    "mode",
                    settings.mode,
                    &[
                        (Mode::Scroll, "SCROLL", true),
                        (Mode::Paginated, "PAGINATED", true),
                    ],
                    |s, v| s.mode = v,
                    cx,
                ),
                cx,
            ))
            .child(row(
                "BARS =",
                choice(
                    "bars",
                    settings.hide_bars,
                    &[(false, "ALWAYS", true), (true, "ON HOVER", true)],
                    |s, v| s.hide_bars = v,
                    cx,
                ),
                cx,
            ))
            .child(row(
                "IMAGES =",
                choice(
                    "images",
                    settings.dither_images,
                    &[(false, "ORIGINAL", true), (true, "DITHERED", true)],
                    |s, v| s.dither_images = v,
                    cx,
                ),
                cx,
            ))
            .child(row(
                "SHOW HIDDEN =",
                choice(
                    "show-hidden",
                    settings.show_hidden,
                    &[(false, "OFF", true), (true, "ON", true)],
                    |s, v| s.show_hidden = v,
                    cx,
                ),
                cx,
            ))
            .children(association)
            .child(div().h(px(1.)).bg(theme.border))
            .child(folders);

        let view = cx.entity().downgrade();
        menu::shield("settings", move |_, cx| {
            view.update(cx, |_, cx| cx.emit(CloseSettings)).ok();
        })
        .key_context("Settings")
        .track_focus(&self.focus)
        .on_action(cx.listener(Self::close))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .bg(theme.overlay)
        .child(panel)
    }
}

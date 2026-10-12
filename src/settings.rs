use book_buddy::library::{FONT_SIZES, Font, Mode, Settings, Theme};
use gpui::{
    App, BorrowAppContext, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, actions, div, px,
};
use gpui_kit::component::button::{Button, ButtonGroup, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Sizable, ThemeMode};

use crate::Back;
use crate::scan;
use crate::store::Store;

const PANEL_WIDTH: f32 = 480.;
const LABEL_WIDTH: f32 = 110.;

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

fn apply_theme(cx: &mut App) {
    let mode = match cx.global::<Store>().library.settings.theme {
        Theme::Light => ThemeMode::Light,
        Theme::Dark => ThemeMode::Dark,
    };
    if cx.theme().mode != mode {
        gpui_kit::component::Theme::change(mode, None, cx);
    }
}

pub struct SettingsPanel {
    focus: FocusHandle,
    _subscription: Subscription,
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
        Self {
            focus: cx.focus_handle(),
            _subscription: cx.observe_global::<Store>(|_, cx| cx.notify()),
        }
    }

    fn close(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(CloseSettings);
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

/// One button for each choice. The button of the current value is selected.
fn choice<T: Copy + PartialEq + 'static>(
    id: &'static str,
    current: T,
    options: &[(T, &'static str, bool)],
    set: fn(&mut Settings, T),
) -> ButtonGroup {
    let values: Vec<T> = options.iter().map(|(value, ..)| *value).collect();
    ButtonGroup::new(id)
        .outline()
        .small()
        .children(
            options
                .iter()
                .enumerate()
                .map(|(ix, (value, label, enabled))| {
                    Button::new(ix)
                        .label(*label)
                        .selected(*value == current)
                        .disabled(!enabled)
                }),
        )
        .on_click(move |clicked: &Vec<usize>, _, cx| {
            if let Some(&value) = clicked.first().and_then(|&ix| values.get(ix)) {
                change(cx, |settings| set(settings, value));
            }
        })
}

fn row(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(div().flex_none().w(px(LABEL_WIDTH)).child(label))
        .child(control)
}

impl Render for SettingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let library = &cx.global::<Store>().library;
        let settings = library.settings.clone();
        let folders = library.folders.clone();

        let size = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("smaller")
                    .label("-")
                    .outline()
                    .small()
                    .disabled(settings.font_size <= *FONT_SIZES.start())
                    .on_click(|_, _, cx| change(cx, |s| s.font_size -= 1)),
            )
            .child(
                div()
                    .w(px(32.))
                    .text_center()
                    .child(settings.font_size.to_string()),
            )
            .child(
                Button::new("larger")
                    .label("+")
                    .outline()
                    .small()
                    .disabled(settings.font_size >= *FONT_SIZES.end())
                    .on_click(|_, _, cx| change(cx, |s| s.font_size += 1)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Ctrl+= / Ctrl+-"),
            );

        let folder_rows = folders.into_iter().enumerate().map(|(ix, folder)| {
            let shown: SharedString = folder.display().to_string().into();
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_1().min_w_0().truncate().child(shown))
                .child(
                    Button::new(("remove-folder", ix))
                        .label("Remove")
                        .ghost()
                        .small()
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
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Watched folders"),
            )
            .children(folder_rows)
            .child(
                div().flex().child(
                    Button::new("add-folder")
                        .label("Add folder")
                        .small()
                        .on_click(cx.listener(|panel, _, _, cx| panel.add_folder(cx))),
                ),
            );

        let panel = div()
            .id("settings-panel")
            .w(px(PANEL_WIDTH))
            .max_w_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .occlude()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Settings"),
                    )
                    .child(
                        Button::new("close-settings")
                            .label("Close")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(CloseSettings))),
                    ),
            )
            .child(row(
                "Theme",
                choice(
                    "theme",
                    settings.theme,
                    &[(Theme::Light, "Light", true), (Theme::Dark, "Dark", true)],
                    |s, v| s.theme = v,
                ),
            ))
            .child(row(
                "Font",
                choice(
                    "font",
                    settings.font,
                    &[(Font::Serif, "Serif", true), (Font::Sans, "Sans", true)],
                    |s, v| s.font = v,
                ),
            ))
            .child(row("Font size", size))
            .child(row(
                "Mode",
                choice(
                    "mode",
                    settings.mode,
                    &[
                        (Mode::Scroll, "Scroll", true),
                        (Mode::Paginated, "Paginated", true),
                    ],
                    |s, v| s.mode = v,
                ),
            ))
            .child(div().h(px(1.)).bg(theme.border))
            .child(folders);

        div()
            .id("settings")
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
            .on_click(cx.listener(|_, _, _, cx| cx.emit(CloseSettings)))
            .child(panel)
    }
}

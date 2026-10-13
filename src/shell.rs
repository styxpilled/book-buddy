use std::path::PathBuf;

use book_buddy::library::WindowState;
use gpui::{
    App, AppContext, BorrowAppContext, Context, DispatchPhase, Entity, ExternalPaths, Focusable,
    InteractiveElement, IntoElement, Keystroke, MouseButton, MouseDownEvent, NavigationDirection,
    ParentElement, PathPromptOptions, Render, Styled, Subscription, Window, WindowBounds, canvas,
    div,
};

use crate::cover_adjust::{CloseCoverAdjust, CoverAdjust};
use crate::details::{CloseDetails, DetailsDialog};
use crate::library_view::{LibraryEvent, LibraryView};
use crate::reader::{Reader, ReaderEvent};
use crate::scan;
use crate::settings::{CloseSettings, OpenSettings, SettingsPanel};
use crate::store::Store;
use crate::{Import, ToggleFullscreen};

/// The root view. It shows the library or one book.
pub struct Shell {
    library: Entity<LibraryView>,
    reader: Option<Entity<Reader>>,
    cover_adjust: Option<(Entity<CoverAdjust>, Subscription)>,
    details: Option<(Entity<DetailsDialog>, Subscription)>,
    settings: Option<Entity<SettingsPanel>>,
    _settings_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    _reader_subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = cx.new(|cx| LibraryView::new(window, cx));
        let subscription =
            cx.subscribe_in(&library, window, |this, _, event, window, cx| match event {
                LibraryEvent::Open(path) => this.open(path.clone(), window, cx),
                LibraryEvent::AdjustCover(id) => this.open_cover_adjust(id, window, cx),
                LibraryEvent::EditDetails(id) => this.open_details(id, window, cx),
            });
        let mut shell = Self {
            library,
            reader: None,
            cover_adjust: None,
            details: None,
            settings: None,
            _settings_subscription: None,
            _subscriptions: vec![subscription],
            _reader_subscriptions: Vec::new(),
        };
        match path {
            Some(path) => shell.open(path, window, cx),
            None => shell.library.focus_handle(cx).focus(window, cx),
        }
        window.on_window_should_close(cx, |window, cx| {
            save_window_state(window, cx);
            true
        });
        shell
    }

    pub fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        let reader = cx.new(|cx| Reader::new(Some(path), cx));
        self._reader_subscriptions = vec![
            cx.subscribe_in(&reader, window, |this, _, event, window, cx| match event {
                ReaderEvent::Close => this.close(window, cx),
            }),
            // The library stays until the book is ready.
            cx.observe(&reader, |_, _, cx| cx.notify()),
        ];
        reader.focus_handle(cx).focus(window, cx);
        self.reader = Some(reader);
        cx.notify();
    }

    fn open_cover_adjust(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = CoverAdjust::open(id, cx) else {
            return;
        };
        let subscription = cx.subscribe_in(
            &view,
            window,
            |this, _, _: &CloseCoverAdjust, window, cx| {
                this.cover_adjust = None;
                this.library.focus_handle(cx).focus(window, cx);
                cx.notify();
            },
        );
        view.focus_handle(cx).focus(window, cx);
        self.cover_adjust = Some((view, subscription));
        cx.notify();
    }

    fn open_details(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = DetailsDialog::open(id, window, cx) else {
            return;
        };
        let subscription =
            cx.subscribe_in(&dialog, window, |this, _, _: &CloseDetails, window, cx| {
                this.details = None;
                this.library.focus_handle(cx).focus(window, cx);
                cx.notify();
            });
        dialog.focus_handle(cx).focus(window, cx);
        self.details = Some((dialog, subscription));
        cx.notify();
    }

    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.is_none() {
            let panel = cx.new(SettingsPanel::new);
            self._settings_subscription = Some(cx.subscribe_in(
                &panel,
                window,
                |this, _, _: &CloseSettings, window, cx| this.close_settings(window, cx),
            ));
            self.settings = Some(panel);
        }
        if let Some(panel) = &self.settings {
            panel.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        self._settings_subscription = None;
        match &self.reader {
            Some(reader) => reader.focus_handle(cx).focus(window, cx),
            None => self.library.focus_handle(cx).focus(window, cx),
        }
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(reader) = self.reader.take() {
            reader.update(cx, |reader, cx| reader.close(window, cx));
            // The next start opens the library, not this book.
            cx.update_global(|store: &mut Store, _| {
                store.library.last_book = None;
                store.save();
            });
            self._reader_subscriptions.clear();
            self.library.focus_handle(cx).focus(window, cx);
            cx.notify();
        }
    }

    fn import_dialog(&mut self, _: &Import, _: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add books".into()),
        });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                cx.update(|cx| scan::import(paths, cx));
            }
        })
        .detach();
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match (&self.reader, &self.cover_adjust) {
            (Some(reader), _) if !reader.read(cx).loading() => reader.clone().into_any_element(),
            (None, Some((view, _))) => view.clone().into_any_element(),
            _ => self.library.clone().into_any_element(),
        };
        #[cfg(target_os = "windows")]
        let title_bar = crate::title_bar::render(window, cx);
        #[cfg(not(target_os = "windows"))]
        let title_bar = None::<gpui::Empty>;
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .key_context("Shell")
            .on_action(cx.listener(Self::import_dialog))
            .on_action(cx.listener(Self::open_settings))
            .on_action(|_: &ToggleFullscreen, window, _| window.toggle_fullscreen())
            .on_drop(cx.listener(|_, paths: &ExternalPaths, _, cx| {
                scan::import(paths.paths().to_vec(), cx);
            }))
            .children(title_bar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(content)
                    .children(self.details.as_ref().map(|(dialog, _)| dialog.clone()))
                    .children(self.settings.clone()),
            )
            .child(back_button())
    }
}

fn save_window_state(window: &Window, cx: &mut App) {
    let (bounds, maximized, fullscreen) = match window.window_bounds() {
        WindowBounds::Windowed(bounds) => (bounds, false, false),
        WindowBounds::Maximized(bounds) => (bounds, true, false),
        WindowBounds::Fullscreen(bounds) => (bounds, false, true),
    };
    let state = WindowState {
        x: bounds.origin.x.into(),
        y: bounds.origin.y.into(),
        width: bounds.size.width.into(),
        height: bounds.size.height.into(),
        maximized,
        fullscreen,
    };
    cx.update_global(|store: &mut Store, _| {
        store.library.window = Some(state);
        store.save();
    });
}

/// The back button of a mouse does what Escape does. The listener is on the
/// window, so overlays that block the mouse do not stop it.
fn back_button() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |_, _, window, _| {
            window.on_mouse_event(|event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Navigate(NavigationDirection::Back)
                {
                    window.defer(cx, |window, cx| {
                        if let Ok(escape) = Keystroke::parse("escape") {
                            window.dispatch_keystroke(escape, cx);
                        }
                    });
                }
            });
        },
    )
    .absolute()
    .size_0()
}

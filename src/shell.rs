use std::path::PathBuf;

use book_buddy::library::WindowState;
use gpui::{
    Action, App, AppContext, BorrowAppContext, Context, DispatchPhase, Entity, ExternalPaths,
    Focusable, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, NavigationDirection,
    ParentElement, PathPromptOptions, Render, Styled, Subscription, Window, WindowBounds, canvas,
    div,
};

use crate::library_view::cover_adjust::{CloseCoverAdjust, CoverAdjust};
use crate::library_view::cover_editor::{CloseCoverEditor, CoverEditor};
use crate::library_view::details::{CloseDetails, DetailsDialog};
use crate::library_view::{LibraryEvent, LibraryView};
use crate::reader::{Reader, ReaderEvent};
use crate::scan;
use crate::settings::{CloseSettings, OpenSettings, SettingsPanel};
use crate::store::Store;
use crate::{CloseWindow, HistoryBack, HistoryForward, Import, ToggleFullscreen};

/// An entry of the window history.
#[derive(Clone, PartialEq)]
enum View {
    Library,
    Book(PathBuf),
}

/// The root view. It shows the library or one book.
pub struct Shell {
    library: Entity<LibraryView>,
    reader: Option<Entity<Reader>>,
    /// The book on screen while `reader` loads the next one.
    previous: Option<Entity<Reader>>,
    cover_adjust: Option<(Entity<CoverAdjust>, Subscription)>,
    cover_editor: Option<(Entity<CoverEditor>, Subscription)>,
    details: Option<(Entity<DetailsDialog>, Subscription)>,
    settings: Option<(Entity<SettingsPanel>, Subscription)>,
    back: Vec<View>,
    forward: Vec<View>,
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
                LibraryEvent::EditCover(id) => this.open_cover_editor(id, window, cx),
                LibraryEvent::EditDetails(id) => this.open_details(id, window, cx),
            });
        let mut shell = Self {
            library,
            reader: None,
            previous: None,
            cover_adjust: None,
            cover_editor: None,
            details: None,
            settings: None,
            back: Vec::new(),
            forward: Vec::new(),
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

    /// Opens a book as a new history entry.
    pub fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let view = View::Book(path);
        self.record(view.clone(), cx);
        self.show(view, window, cx);
    }

    fn current(&self, cx: &App) -> View {
        match &self.reader {
            Some(reader) => View::Book(reader.read(cx).path().to_path_buf()),
            None => View::Library,
        }
    }

    /// Adds the current view to the history before a change to `next`.
    fn record(&mut self, next: View, cx: &App) {
        let current = self.current(cx);
        if current != next {
            self.back.push(current);
        }
        self.forward.clear();
    }

    fn show(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        match view {
            View::Library => self.close(window, cx),
            View::Book(path) => self.show_book(path, window, cx),
        }
    }

    fn history_back(&mut self, _: &HistoryBack, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.is_some() {
            self.close_settings(window, cx);
        } else if self.details.is_some() {
            self.close_details(window, cx);
        } else if self.cover_adjust.is_some() {
            self.close_cover_adjust(window, cx);
        } else if self.cover_editor.is_some() {
            self.close_cover_editor(window, cx);
        } else if let Some(view) = self.back.pop() {
            self.forward.push(self.current(cx));
            self.show(view, window, cx);
        }
    }

    fn history_forward(&mut self, _: &HistoryForward, window: &mut Window, cx: &mut Context<Self>) {
        let overlay = self.settings.is_some()
            || self.details.is_some()
            || self.cover_adjust.is_some()
            || self.cover_editor.is_some();
        if !overlay && let Some(view) = self.forward.pop() {
            self.back.push(self.current(cx));
            self.show(view, window, cx);
        }
    }

    fn show_book(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let reader = match self
            .previous
            .take_if(|previous| previous.read(cx).path() == path)
        {
            Some(previous) => {
                if let Some(reader) = self.reader.take() {
                    reader.update(cx, |reader, cx| reader.close(window, cx));
                }
                previous
            }
            None => {
                if let Some(reader) = self.reader.take() {
                    if reader.read(cx).loading() {
                        reader.update(cx, |reader, cx| reader.close(window, cx));
                    } else {
                        self.previous = Some(reader);
                    }
                }
                self.library.update(cx, |library, _| library.book_opened());
                cx.new(|cx| Reader::new(path, cx))
            }
        };
        self._reader_subscriptions = vec![
            cx.subscribe_in(&reader, window, |this, _, event, window, cx| match event {
                ReaderEvent::Close => {
                    this.record(View::Library, cx);
                    this.close(window, cx);
                }
                ReaderEvent::Jumped => this.forward.clear(),
                ReaderEvent::HistoryBack => this.history_back(&HistoryBack, window, cx),
                ReaderEvent::HistoryForward => this.history_forward(&HistoryForward, window, cx),
            }),
            // The library or the previous book stays until the book is ready.
            cx.observe_in(&reader, window, |this, reader, window, cx| {
                if !reader.read(cx).loading() {
                    this.close_previous(window, cx);
                }
                cx.notify();
            }),
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
            |this, _, _: &CloseCoverAdjust, window, cx| this.close_cover_adjust(window, cx),
        );
        view.focus_handle(cx).focus(window, cx);
        self.cover_adjust = Some((view, subscription));
        cx.notify();
    }

    fn open_cover_editor(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = CoverEditor::open(id, window, cx) else {
            return;
        };
        let subscription = cx.subscribe_in(
            &view,
            window,
            |this, _, _: &CloseCoverEditor, window, cx| this.close_cover_editor(window, cx),
        );
        view.focus_handle(cx).focus(window, cx);
        self.cover_editor = Some((view, subscription));
        cx.notify();
    }

    fn open_details(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = DetailsDialog::open(id, window, cx) else {
            return;
        };
        let subscription =
            cx.subscribe_in(&dialog, window, |this, _, _: &CloseDetails, window, cx| {
                this.close_details(window, cx)
            });
        dialog.focus_handle(cx).focus(window, cx);
        self.details = Some((dialog, subscription));
        cx.notify();
    }

    fn close_cover_adjust(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cover_adjust = None;
        self.library.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn close_cover_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cover_editor = None;
        self.library.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn close_details(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.details = None;
        self.library.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.is_none() {
            let panel = cx.new(SettingsPanel::new);
            let subscription =
                cx.subscribe_in(&panel, window, |this, _, _: &CloseSettings, window, cx| {
                    this.close_settings(window, cx)
                });
            self.settings = Some((panel, subscription));
        }
        if let Some((panel, _)) = &self.settings {
            panel.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        match &self.reader {
            Some(reader) => reader.focus_handle(cx).focus(window, cx),
            None => self.library.focus_handle(cx).focus(window, cx),
        }
        cx.notify();
    }

    fn close_previous(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(previous) = self.previous.take() {
            previous.update(cx, |reader, cx| reader.close(window, cx));
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(reader) = self.reader.take() {
            reader.update(cx, |reader, cx| reader.close(window, cx));
            self.close_previous(window, cx);
            self.library.update(cx, |library, _| library.shown());
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
            // The Windows dialog cannot pick files and folders together.
            directories: cfg!(target_os = "macos"),
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
        let content = match (&self.reader, &self.previous) {
            (Some(reader), _) if !reader.read(cx).loading() => reader.clone().into_any_element(),
            (_, Some(previous)) => previous.clone().into_any_element(),
            (None, _) => {
                if let Some((view, _)) = &self.cover_adjust {
                    view.clone().into_any_element()
                } else if let Some((view, _)) = &self.cover_editor {
                    view.clone().into_any_element()
                } else {
                    self.library.clone().into_any_element()
                }
            }
            _ => self.library.clone().into_any_element(),
        };
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        let title_bar = crate::ui::title_bar::render(window, cx);
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let title_bar = None::<gpui::Empty>;
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .key_context("Shell")
            .on_action(cx.listener(Self::import_dialog))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::history_back))
            .on_action(cx.listener(Self::history_forward))
            .on_action(|_: &ToggleFullscreen, window, _| window.toggle_fullscreen())
            .on_action(|_: &CloseWindow, window, cx| {
                save_window_state(window, cx);
                window.remove_window();
            })
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
                    .children(self.settings.as_ref().map(|(panel, _)| panel.clone())),
            )
            .child(history_buttons())
    }
}

pub fn save_window_state(window: &Window, cx: &mut App) {
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

/// The back and forward buttons of a mouse do what Alt+Left and Alt+Right
/// do. The listener is on the window, so overlays that block the mouse do not
/// stop it.
fn history_buttons() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |_, _, window, _| {
            window.on_mouse_event(|event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let action: Box<dyn Action> = match event.button {
                    MouseButton::Navigate(NavigationDirection::Back) => Box::new(HistoryBack),
                    MouseButton::Navigate(NavigationDirection::Forward) => Box::new(HistoryForward),
                    _ => return,
                };
                window.defer(cx, move |window, cx| window.dispatch_action(action, cx));
            });
        },
    )
    .absolute()
    .size_0()
}

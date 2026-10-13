use std::path::PathBuf;

use gpui::{
    AppContext, BorrowAppContext, Context, Entity, ExternalPaths, Focusable, InteractiveElement,
    IntoElement, ParentElement, PathPromptOptions, Render, Styled, Subscription, Window, div,
};

use crate::Import;
use crate::library_view::{LibraryEvent, LibraryView};
use crate::reader::{Reader, ReaderEvent};
use crate::scan;
use crate::settings::{CloseSettings, OpenSettings, SettingsPanel};
use crate::store::Store;

/// The root view. It shows the library or one book.
pub struct Shell {
    library: Entity<LibraryView>,
    reader: Option<Entity<Reader>>,
    settings: Option<Entity<SettingsPanel>>,
    _settings_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    _reader_subscription: Option<Subscription>,
}

impl Shell {
    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = cx.new(|cx| LibraryView::new(window, cx));
        let subscription = cx.subscribe_in(&library, window, |this, _, event, window, cx| {
            let LibraryEvent::Open(path) = event;
            this.open(path.clone(), window, cx);
        });
        let mut shell = Self {
            library,
            reader: None,
            settings: None,
            _settings_subscription: None,
            _subscriptions: vec![subscription],
            _reader_subscription: None,
        };
        match path {
            Some(path) => shell.open(path, window, cx),
            None => shell.library.focus_handle(cx).focus(window, cx),
        }
        shell
    }

    fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        let reader = cx.new(|cx| Reader::new(Some(path), cx));
        self._reader_subscription =
            Some(
                cx.subscribe_in(&reader, window, |this, _, event, window, cx| match event {
                    ReaderEvent::Close => this.close(window, cx),
                }),
            );
        reader.focus_handle(cx).focus(window, cx);
        self.reader = Some(reader);
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
            self._reader_subscription = None;
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.reader {
            Some(reader) => reader.clone().into_any_element(),
            None => self.library.clone().into_any_element(),
        };
        div()
            .relative()
            .size_full()
            .key_context("Shell")
            .on_action(cx.listener(Self::import_dialog))
            .on_action(cx.listener(Self::open_settings))
            .on_drop(cx.listener(|_, paths: &ExternalPaths, _, cx| {
                scan::import(paths.paths().to_vec(), cx);
            }))
            .child(content)
            .children(self.settings.clone())
    }
}

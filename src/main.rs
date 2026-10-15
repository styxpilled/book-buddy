#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod library_view;
#[cfg(target_os = "windows")]
mod platform;
mod reader;
mod scan;
mod settings;
mod shell;
mod store;
mod ui;

use std::path::{self, PathBuf};

use book_buddy::library::WindowState;
use gpui::{
    App, AppContext, Bounds, DisplayId, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
    actions, point, px, size,
};
use gpui_kit::component::Root;

use shell::Shell;
use store::Store;

actions!(
    book_buddy,
    [Back, HistoryBack, HistoryForward, Import, ToggleFullscreen]
);

fn main() {
    env_logger::init();
    let arg = std::env::args_os()
        .nth(1)
        .map(|arg| path::absolute(&arg).unwrap_or_else(|_| PathBuf::from(arg)));
    #[cfg(target_os = "windows")]
    let Some(opened) = platform::instance::claim(arg.as_deref()) else {
        return;
    };
    let store = Store::load();
    let saved_window = store.library.window;
    let path = arg.or_else(|| store.last_book_path());

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            ui::style::init(cx);
            reader::init(cx);
            cx.set_global(store);
            settings::init(cx);
            cx.bind_keys([
                KeyBinding::new("escape", Back, Some("Reader")),
                KeyBinding::new("escape", Back, Some("CoverAdjust")),
                KeyBinding::new("escape", Back, Some("Details")),
                KeyBinding::new("alt-left", HistoryBack, None),
                KeyBinding::new("alt-right", HistoryForward, None),
                KeyBinding::new("ctrl-o", Import, None),
                KeyBinding::new("f11", ToggleFullscreen, None),
            ]);
            scan::start(cx);

            let (window_bounds, display_id) = restore_window(saved_window, cx);
            let options = WindowOptions {
                // The sim window stays behind the others.
                focus: !cfg!(feature = "sim"),
                titlebar: Some(TitlebarOptions {
                    // Scripts pick the window by this exact title, never the release build.
                    title: Some(
                        if cfg!(debug_assertions) {
                            "book-buddy dev"
                        } else {
                            "book-buddy"
                        }
                        .into(),
                    ),
                    // The app draws its own title bar.
                    appears_transparent: cfg!(target_os = "windows"),
                    ..Default::default()
                }),
                window_bounds: Some(window_bounds),
                display_id,
                ..Default::default()
            };
            let mut shell = None;
            let window = cx
                .open_window(options, |window, cx| {
                    let view = cx.new(|cx| Shell::new(path, window, cx));
                    shell = Some(view.downgrade());
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("cannot open the window");
            #[cfg(target_os = "windows")]
            if let Some(shell) = shell {
                open_from_later_starts(opened, shell, window, cx);
            }
            #[cfg(not(target_os = "windows"))]
            let _ = shell;
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            #[cfg(all(feature = "sim", target_os = "windows"))]
            platform::sim::serve(window.into(), cx);
            #[cfg(not(all(feature = "sim", target_os = "windows")))]
            {
                let _ = window;
                cx.activate(true);
            }
        });
}

/// The saved bounds and the display that has their center. A window that
/// would be off-screen opens on the main display, with the saved size.
fn restore_window(saved: Option<WindowState>, cx: &App) -> (WindowBounds, Option<DisplayId>) {
    let Some(saved) = saved else {
        let bounds = Bounds::centered(None, size(px(1000.), px(800.)), cx);
        return (WindowBounds::Windowed(bounds), None);
    };
    let bounds = Bounds::new(
        point(px(saved.x), px(saved.y)),
        size(px(saved.width), px(saved.height)),
    );
    let display = cx
        .displays()
        .into_iter()
        .find(|display| display.bounds().contains(&bounds.center()));
    let (bounds, display_id) = match display {
        Some(display) => (bounds, Some(display.id())),
        None => (Bounds::centered(None, bounds.size, cx), None),
    };
    let bounds = if saved.fullscreen {
        WindowBounds::Fullscreen(bounds)
    } else if saved.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    };
    (bounds, display_id)
}

/// A later start sends its path here. The window comes to the front and opens it.
#[cfg(target_os = "windows")]
fn open_from_later_starts(
    mut opened: platform::instance::Opened,
    shell: gpui::WeakEntity<Shell>,
    window: gpui::WindowHandle<Root>,
    cx: &mut App,
) {
    use futures::StreamExt;

    cx.spawn(async move |cx| {
        while let Some(path) = opened.next().await {
            window
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    if let Some(path) = path {
                        shell
                            .update(cx, |shell, cx| shell.open(path, window, cx))
                            .ok();
                    }
                })
                .ok();
        }
    })
    .detach();
}

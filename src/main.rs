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
    App, AppContext, Bounds, DisplayId, Global, KeyBinding, TitlebarOptions, WeakEntity,
    WindowBounds, WindowHandle, WindowOptions, actions, point, px, size,
};
use gpui_kit::component::Root;

use shell::Shell;
use store::Store;

actions!(
    book_buddy,
    [
        Back,
        CloseWindow,
        HistoryBack,
        HistoryForward,
        Import,
        Quit,
        ToggleFullscreen
    ]
);

/// The Windows scripts pick the window by its title, so it stays the same there.
const APP_NAME: &str = if cfg!(target_os = "macos") {
    "Book Buddy"
} else {
    "book-buddy"
};

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
    let path = arg.or_else(|| store.last_book_path());

    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    // Finder sends the files to open as URLs, also to a running app.
    #[cfg(target_os = "macos")]
    let opened = {
        let (send, opened) = futures::channel::mpsc::unbounded();
        app.on_open_urls(move |urls| {
            for path in urls.iter().filter_map(|url| file_path(url)) {
                send.unbounded_send(Some(path)).ok();
            }
        });
        opened
    };
    // On macOS the app stays open without a window. The Dock icon opens it again.
    #[cfg(target_os = "macos")]
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            show(None, cx);
        }
    });
    app.run(move |cx: &mut App| {
        gpui_kit::init(cx);
        ui::style::init(cx);
        reader::init(cx);
        cx.set_global(store);
        settings::init(cx);
        cx.bind_keys([
            KeyBinding::new("escape", Back, Some("Reader")),
            KeyBinding::new("escape", Back, Some("CoverAdjust")),
            KeyBinding::new("escape", Back, Some("CoverEditor")),
            KeyBinding::new("escape", Back, Some("Details")),
            KeyBinding::new("alt-left", HistoryBack, None),
            KeyBinding::new("alt-right", HistoryForward, None),
            KeyBinding::new("secondary-o", Import, None),
            KeyBinding::new("f11", ToggleFullscreen, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        save_windows_on_quit(cx);
        #[cfg(target_os = "macos")]
        mac_menus(cx);
        scan::start(cx);

        let window = open_main_window(path, cx);
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        open_from_later_starts(opened, cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() && !cfg!(target_os = "macos") {
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

/// The window and its shell, while the window is open.
struct MainWindow {
    shell: WeakEntity<Shell>,
    window: WindowHandle<Root>,
}

impl Global for MainWindow {}

fn open_main_window(path: Option<PathBuf>, cx: &mut App) -> WindowHandle<Root> {
    let (window_bounds, display_id) = restore_window(cx.global::<Store>().library.window, cx);
    let options = WindowOptions {
        // The sim window stays behind the others.
        focus: !cfg!(feature = "sim"),
        titlebar: Some(TitlebarOptions {
            // Scripts pick the window by this exact title, never the release build.
            title: Some(
                if cfg!(debug_assertions) {
                    format!("{APP_NAME} dev")
                } else {
                    APP_NAME.to_string()
                }
                .into(),
            ),
            // The app draws its own title bar.
            appears_transparent: cfg!(any(target_os = "windows", target_os = "macos")),
            // In the middle of the title bar that the app draws.
            traffic_light_position: cfg!(target_os = "macos").then(|| point(px(12.), px(9.))),
            ..Default::default()
        }),
        // The title bar moves the window itself, so clicks on it have no delay.
        app_owns_titlebar_drag: cfg!(target_os = "macos"),
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
    if let Some(shell) = shell {
        cx.set_global(MainWindow { shell, window });
    }
    window
}

/// Brings the window to the front and opens the book. If the window is
/// closed, opens a new one with the book, or with the last book.
fn show(mut path: Option<PathBuf>, cx: &mut App) {
    if let Some(main) = cx.try_global::<MainWindow>() {
        let (shell, window) = (main.shell.clone(), main.window);
        let shown = window.update(cx, |_, window, cx| {
            window.activate_window();
            if let Some(path) = path.take() {
                shell
                    .update(cx, |shell, cx| shell.open(path, window, cx))
                    .ok();
            }
        });
        if shown.is_ok() {
            #[cfg(target_os = "macos")]
            cx.activate(true);
            return;
        }
    }
    let path = path.or_else(|| cx.global::<Store>().last_book_path());
    open_main_window(path, cx);
    cx.activate(true);
}

/// Quitting from the menu, the Dock or a log out does not close the windows,
/// so their state is saved here.
fn save_windows_on_quit(cx: &mut App) {
    cx.on_app_quit(|cx| {
        for window in cx.windows() {
            if let Err(e) = window.update(cx, |_, window, cx| shell::save_window_state(window, cx))
            {
                log::error!("cannot save the window state: {e:#}");
            }
        }
        async {}
    })
    .detach();
}

#[cfg(target_os = "macos")]
fn mac_menus(cx: &mut App) {
    use gpui::{Menu, MenuItem, OsAction, SystemMenuType};

    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-[", HistoryBack, None),
        KeyBinding::new("cmd-]", HistoryForward, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullscreen, None),
    ]);
    cx.set_menus([
        Menu::new(APP_NAME).items([
            MenuItem::action("Settings...", settings::OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(format!("Quit {APP_NAME}"), Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Add Books...", Import),
            MenuItem::separator(),
            MenuItem::action("Close Window", CloseWindow),
        ]),
        Menu::new("Edit").items([MenuItem::os_action(
            "Copy",
            reader::CopySelection,
            OsAction::Copy,
        )]),
        Menu::new("View").items([
            MenuItem::action("Back", HistoryBack),
            MenuItem::action("Forward", HistoryForward),
            MenuItem::separator(),
            MenuItem::action("Toggle Full Screen", ToggleFullscreen),
        ]),
    ]);
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

/// A later start, or Finder on macOS, sends its path here.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn open_from_later_starts(
    mut opened: futures::channel::mpsc::UnboundedReceiver<Option<PathBuf>>,
    cx: &mut App,
) {
    use futures::StreamExt;

    cx.spawn(async move |cx| {
        while let Some(path) = opened.next().await {
            cx.update(|cx| show(path, cx));
        }
    })
    .detach();
}

#[cfg(target_os = "macos")]
fn file_path(url: &str) -> Option<PathBuf> {
    let path = url.strip_prefix("file://")?;
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .ok()?;
    Some(PathBuf::from(path.as_ref()))
}

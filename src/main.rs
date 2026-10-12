#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod image_cache;
mod library_view;
mod reader;
mod scan;
mod settings;
mod shell;
#[cfg(feature = "sim")]
mod sim;
mod store;

use std::path::{self, PathBuf};

use gpui::{
    App, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, actions, px,
    size,
};
use gpui_kit::component::Root;

use shell::Shell;
use store::Store;

actions!(book_buddy, [Back, Import]);

fn main() {
    env_logger::init();
    let store = Store::load();
    let path = std::env::args_os()
        .nth(1)
        .map(|arg| path::absolute(&arg).unwrap_or_else(|_| PathBuf::from(arg)))
        .or_else(|| store.last_book_path());

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            reader::init(cx);
            cx.set_global(store);
            settings::init(cx);
            cx.bind_keys([
                KeyBinding::new("escape", Back, Some("Reader")),
                KeyBinding::new("ctrl-o", Import, None),
            ]);
            scan::start(cx);

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
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1000.), px(800.)),
                    cx,
                ))),
                ..Default::default()
            };
            let window = cx
                .open_window(options, |window, cx| {
                    let shell = cx.new(|cx| Shell::new(path, window, cx));
                    cx.new(|cx| Root::new(shell, window, cx))
                })
                .expect("cannot open the window");
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            #[cfg(feature = "sim")]
            sim::serve(window.into(), cx);
            #[cfg(not(feature = "sim"))]
            {
                let _ = window;
                cx.activate(true);
            }
        });
}

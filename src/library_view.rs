use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use book_buddy::dither::{Adjust, dither};
use book_buddy::library::BookEntry;
use gpui::{
    App, AppContext, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ObjectFit, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Styled, StyledImage, Subscription, WeakEntity,
    Window, div, img, prelude::*, px, relative, uniform_list,
};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, Sizable};

mod backdrop;
pub(crate) mod cover_adjust;
pub(crate) mod details;
mod image_cache;

use crate::Import;
use crate::scan::{COVER_HEIGHT, COVER_WIDTH};
use crate::settings::OpenSettings;
use crate::store::{Store, cover_path};
use crate::ui::menu::{self, Dismiss, Menu, MenuItem};
use crate::ui::style;
use backdrop::backdrop;
use image_cache::{BoundedImageCache, Filter};

const MARGIN: f32 = 16.;
const GAP: f32 = 32.;
const GAP_TOP: f32 = 8.;
const TITLE_LINE: f32 = 16.;
const AUTHOR_LINE: f32 = 14.;
const COVER_GAP: f32 = 4.;
const CARD_BOTTOM: f32 = 8.;
/// The space under the cover, gaps and lines included, for a title of two lines.
const CARD_TEXT_HEIGHT: f32 = COVER_GAP + 2. + 2. * TITLE_LINE + 2. + AUTHOR_LINE + CARD_BOTTOM;
const CARD: &str = "card";

pub enum LibraryEvent {
    Open(PathBuf),
    /// Opens the cover adjustment screen for the book with this ID.
    AdjustCover(String),
    /// Opens the dialog to edit the title and author of the book with this ID.
    EditDetails(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    LastOpened,
    Title,
    Author,
    Added,
}

/// The title in lower case, without a leading article.
fn title_key(title: &str) -> String {
    let title = title.trim_start().to_lowercase();
    ["the ", "an ", "a "]
        .iter()
        .find_map(|article| title.strip_prefix(article))
        .map(|rest| rest.trim_start().to_string())
        .unwrap_or(title)
}

impl Sort {
    const ALL: [Sort; 4] = [Sort::LastOpened, Sort::Title, Sort::Author, Sort::Added];

    fn label(self) -> &'static str {
        match self {
            Sort::LastOpened => "LAST OPENED",
            Sort::Title => "TITLE",
            Sort::Author => "AUTHOR",
            Sort::Added => "DATE ADDED",
        }
    }

    fn apply(self, books: &mut [BookEntry]) {
        match self {
            Sort::LastOpened => books.sort_by_key(|b| Reverse((b.last_opened, b.added))),
            Sort::Title => books.sort_by_cached_key(|b| title_key(b.shown_title())),
            Sort::Author => books.sort_by_cached_key(|b| {
                (b.shown_author().to_lowercase(), title_key(b.shown_title()))
            }),
            Sort::Added => books.sort_by_key(|b| Reverse(b.added)),
        }
    }
}

pub struct LibraryView {
    filter: Entity<InputState>,
    sort: Sort,
    covers: Entity<BoundedImageCache>,
    /// The tones the covers are dithered to. They follow the theme.
    tones: ([u8; 3], [u8; 3]),
    /// The adjustments of the covers in `covers`, by cover path.
    adjusts: Arc<HashMap<PathBuf, Adjust>>,
    menu: Option<OpenMenu>,
    _subscriptions: Vec<Subscription>,
}

struct OpenMenu {
    menu: Entity<Menu>,
    /// At the pointer, or under the sort button.
    at: Option<Point<Pixels>>,
    _dismiss: Subscription,
}

impl EventEmitter<LibraryEvent> for LibraryView {}

impl Focusable for LibraryView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.filter.focus_handle(cx)
    }
}

impl LibraryView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("title or author"));
        let subscriptions = vec![
            cx.subscribe_in(&filter, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.observe_global::<Store>(|_, cx| cx.notify()),
        ];
        let tones = style::tones(cx);
        let adjusts = Arc::new(cover_adjusts(cx));
        Self {
            filter,
            sort: Sort::LastOpened,
            covers: cover_cache(tones, adjusts.clone(), window, cx),
            tones,
            adjusts,
            menu: None,
            _subscriptions: subscriptions,
        }
    }

    fn open_menu(
        &mut self,
        at: Option<Point<Pixels>>,
        items: Vec<MenuItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = cx.new(|cx| Menu::new(items, window, cx));
        let dismiss = cx.subscribe_in(&menu, window, |this, _, _: &Dismiss, window, cx| {
            this.menu = None;
            this.filter.focus_handle(cx).focus(window, cx);
            cx.notify();
        });
        self.menu = Some(OpenMenu {
            menu,
            at,
            _dismiss: dismiss,
        });
        cx.notify();
    }

    /// The open menu, if it is the sort menu or not as `sort` says.
    fn render_menu(&self, sort: bool, window: &Window) -> Option<impl IntoElement> {
        let open = self
            .menu
            .as_ref()
            .filter(|open| open.at.is_none() == sort)?;
        Some(menu::popup(&open.menu, open.at, window))
    }

    fn sort_items(&self, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let view = cx.entity().downgrade();
        Sort::ALL
            .into_iter()
            .map(|option| {
                let view = view.clone();
                MenuItem::new(option.label(), move |_, cx| {
                    view.update(cx, |this, cx| {
                        this.sort = option;
                        cx.notify();
                    })
                    .ok();
                })
                .checked(option == self.sort)
            })
            .collect()
    }

    fn visible_books(&self, cx: &App) -> Vec<BookEntry> {
        let filter = self.filter.read(cx).value().to_lowercase();
        let library = &cx.global::<Store>().library;
        let show_hidden = library.settings.show_hidden;
        let mut books: Vec<BookEntry> = library
            .books
            .iter()
            .filter(|b| show_hidden || !b.hidden)
            .filter(|b| {
                filter.is_empty()
                    || b.shown_title().to_lowercase().contains(&filter)
                    || b.shown_author().to_lowercase().contains(&filter)
            })
            .cloned()
            .collect();
        self.sort.apply(&mut books);
        books
    }

    fn render_header(
        &self,
        count: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(20.))
            .px(px(MARGIN))
            .pt(px(MARGIN))
            .pb(px(8.))
            .child(
                div()
                    .w(px(330.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    // .child(style::tag("FILTER", cx))
                    .child(style::input(&self.filter, cx)),
            )
            .child(
                div()
                    .relative()
                    .child(
                        Button::new("sort")
                            .label(self.sort.label())
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                let items = this.sort_items(cx);
                                this.open_menu(None, items, window, cx);
                            })),
                    )
                    .children(
                        self.render_menu(true, window).map(|menu| {
                            div().absolute().top_full().left_0().mt(px(4.)).child(menu)
                        }),
                    ),
            )
            .child(style::rule(cx))
            .child(style::dot(format!("{count:02}"), 26.))
            .child(
                style::button("add", "+ ADD BOOKS", true, cx)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Import), cx)),
            )
            .child(
                style::button("settings", "SETTINGS", false, cx)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx)),
            )
    }
}

fn columns(window: &Window) -> usize {
    (((grid_width(window) + GAP) / (COVER_WIDTH as f32 + GAP)).floor() as usize).max(1)
}

fn grid_width(window: &Window) -> f32 {
    f32::from(window.viewport_size().width) - 2. * MARGIN
}

/// The covers of two screens stay decoded, so a scroll back does not decode
/// them again. More covers would keep the memory of every book opened.
fn decoded_covers(window: &Window) -> usize {
    let height = f32::from(window.viewport_size().height);
    let rows = (height / (COVER_HEIGHT as f32 + CARD_TEXT_HEIGHT)).ceil() as usize + 1;
    2 * rows * columns(window)
}

fn cover_adjusts(cx: &App) -> HashMap<PathBuf, Adjust> {
    let store = cx.global::<Store>();
    store
        .library
        .books
        .iter()
        .filter(|b| !b.cover_adjust.is_none())
        .map(|b| (cover_path(store.covers(), &b.id), b.cover_adjust))
        .collect()
}

fn cover_cache(
    tones: ([u8; 3], [u8; 3]),
    adjusts: Arc<HashMap<PathBuf, Adjust>>,
    window: &Window,
    cx: &mut App,
) -> Entity<BoundedImageCache> {
    BoundedImageCache::new(decoded_covers(window), cover_filter(tones, adjusts), cx)
}

fn cover_filter(
    (dark, light): ([u8; 3], [u8; 3]),
    adjusts: Arc<HashMap<PathBuf, Adjust>>,
) -> Filter {
    Arc::new(move |path, image| {
        let adjust = adjusts.get(path).unwrap_or(&Adjust::NONE);
        dither(&image, style::DITHER_CELL, adjust, dark, light)
    })
}

impl Render for LibraryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tones = style::tones(cx);
        let adjusts = cover_adjusts(cx);
        if tones != self.tones {
            self.tones = tones;
            self.adjusts = Arc::new(adjusts);
            self.covers = cover_cache(tones, self.adjusts.clone(), window, cx);
        } else if adjusts != *self.adjusts {
            let changed: Vec<PathBuf> = adjusts
                .keys()
                .chain(self.adjusts.keys())
                .filter(|path| adjusts.get(*path) != self.adjusts.get(*path))
                .cloned()
                .collect();
            self.adjusts = Arc::new(adjusts);
            let filter = cover_filter(tones, self.adjusts.clone());
            self.covers
                .update(cx, |covers, cx| covers.set_filter(filter, &changed, cx));
        }
        let books = Rc::new(self.visible_books(cx));
        let count = books.len();
        let capacity = decoded_covers(window);
        self.covers
            .update(cx, |covers, _| covers.set_capacity(capacity));
        let theme = cx.theme();
        let columns = columns(window);
        let rows = books.len().div_ceil(columns);
        // The room left after the columns goes to both sides of the grid.
        let used = columns as f32 * (COVER_WIDTH as f32 + GAP) - GAP;
        let side = MARGIN + (grid_width(window) - used).max(0.) / 2.;

        let body = if books.is_empty() {
            let empty = cx.global::<Store>().library.books.is_empty();
            let message = if empty {
                "No books yet. Drop .epub files here, or press Ctrl+O."
            } else {
                "No books match."
            };
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child(message)
                .into_any_element()
        } else {
            let view = cx.entity().downgrade();
            let covers = self.covers.clone();
            // `uniform_list` lays out its rows in prepaint, where an
            // `image_cache` parent is not in scope. Without its own cache, an
            // `img` loads into the app cache, which keeps every image.
            div()
                .flex_1()
                .child(
                    uniform_list("books", rows, move |range, _, cx| {
                        range
                            .map(|row| {
                                let start = row * columns;
                                let end = (start + columns).min(books.len());
                                div().flex().gap(px(GAP)).px(px(side)).children(
                                    books[start..end]
                                        .iter()
                                        .map(|book| card(book, &covers, view.clone(), cx)),
                                )
                            })
                            .collect()
                    })
                    .size_full()
                    .pt(px(GAP_TOP))
                    .pb(px(MARGIN)),
                )
                .into_any_element()
        };

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(backdrop(window, cx))
            .child(self.render_header(count, window, cx))
            .child(body)
            .children(self.render_menu(false, window))
    }
}

fn card(
    book: &BookEntry,
    covers: &Entity<BoundedImageCache>,
    view: WeakEntity<LibraryView>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let store = cx.global::<Store>();
    let cover = cover_path(store.covers(), &book.id);
    let has_cover = cover.exists();
    let progress = div()
        .h(px(18.))
        .px(px(5.))
        .flex()
        .items_center()
        .gap(px(6.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .child(style::progress(book.percent, cx))
        .child(style::dot(format!("{:.0}%", book.percent), 13.));

    let cover = div()
        .relative()
        .flex_none()
        .w(px(COVER_WIDTH as f32))
        .h(px(COVER_HEIGHT as f32))
        .mb(px(COVER_GAP))
        .overflow_hidden()
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted)
        .map(|frame| {
            if has_cover {
                frame.child(
                    img(cover)
                        .image_cache(covers)
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
            } else {
                frame
                    .flex()
                    .items_center()
                    .justify_center()
                    .p(px(10.))
                    .child(style::cross(style::faint(cx)))
                    .child(
                        div()
                            .relative()
                            .p(px(6.))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.muted)
                            .text_size(px(10.))
                            .line_height(relative(1.3))
                            .text_center()
                            .child(book.shown_title().to_uppercase()),
                    )
            }
        })
        .when(book.missing || book.hidden, |cover| {
            cover.child(
                div()
                    .absolute()
                    .left(px(6.))
                    .top(px(6.))
                    .flex()
                    .gap(px(6.))
                    .when(book.missing, |row| {
                        row.child(style::tag("FILE MISSING", cx))
                    })
                    .when(book.hidden, |row| row.child(style::tag("HIDDEN", cx))),
            )
        })
        .child(on_hover(progress.absolute().left(px(6.)).bottom(px(6.))))
        .when(book.last_opened.is_none(), |cover| {
            cover.child(on_hover(
                style::tag("NEW", cx)
                    .absolute()
                    .right(px(6.))
                    .bottom(px(6.)),
            ))
        });

    let id = book.id.clone();
    let path = book.path.clone();
    let missing = book.missing;
    let hidden = book.hidden;
    let card = div()
        .id(SharedString::from(book.id.clone()))
        .group(CARD)
        .flex()
        .flex_col()
        .gap(px(2.))
        .when(missing || hidden, |card| card.opacity(0.42))
        .when(!missing, |card| card.cursor_pointer())
        .child(cover)
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(TITLE_LINE))
                .max_h(px(2. * TITLE_LINE))
                .overflow_hidden()
                .child(book.shown_title().to_string()),
        )
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(AUTHOR_LINE))
                .text_color(theme.muted_foreground)
                .truncate()
                .child(book.shown_author().to_string()),
        )
        .on_click({
            let view = view.clone();
            let path = path.clone();
            move |_, _, cx| {
                if !missing {
                    view.update(cx, |_, cx| cx.emit(LibraryEvent::Open(path.clone())))
                        .ok();
                }
            }
        })
        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
            let items = book_items(&id, &path, hidden, missing, has_cover, view.clone());
            view.update(cx, |this, cx| {
                this.open_menu(Some(event.position), items, window, cx)
            })
            .ok();
        });

    // A fixed height keeps the rows even. Only the card in it is clickable.
    div()
        .flex_none()
        .w(px(COVER_WIDTH as f32))
        .h(px(COVER_HEIGHT as f32 + CARD_TEXT_HEIGHT))
        .child(card)
}

fn book_items(
    id: &str,
    path: &Path,
    hidden: bool,
    missing: bool,
    has_cover: bool,
    view: WeakEntity<LibraryView>,
) -> Vec<MenuItem> {
    let (hide_id, remove_id) = (id.to_string(), id.to_string());
    let (adjust_id, details_id) = (id.to_string(), id.to_string());
    let details_view = view.clone();
    let mut items = vec![
        MenuItem::new(if hidden { "UNHIDE" } else { "HIDE" }, move |_, cx| {
            cx.update_global(|store: &mut Store, _| {
                if let Some(book) = store.library.book_mut(&hide_id) {
                    book.hidden = !hidden;
                    store.save();
                }
            });
        }),
        MenuItem::new("EDIT DETAILS", move |_, cx| {
            details_view
                .update(cx, |_, cx| {
                    cx.emit(LibraryEvent::EditDetails(details_id.clone()))
                })
                .ok();
        }),
        MenuItem::new("ADJUST COVER", move |_, cx| {
            view.update(cx, |_, cx| {
                cx.emit(LibraryEvent::AdjustCover(adjust_id.clone()))
            })
            .ok();
        })
        .disabled(!has_cover),
        MenuItem::new("REMOVE FROM LIBRARY", move |_, cx| {
            cx.update_global(|store: &mut Store, _| store.remove_book(&remove_id));
        }),
    ];
    #[cfg(target_os = "windows")]
    items.push(show_in_explorer(path).disabled(missing));
    #[cfg(not(target_os = "windows"))]
    let _ = (path, missing);
    items
}

#[cfg(target_os = "windows")]
fn show_in_explorer(path: &Path) -> MenuItem {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let select = format!("/select,\"{}\"", path.display());
    MenuItem::new("SHOW IN EXPLORER", move |_, _| {
        if let Err(e) = Command::new("explorer").raw_arg(&select).spawn() {
            log::error!("cannot start Explorer: {e}");
        }
    })
}

/// Shown only while the pointer is on the card.
fn on_hover(element: Div) -> Div {
    element
        .opacity(0.)
        .group_hover(CARD, |style| style.opacity(1.))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_key_skips_the() {
        assert_eq!(
            title_key("The Picture of Dorian Gray"),
            "picture of dorian gray"
        );
        assert_eq!(title_key("  THE  Hound"), "hound");
        assert_eq!(title_key("Theatre"), "theatre");
        assert_eq!(title_key("The"), "the");
        assert_eq!(title_key("A Tale of Two Cities"), "tale of two cities");
        assert_eq!(title_key("An Ideal Husband"), "ideal husband");
        assert_eq!(title_key("Anna Karenina"), "anna karenina");
        assert_eq!(title_key("A"), "a");
    }

    #[test]
    fn sorts_are_stable() {
        let entry = |id: &str, title: &str, author: &str| -> BookEntry {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "path": format!("{id}.epub"),
                "size": 0,
                "modified": "2026-01-01T00:00:00Z",
                "title": title,
                "author": author,
                "added": "2026-01-01T00:00:00Z",
            }))
            .unwrap()
        };
        let mut custom = entry("d", "Zebra", "Austen");
        custom.custom_title = Some("Emma".into());
        let books = vec![
            entry("f", "Anna", "austen"),
            entry("b", "hound", "Doyle"),
            custom,
            entry("a", "The Hound", "Doyle"),
            entry("e", "An Ideal Husband", "Wilde"),
            entry("c", "A Study", "Doyle"),
        ];
        let sorted = |sort: Sort| {
            let mut books = books.clone();
            sort.apply(&mut books);
            books.into_iter().map(|b| b.id).collect::<Vec<_>>()
        };
        assert_eq!(sorted(Sort::Title), ["f", "d", "b", "a", "e", "c"]);
        assert_eq!(sorted(Sort::Author), ["f", "d", "b", "a", "c", "e"]);
    }
}

use std::cmp::Reverse;
use std::collections::HashMap;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use book_buddy::cover;
use book_buddy::dither::{Adjust, dither};
use book_buddy::library::BookEntry;
use gpui::{
    App, AppContext, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ObjectFit, ParentElement, Pixels, Point, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, StyledImage, Subscription,
    UniformListScrollHandle, WeakEntity, Window, div, img, prelude::*, px, uniform_list,
};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollbarHandle;
use gpui_kit::component::{ActiveTheme, Sizable};

mod backdrop;
pub(crate) mod cover_adjust;
pub(crate) mod cover_editor;
pub(crate) mod details;
mod image_cache;

use crate::Import;
use crate::scan::{COVER_HEIGHT, COVER_WIDTH};
use crate::settings::OpenSettings;
use crate::store::{Store, cover_path, is_text_cover, text_cover_path};
use crate::ui::autoscroll::{self, Autoscroll};
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
    /// Opens the editor of the generated cover of the book with this ID.
    EditCover(String),
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
    /// The adjustment and the revision (see `Store::cover_revision`) of each
    /// cover file that has one of them.
    looks: Arc<HashMap<PathBuf, Look>>,
    menu: Option<OpenMenu>,
    /// The scroll stays while a book or cover adjust is on screen.
    scroll: UniformListScrollHandle,
    /// A book opened in the last opened order, so it is now the first card.
    scroll_to_top: bool,
    autoscroll: Option<Autoscroll>,
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
        let looks = Arc::new(cover_looks(cx));
        Self {
            filter,
            sort: Sort::LastOpened,
            covers: cover_cache(tones, looks.clone(), window, cx),
            tones,
            looks,
            menu: None,
            scroll: UniformListScrollHandle::new(),
            scroll_to_top: false,
            autoscroll: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn book_opened(&mut self) {
        if self.sort == Sort::LastOpened {
            self.scroll_to_top = true;
        }
    }

    /// The library is on screen again after a book.
    pub fn shown(&mut self) {
        if mem::take(&mut self.scroll_to_top) {
            self.scroll.scroll_to_item(0, ScrollStrategy::Top);
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

    fn autoscroll_frame(&mut self, window: &Window) {
        let Some(step) = self.autoscroll.as_mut().map(|a| a.step(window)) else {
            return;
        };
        let max = (self.scroll.content_size().height - self.scroll.viewport_bounds().size.height)
            .max(px(0.));
        let mut offset = self.scroll.offset();
        offset.y = (offset.y - step).max(-max).min(px(0.));
        self.scroll.set_offset(offset);
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

type Look = (Adjust, u32);

fn cover_looks(cx: &App) -> HashMap<PathBuf, Look> {
    let store = cx.global::<Store>();
    let mut looks = HashMap::new();
    for book in &store.library.books {
        if !book.cover_adjust.is_none() {
            looks.insert(cover_path(store.covers(), &book.id), (book.cover_adjust, 0));
        }
        let revision = store.cover_revision(&book.id);
        if revision > 0 {
            looks.insert(
                text_cover_path(store.covers(), &book.id),
                (Adjust::NONE, revision),
            );
        }
    }
    looks
}

fn cover_cache(
    tones: ([u8; 3], [u8; 3]),
    looks: Arc<HashMap<PathBuf, Look>>,
    window: &Window,
    cx: &mut App,
) -> Entity<BoundedImageCache> {
    BoundedImageCache::new(decoded_covers(window), cover_filter(tones, looks), cx)
}

fn cover_filter((dark, light): ([u8; 3], [u8; 3]), looks: Arc<HashMap<PathBuf, Look>>) -> Filter {
    Arc::new(move |path, image| {
        if is_text_cover(path) {
            return cover::colorize(image, dark, light);
        }
        let adjust = looks.get(path).map_or(&Adjust::NONE, |(adjust, _)| adjust);
        dither(&image, style::DITHER_CELL, adjust, dark, light)
    })
}

impl Render for LibraryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.autoscroll_frame(window);
        let tones = style::tones(cx);
        let looks = cover_looks(cx);
        if tones != self.tones {
            self.tones = tones;
            self.looks = Arc::new(looks);
            self.covers = cover_cache(tones, self.looks.clone(), window, cx);
        } else if looks != *self.looks {
            let changed: Vec<PathBuf> = looks
                .keys()
                .chain(self.looks.keys())
                .filter(|path| looks.get(*path) != self.looks.get(*path))
                .cloned()
                .collect();
            self.looks = Arc::new(looks);
            let filter = cover_filter(tones, self.looks.clone());
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
                if cfg!(target_os = "macos") {
                    "No books yet. Drop .epub files here, or press Cmd+O."
                } else {
                    "No books yet. Drop .epub files here, or press Ctrl+O."
                }
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
                .relative()
                .flex_1()
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                        this.autoscroll = Some(Autoscroll::new(event.position));
                        cx.notify();
                    }),
                )
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
                    .track_scroll(&self.scroll)
                    .size_full()
                    .pt(px(GAP_TOP))
                    .pb(px(MARGIN)),
                )
                .child(autoscroll::scrollbar(&self.scroll))
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
            .children(
                self.autoscroll
                    .is_some()
                    .then(|| autoscroll::overlay(|this: &mut Self| &mut this.autoscroll, cx)),
            )
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
    let shown = store.shown_cover(&book.id);
    let has_image = shown.as_deref().is_some_and(|path| !is_text_cover(path));
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
        .children(shown.map(|path| {
            img(path)
                .image_cache(covers)
                .size_full()
                .object_fit(ObjectFit::Cover)
        }))
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
            let items = book_items(&id, &path, hidden, missing, has_image, view.clone());
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
    has_image: bool,
    view: WeakEntity<LibraryView>,
) -> Vec<MenuItem> {
    let (hide_id, remove_id) = (id.to_string(), id.to_string());
    let (cover_id, details_id) = (id.to_string(), id.to_string());
    let details_view = view.clone();
    let (cover_label, cover_event): (_, fn(String) -> LibraryEvent) = if has_image {
        ("ADJUST COVER", LibraryEvent::AdjustCover)
    } else {
        ("EDIT COVER", LibraryEvent::EditCover)
    };
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
        MenuItem::new(cover_label, move |_, cx| {
            view.update(cx, |_, cx| cx.emit(cover_event(cover_id.clone())))
                .ok();
        }),
        MenuItem::new("REMOVE FROM LIBRARY", move |_, cx| {
            cx.update_global(|store: &mut Store, _| store.remove_book(&remove_id));
        }),
    ];
    #[cfg(target_os = "windows")]
    items.push(show_in_explorer(path).disabled(missing));
    #[cfg(target_os = "macos")]
    items.push(show_in_finder(path).disabled(missing));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
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

#[cfg(target_os = "macos")]
fn show_in_finder(path: &Path) -> MenuItem {
    let path = path.to_path_buf();
    MenuItem::new("SHOW IN FINDER", move |_, _| {
        if let Err(e) = std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
        {
            log::error!("cannot start Finder: {e}");
        }
    })
}

use std::cmp::Reverse;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

use book_buddy::dither::dither;
use book_buddy::library::BookEntry;
use gpui::{
    App, AppContext, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ObjectFit, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, StyledImage, Subscription, WeakEntity, Window, div,
    image_cache, img, prelude::*, px, relative, uniform_list,
};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Sizable};

use crate::Import;
use crate::image_cache::{BoundedImageCache, Filter};
use crate::scan::{COVER_HEIGHT, COVER_WIDTH};
use crate::settings::OpenSettings;
use crate::store::{Store, cover_path};
use crate::style;

const MARGIN: f32 = 32.;
const GAP: f32 = 32.;
const TITLE_LINE: f32 = 16.;
const AUTHOR_LINE: f32 = 14.;
const COVER_GAP: f32 = 4.;
const CARD_BOTTOM: f32 = 4.;
/// The space under the cover, gaps and lines included, for a title of two lines.
const CARD_TEXT_HEIGHT: f32 = COVER_GAP + 2. + 2. * TITLE_LINE + 2. + AUTHOR_LINE + CARD_BOTTOM;
const DECODED_COVERS: usize = 150;
const CARD: &str = "card";

pub enum LibraryEvent {
    Open(PathBuf),
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    LastOpened,
    Title,
    Author,
    Added,
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
            Sort::Title => books.sort_by_key(|b| b.title.to_lowercase()),
            Sort::Author => {
                books.sort_by_key(|b| (b.author.to_lowercase(), b.title.to_lowercase()))
            }
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
    _subscriptions: Vec<Subscription>,
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
        Self {
            filter,
            sort: Sort::LastOpened,
            covers: cover_cache(tones, cx),
            tones,
            _subscriptions: subscriptions,
        }
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
                    || b.title.to_lowercase().contains(&filter)
                    || b.author.to_lowercase().contains(&filter)
            })
            .cloned()
            .collect();
        self.sort.apply(&mut books);
        books
    }

    fn render_header(&self, count: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let sort = self.sort;
        div()
            .flex()
            .items_center()
            .gap(px(20.))
            .px(px(MARGIN))
            .pt(px(18.))
            .pb(px(4.))
            .child(
                div()
                    .w(px(330.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(style::tag("FILTER", cx))
                    .child(
                        div()
                            .flex_1()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(Input::new(&self.filter).appearance(false).cleanable(true)),
                    ),
            )
            .child(
                Button::new("sort")
                    .label(sort.label())
                    .ghost()
                    .small()
                    .dropdown_menu(move |mut menu, _, _| {
                        for option in Sort::ALL {
                            let view = view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(option.label())
                                    .checked(option == sort)
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |this, cx| {
                                            this.sort = option;
                                            cx.notify();
                                        })
                                        .ok();
                                    }),
                            );
                        }
                        menu
                    }),
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

fn cover_cache((dark, light): ([u8; 3], [u8; 3]), cx: &mut App) -> Entity<BoundedImageCache> {
    let filter: Filter = Arc::new(move |image| dither(&image, style::DITHER_CELL, dark, light));
    BoundedImageCache::new(DECODED_COVERS, Some(filter), cx)
}

impl Render for LibraryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tones = style::tones(cx);
        if tones != self.tones {
            self.tones = tones;
            self.covers = cover_cache(tones, cx);
        }
        let books = Rc::new(self.visible_books(cx));
        let count = books.len();
        let theme = cx.theme();
        let width = f32::from(window.viewport_size().width) - 2. * MARGIN;
        let columns = (((width + GAP) / (COVER_WIDTH as f32 + GAP)).floor() as usize).max(1);
        let rows = books.len().div_ceil(columns);

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
            image_cache(self.covers.clone())
                .flex_1()
                .child(
                    uniform_list("books", rows, move |range, _, cx| {
                        range
                            .map(|row| {
                                let start = row * columns;
                                let end = (start + columns).min(books.len());
                                div().flex().gap(px(GAP)).px(px(MARGIN)).children(
                                    books[start..end]
                                        .iter()
                                        .map(|book| card(book, view.clone(), cx)),
                                )
                            })
                            .collect()
                    })
                    .size_full()
                    .pt(px(GAP)),
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
            .child(style::grid(cx))
            .child(self.render_header(count, cx))
            .child(body)
    }
}

fn card(book: &BookEntry, view: WeakEntity<LibraryView>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let store = cx.global::<Store>();
    let cover = cover_path(store.covers(), &book.id);
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
            if cover.exists() {
                frame.child(img(cover).size_full().object_fit(ObjectFit::Cover))
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
                            .child(book.title.to_uppercase()),
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
                .child(book.title.clone()),
        )
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(AUTHOR_LINE))
                .text_color(theme.muted_foreground)
                .truncate()
                .child(book.author.clone()),
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
        .context_menu(move |menu, _, _| {
            let hide_id = id.clone();
            let remove_id = id.clone();
            let shown = path.clone();
            menu.item(
                PopupMenuItem::new(if hidden { "UNHIDE" } else { "HIDE" }).on_click(
                    move |_, _, cx| {
                        cx.update_global(|store: &mut Store, _| {
                            if let Some(book) = store.library.book_mut(&hide_id) {
                                book.hidden = !hidden;
                                store.save();
                            }
                        });
                    },
                ),
            )
            .item(
                PopupMenuItem::new("REMOVE FROM LIBRARY").on_click(move |_, _, cx| {
                    cx.update_global(|store: &mut Store, _| {
                        if let Some(removed) = store.library.remove(&remove_id) {
                            std::fs::remove_file(cover_path(store.covers(), &removed.id)).ok();
                        }
                        store.save();
                    });
                }),
            )
            .item(
                PopupMenuItem::new("SHOW IN EXPLORER")
                    .disabled(missing)
                    .on_click(move |_, _, _| {
                        let select = format!("/select,\"{}\"", shown.display());
                        if let Err(e) = Command::new("explorer").raw_arg(select).spawn() {
                            log::error!("cannot start Explorer: {e}");
                        }
                    }),
            )
        });

    // A fixed height keeps the rows even. Only the card in it is clickable.
    div()
        .flex_none()
        .w(px(COVER_WIDTH as f32))
        .h(px(COVER_HEIGHT as f32 + CARD_TEXT_HEIGHT))
        .child(card)
}

/// Shown only while the pointer is on the card.
fn on_hover(element: Div) -> Div {
    element
        .opacity(0.)
        .group_hover(CARD, |style| style.opacity(1.))
}

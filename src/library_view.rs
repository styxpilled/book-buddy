use std::cmp::Reverse;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use book_buddy::library::BookEntry;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ObjectFit, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, StyledImage, Subscription, WeakEntity, Window, div, image_cache, img, prelude::*, px,
    uniform_list,
};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Sizable};

use crate::Import;
use crate::image_cache::BoundedImageCache;
use crate::scan::{COVER_HEIGHT, COVER_WIDTH};
use crate::store::{Store, cover_path};

const MARGIN: f32 = 32.;
const GAP: f32 = 24.;
const CARD_TEXT_HEIGHT: f32 = 76.;
const DECODED_COVERS: usize = 150;

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
            Sort::LastOpened => "Last opened",
            Sort::Title => "Title",
            Sort::Author => "Author",
            Sort::Added => "Date added",
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
    show_hidden: bool,
    covers: Entity<BoundedImageCache>,
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
        let filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter by title or author"));
        let subscriptions = vec![
            cx.subscribe_in(&filter, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.observe_global::<Store>(|_, cx| cx.notify()),
        ];
        Self {
            filter,
            sort: Sort::LastOpened,
            show_hidden: false,
            covers: BoundedImageCache::new(DECODED_COVERS, cx),
            _subscriptions: subscriptions,
        }
    }

    fn visible_books(&self, cx: &App) -> Vec<BookEntry> {
        let filter = self.filter.read(cx).value().to_lowercase();
        let mut books: Vec<BookEntry> = cx
            .global::<Store>()
            .library
            .books
            .iter()
            .filter(|b| self.show_hidden || !b.hidden)
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

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let sort = self.sort;
        div()
            .flex()
            .items_center()
            .gap_3()
            .px(px(MARGIN))
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .w(px(320.))
                    .child(Input::new(&self.filter).cleanable(true)),
            )
            .child(
                Button::new("sort")
                    .label(format!("Sort: {}", sort.label()))
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
            .child(
                Checkbox::new("show-hidden")
                    .label("Show hidden")
                    .small()
                    .checked(self.show_hidden)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.show_hidden = *checked;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .child(
                Button::new("add")
                    .label("Add books")
                    .small()
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Import), cx)),
            )
    }
}

impl Render for LibraryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let books = Rc::new(self.visible_books(cx));
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
                                div()
                                    .flex()
                                    .gap(px(GAP))
                                    .px(px(MARGIN))
                                    .pt(px(GAP))
                                    .children(
                                        books[start..end]
                                            .iter()
                                            .map(|book| card(book, view.clone(), cx)),
                                    )
                            })
                            .collect()
                    })
                    .size_full(),
                )
                .into_any_element()
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_toolbar(cx))
            .child(body)
    }
}

fn card(book: &BookEntry, view: WeakEntity<LibraryView>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let store = cx.global::<Store>();
    let cover = cover_path(store.covers(), &book.id);
    let status: SharedString = if book.missing {
        "File missing".into()
    } else if book.last_opened.is_none() {
        "New".into()
    } else {
        format!("{:.0}%", book.percent).into()
    };

    let cover = div()
        .w(px(COVER_WIDTH as f32))
        .h(px(COVER_HEIGHT as f32))
        .rounded_md()
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
                    .p_3()
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .child(book.title.clone())
            }
        });

    let id = book.id.clone();
    let path = book.path.clone();
    let missing = book.missing;
    let hidden = book.hidden;
    div()
        .id(SharedString::from(book.id.clone()))
        .w(px(COVER_WIDTH as f32))
        .h(px(COVER_HEIGHT as f32 + CARD_TEXT_HEIGHT))
        .flex()
        .flex_col()
        .gap_1()
        .when(missing || hidden, |card| card.opacity(0.45))
        .when(!missing, |card| card.cursor_pointer())
        .child(cover)
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .line_clamp(2)
                .child(book.title.clone()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .truncate()
                .child(book.author.clone()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(status),
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
                PopupMenuItem::new(if hidden { "Unhide" } else { "Hide" }).on_click(
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
                PopupMenuItem::new("Remove from library").on_click(move |_, _, cx| {
                    cx.update_global(|store: &mut Store, _| {
                        if let Some(removed) = store.library.remove(&remove_id) {
                            std::fs::remove_file(cover_path(store.covers(), &removed.id)).ok();
                        }
                        store.save();
                    });
                }),
            )
            .item(
                PopupMenuItem::new("Show in Explorer")
                    .disabled(missing)
                    .on_click(move |_, _, _| {
                        let select = format!("/select,\"{}\"", shown.display());
                        if let Err(e) = Command::new("explorer").raw_arg(select).spawn() {
                            log::error!("cannot start Explorer: {e}");
                        }
                    }),
            )
        })
}

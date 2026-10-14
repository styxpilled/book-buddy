//! A dialog to edit the title and author of a book. The values go in the
//! library, not in the EPUB file.

use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, div, px,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use crate::Back;
use crate::store::Store;
use crate::ui::menu;
use crate::ui::style;

const PANEL_WIDTH: f32 = 500.;
const LABEL_WIDTH: f32 = 80.;

pub struct CloseDetails;

pub struct DetailsDialog {
    id: String,
    title: Entity<InputState>,
    author: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<CloseDetails> for DetailsDialog {}

impl Focusable for DetailsDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title.focus_handle(cx)
    }
}

impl DetailsDialog {
    /// `None` if the library has no book with this ID.
    pub fn open(id: &str, window: &mut Window, cx: &mut App) -> Option<Entity<Self>> {
        let book = cx.global::<Store>().library.book(id)?.clone();
        Some(cx.new(|cx| {
            // An empty field shows the value from the file, which is what it gives.
            let mut field = |value: &str, from_file: &str, cx: &mut Context<Self>| {
                let (value, from_file) = (value.to_string(), from_file.to_string());
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(from_file)
                        .default_value(value)
                })
            };
            let title = field(book.shown_title(), &book.title, cx);
            let author = field(book.shown_author(), &book.author, cx);
            let subscriptions = [&title, &author]
                .map(|input| {
                    cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::PressEnter { .. }) {
                            this.save(cx);
                        }
                    })
                })
                .into();
            Self {
                id: book.id,
                title,
                author,
                _subscriptions: subscriptions,
            }
        }))
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let title = self.title.read(cx).value();
        let author = self.author.read(cx).value();
        cx.update_global(|store: &mut Store, _| {
            if let Some(book) = store.library.book_mut(&self.id) {
                book.set_details(&title, &author);
                store.save();
            }
        });
        cx.emit(CloseDetails);
    }
}

fn field(label: &'static str, input: &Entity<InputState>, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(style::label(label, cx).flex_none().w(px(LABEL_WIDTH)))
        .child(
            div()
                .flex_1()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Input::new(input).appearance(false).cleanable(true)),
        )
}

impl Render for DetailsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let panel = div()
            .id("details-panel")
            .relative()
            .w(px(PANEL_WIDTH))
            .max_w_full()
            .flex()
            .flex_col()
            .gap(px(14.))
            .pt(px(22.))
            .px(px(24.))
            .pb(px(20.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .occlude()
            .children(style::crop_marks(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(style::tag_inverted("EDIT DETAILS", cx))
                    .child(style::rule(cx)),
            )
            .child(field("TITLE =", &self.title, cx))
            .child(field("AUTHOR =", &self.author, cx))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(10.))
                    .pt(px(6.))
                    .child(
                        style::button("cancel-details", "ESC", false, cx)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(CloseDetails))),
                    )
                    .child(
                        style::button("save-details", "SAVE", true, cx)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    ),
            );

        let view = cx.entity().downgrade();
        menu::shield("details", move |_, cx| {
            view.update(cx, |_, cx| cx.emit(CloseDetails)).ok();
        })
        .key_context("Details")
        .on_action(cx.listener(|_, _: &Back, _, cx| cx.emit(CloseDetails)))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .bg(theme.overlay)
        .child(panel)
    }
}

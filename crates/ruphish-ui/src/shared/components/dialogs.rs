use super::{ActionButton, ButtonStyle};
use crate::feedback::Message;
use leptos::{html, prelude::*};

/// Use native modal focus containment and make the background inert.
pub(crate) fn modal_node() -> NodeRef<html::Dialog> {
    let node = NodeRef::<html::Dialog>::new();
    Effect::new(move |_| {
        if let Some(dialog) = node.get() {
            let _ = dialog.show_modal();
        }
    });
    node
}

#[component]
pub(crate) fn DeleteConfirmation(
    title: String,
    #[prop(into)] description: String,
    confirm_label: &'static str,
    #[prop(into)] busy: Signal<bool>,
    on_confirm: Callback<()>,
    on_cancel: Callback<()>,
    message: Message,
) -> impl IntoView {
    let accessible_title = title.clone();
    let dialog = modal_node();
    let cancel = move || {
        if !busy.get_untracked() {
            if let Some(element) = dialog.get_untracked() {
                element.close();
            }
            on_cancel.run(());
        }
    };
    view! { <dialog node_ref=dialog class="management-dialog delete-confirmation" aria-label=accessible_title
        on:cancel=move |event: leptos::ev::Event| {event.prevent_default(); cancel()}>
        <h2>{title}</h2><p>{description}</p>{message.view()}
        <div class="confirmation-actions">
            <ActionButton style=ButtonStyle::Cancel autofocus=true disabled=busy on_click=move |_|cancel()>"Cancel"</ActionButton>
            <ActionButton style=ButtonStyle::Danger disabled=busy on_click=move |_|on_confirm.run(())>{confirm_label}</ActionButton>
        </div>
    </dialog> }
}

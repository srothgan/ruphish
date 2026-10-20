use super::{ActionButton, ButtonStyle};
use crate::feedback::Message;
use leptos::{html, prelude::*};

/// Native modal focus containment and inert background. The persistent navigation
/// guard opts out of automatic opening and owns its promise-based cancellation.
#[component]
pub(crate) fn Dialog(
    #[prop(default = NodeRef::new())] node: NodeRef<html::Dialog>,
    #[prop(into)] title: String,
    #[prop(default = false)] compact: bool,
    #[prop(default = true)] open_on_mount: bool,
    #[prop(into, optional)] busy: Signal<bool>,
    #[prop(optional)] on_cancel: Option<Callback<()>>,
    children: Children,
) -> impl IntoView {
    Effect::new(move |_| {
        if open_on_mount && let Some(dialog) = node.get() {
            let _ = dialog.show_modal();
        }
    });
    view! {
        <dialog node_ref=node class="management-dialog" class:compact=compact aria-label=title
            on:cancel=move |event: leptos::ev::Event| {
                if let Some(cancel) = on_cancel {
                    event.prevent_default();
                    if !busy.get_untracked() {
                        if let Some(dialog) = node.get_untracked() { dialog.close(); }
                        cancel.run(());
                    }
                }
            }>{children()}</dialog>
    }
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
    let dialog = NodeRef::<html::Dialog>::new();
    let cancel = move || {
        if !busy.get_untracked() {
            if let Some(element) = dialog.get_untracked() {
                element.close();
            }
            on_cancel.run(());
        }
    };
    view! { <Dialog node=dialog title=accessible_title compact=true busy=busy on_cancel=Callback::new(move |_|cancel())>
        <h2>{title}</h2><p>{description}</p>{message.view()}
        <div class="confirmation-actions">
            <ActionButton style=ButtonStyle::Ghost autofocus=true disabled=busy on_click=move |_|cancel()>"Cancel"</ActionButton>
            <ActionButton style=ButtonStyle::Danger disabled=busy on_click=move |_|on_confirm.run(())>{confirm_label}</ActionButton>
        </div>
    </Dialog> }
}

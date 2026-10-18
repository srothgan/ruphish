//! Shared presentation for management pages. Feature modules own data and mutations.
use crate::feedback::Message;
use leptos::{html, prelude::*};

#[component]
pub(crate) fn ManagementPage(children: Children) -> impl IntoView {
    view! { <section class="management-page">{children()}</section> }
}

#[component]
pub(crate) fn PageHeader(
    #[prop(into)] title: Signal<String>,
    #[prop(into, optional)] subtitle: Signal<String>,
    #[prop(default = false)] status: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="management-heading">
            <div><h1>{move || title.get()}</h1>
                <Show when=move || !subtitle.get().is_empty()>
                    <p class:save-state=status role=if status {Some("status")} else {None}>{move || subtitle.get()}</p>
                </Show>
            </div>
            <div class="management-actions">{children()}</div>
        </div>
    }
}

#[component]
pub(crate) fn DataTable(
    label: &'static str,
    headers: Vec<&'static str>,
    children: Children,
    #[prop(default = "")] class: &'static str,
) -> impl IntoView {
    view! { <div class="management-table-wrap"><table class=format!("management-table {class}") aria-label=label>
        <thead><tr>{headers.into_iter().map(|name| view!{<th scope="col">{name}</th>}).collect_view()}</tr></thead>
        <tbody>{children()}</tbody>
    </table></div> }
}

#[component]
pub(crate) fn FormFields(
    #[prop(into)] disabled: Signal<bool>,
    children: Children,
) -> impl IntoView {
    view! { <fieldset class="management-form" disabled=move || disabled.get()>{children()}</fieldset> }
}

#[component]
pub(crate) fn Field(
    #[prop(into)] label: String,
    #[prop(default = false)] checkbox: bool,
    children: Children,
) -> impl IntoView {
    view! { <label class="form-field" class:checkbox-label=checkbox>{label}{children()}</label> }
}

#[component]
pub(crate) fn SaveButton(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    #[prop(default = "button")] kind: &'static str,
    #[prop(optional)] form: Option<&'static str>,
    #[prop(optional)] on_click: Option<Callback<leptos::ev::MouseEvent>>,
) -> impl IntoView {
    view! { <button class="primary" type=kind form=form disabled=move || disabled.get()
    on:click=move |event| {if let Some(action) = on_click {action.run(event)}}>{move || label.get()}</button> }
}

#[component]
pub(crate) fn FormFooter(children: Children) -> impl IntoView {
    view! { <div class="management-footer">{children()}</div> }
}

#[component]
pub(crate) fn EmptyState(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! { <div class="management-empty"><h2>{title}</h2>{children()}</div> }
}

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
    description: &'static str,
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
            <button type="button" class="text-danger" autofocus disabled=move || busy.get() on:click=move |_|cancel()>"Cancel"</button>
            <button type="button" class="danger" disabled=move || busy.get() on:click=move |_|on_confirm.run(())>{confirm_label}</button>
        </div>
    </dialog> }
}

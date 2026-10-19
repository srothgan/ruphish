//! Shared presentation for management pages. Feature modules own data and mutations.
use crate::feedback::Message;
use leptos::{html, prelude::*};
use wasm_bindgen::{JsCast, prelude::*};

#[wasm_bindgen(module = "/web/anchored-panel.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = bindAnchoredPanel)]
    fn bind_panel(trigger: &web_sys::HtmlElement, panel: &web_sys::HtmlElement)
    -> js_sys::Function;
}

#[component]
pub(crate) fn InfoPopover(
    id: String,
    label: &'static str,
    explanation: &'static str,
) -> impl IntoView {
    let trigger = NodeRef::<html::Button>::new();
    let panel = NodeRef::<html::Div>::new();
    let remove = StoredValue::new_local(None::<js_sys::Function>);
    Effect::new(move |_| {
        if let (Some(trigger), Some(panel)) = (trigger.get(), panel.get()) {
            remove.set_value(Some(bind_panel(
                trigger.unchecked_ref(),
                panel.unchecked_ref(),
            )));
        }
    });
    on_cleanup(move || {
        remove.with_value(|remove| {
            if let Some(remove) = remove {
                let _ = remove.call0(&JsValue::NULL);
            }
        });
    });
    let heading_id = format!("{id}-heading");
    view! {
        <button node_ref=trigger type="button" class="info-trigger" aria-label=format!("About {label}") aria-haspopup="dialog" aria-expanded="false" aria-controls=id.clone()>
            <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><circle cx="10" cy="10" r="7.5"/><path d="M10 9v5"/><circle cx="10" cy="6" r=".7" fill="currentColor" stroke="none"/></svg>
        </button>
        <div node_ref=panel id=id class="info-popover" role="dialog" aria-labelledby=heading_id.clone() hidden>
            <div class="info-popover-heading"><strong id=heading_id.clone()>{label}</strong><button type="button" data-panel-close aria-label="Close explanation"><svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="m5 5 10 10M15 5 5 15"/></svg></button></div>
            <p>{explanation}</p>
        </div>
    }
}

#[component]
pub(crate) fn ManagementPage(children: Children) -> impl IntoView {
    view! { <section class="management-page">{children()}</section> }
}

#[component]
pub(crate) fn PageHeader(
    #[prop(into)] title: Signal<String>,
    #[prop(into, optional)] subtitle: Signal<String>,
    #[prop(default = false)] status: bool,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="management-heading">
            <div><h1>{move || title.get()}</h1>
                <Show when=move || !subtitle.get().is_empty()>
                    <p class:save-state=status role=if status {Some("status")} else {None}>{move || subtitle.get()}</p>
                </Show>
            </div>
            {children.map(|children|view!{<div class="management-actions">{children()}</div>})}
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
pub(crate) fn PageControls(
    pages: RwSignal<ruphish_ui::pagination::CursorPages>,
    #[prop(into)] disabled: Signal<bool>,
) -> impl IntoView {
    use ruphish_ui::pagination::CursorPages;
    view! { <FormFooter>
        <button type="button" disabled=move || disabled.get() || !pages.with(CursorPages::has_previous)
            on:click=move |_| {if !disabled.get_untracked() {pages.update(CursorPages::previous)}}>"Previous page"</button>
        <button type="button" disabled=move || disabled.get() || !pages.with(CursorPages::has_next)
            on:click=move |_| {if !disabled.get_untracked() {pages.update(CursorPages::advance)}}>"Next page"</button>
    </FormFooter> }
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

/// Copy a value without placing long verification tokens into the page layout.
#[component]
pub(crate) fn CopyButton(label: &'static str, value: String, message: Message) -> impl IntoView {
    use leptos::leptos_dom::helpers::{TimeoutHandle, set_timeout_with_handle};
    let copied = RwSignal::new(false);
    let copying = RwSignal::new(false);
    let reset = StoredValue::new(None::<TimeoutHandle>);
    on_cleanup(move || {
        reset.update_value(|timer| {
            if let Some(timer) = timer.take() {
                timer.clear();
            }
        });
    });
    view! {
        <button type="button" class="copy-button" title=format!("Copy {label}") disabled=move||copying.get() on:click=move |_|{
            if copying.get_untracked() { return; }
            reset.update_value(|timer| {
                if let Some(timer) = timer.take() { timer.clear(); }
            });
            copied.set(false);
            copying.set(true);
            let value=value.clone();
            leptos::task::spawn_local_scoped_with_cancellation(async move {
                if crate::account::copy_text(&value).await.is_ok() {
                    copied.set(true);
                    reset.set_value(set_timeout_with_handle(move||copied.set(false), std::time::Duration::from_secs(2)).ok());
                } else {message.error(format!("Could not copy {label}."))}
                copying.set(false);
            });
        }><svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true" focusable="false"><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M15 8V4H4v11h4"/></svg>
            <span class="copy-button-label"><span class="copy-button-size" aria-hidden="true">{format!("Copy {label}")}</span><span role="status">{move||if copying.get(){"Copying…".to_owned()}else if copied.get(){"Copied".to_owned()}else{format!("Copy {label}")}}</span></span>
        </button>
    }
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
pub(crate) fn FormFooter(
    children: Children,
    #[prop(default = false)] spread: bool,
) -> impl IntoView {
    view! { <div class="management-footer" class:management-footer-spread=spread>{children()}</div> }
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
            <button type="button" class="text-danger" autofocus disabled=move || busy.get() on:click=move |_|cancel()>"Cancel"</button>
            <button type="button" class="danger" disabled=move || busy.get() on:click=move |_|on_confirm.run(())>{confirm_label}</button>
        </div>
    </dialog> }
}

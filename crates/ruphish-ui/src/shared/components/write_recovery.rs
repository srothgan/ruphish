//! Recovery actions never overwrite an editable draft or advance its base revision implicitly.
use super::{ActionButton, ActionLink, Card, DeleteConfirmation};
use crate::api::error::ApiError;
use crate::tr;
use crate::trs;
use leptos::prelude::*;

#[component]
pub(crate) fn WriteRecovery(
    error: RwSignal<Option<ApiError>>,
    #[prop(default = false)] actions_only: bool,
    #[prop(into)] saved_href: Signal<String>,
    on_reload: Callback<()>,
    #[prop(into, optional)] busy: Signal<bool>,
) -> impl IntoView {
    let confirm = RwSignal::new(false);
    let missing =
        Signal::derive(move || error.with(|e| e.as_ref().is_some_and(ApiError::is_not_found)));
    let visible = move || error.with(|e| e.as_ref().is_some_and(ApiError::needs_write_recovery));
    let actions = move || {
        view! {
            <div class="row-actions">
                <Show when=move || {
                    !missing.get()
                }>
                    {move || {
                        view! {
                            <ActionLink href=saved_href.get() new_tab=true>
                                {tr!(ui.review_saved_version)}
                            </ActionLink>
                        }
                    }} <ActionButton disabled=busy on_click=move |_| confirm.set(true)>
                        {tr!(ui.discard_and_reload)}
                    </ActionButton>
                </Show>
            </div>
        }
    };
    view! {
        <Show when=visible>
            {move || {
                if actions_only {
                    actions().into_any()
                } else {
                    view! {
                        <Card>
                            <WriteRecoveryNotice error=error />
                            {actions()}
                        </Card>
                    }
                        .into_any()
                }
            }}
        </Show>
        <Show when=move || confirm.get()>
            <DeleteConfirmation
                title=trs!(ui.discard_and_reload)
                description=trs!(ui.discard_reload_confirmation)
                confirm_label=trs!(ui.discard_and_reload)
                busy=busy
                message=crate::shared::feedback::Message::new(String::new())
                on_cancel=Callback::new(move |_| confirm.set(false))
                on_confirm=Callback::new(move |_| {
                    confirm.set(false);
                    on_reload.run(());
                })
            />
        </Show>
    }
}

/// A save-conflict explanation that can share the form's existing white surface.
#[component]
pub(crate) fn WriteRecoveryNotice(error: RwSignal<Option<ApiError>>) -> impl IntoView {
    view! {
        <Show when=move || error.with(|e| e.as_ref().is_some_and(ApiError::needs_write_recovery))>
            <p role="alert">
                {move || {
                    if error.with(|e| e.as_ref().is_some_and(ApiError::is_not_found)) {
                        tr!(ui.write_deleted_help)()
                    } else {
                        tr!(ui.write_conflict_help)()
                    }
                }}
            </p>
            <p>
                <code>{move || error.get().map(|e| e.code).unwrap_or_default()}</code>
            </p>
        </Show>
    }
}

/// A rejected deletion requires reviewing the saved record before a fresh confirmation.
#[component]
pub(crate) fn DeleteRecovery(
    error: RwSignal<Option<ApiError>>,
    #[prop(into)] saved_href: Signal<String>,
) -> impl IntoView {
    view! {
        <Show when=move || error.with(|e| e.as_ref().is_some_and(ApiError::needs_write_recovery))>
            <p>{tr!(ui.delete_conflict_help)}</p>
            <code>{move || error.get().map(|e| e.code).unwrap_or_default()}</code>
            <Show when=move || {
                error.with(|e| e.as_ref().is_some_and(|e| !e.is_not_found()))
            }>
                {move || {
                    view! {
                        <ActionLink href=saved_href.get() new_tab=true>
                            {tr!(ui.review_saved_version)}
                        </ActionLink>
                    }
                }}
            </Show>
        </Show>
    }
}

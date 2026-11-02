//! Recovery actions never overwrite an editable draft or advance its base revision implicitly.
use super::{ActionButton, ActionLink, Card, DeleteConfirmation};
use leptos::prelude::*;
use ruphish_ui::{api_error::ApiError, tr, trs};

#[component]
pub(crate) fn WriteRecovery(
    error: RwSignal<Option<ApiError>>,
    #[prop(into)] saved_href: Signal<String>,
    on_reload: Callback<()>,
    #[prop(into, optional)] busy: Signal<bool>,
) -> impl IntoView {
    let confirm = RwSignal::new(false);
    let deleted_help = trs!(ui.write_deleted_help);
    let conflict_help = trs!(ui.write_conflict_help);
    let missing =
        Signal::derive(move || error.with(|e| e.as_ref().is_some_and(ApiError::is_not_found)));
    let visible = move || error.with(|e| e.as_ref().is_some_and(ApiError::needs_write_recovery));
    view! {
        <Show when=visible>
            <Card>
                <p role="alert">
                    {move || {
                        if missing.get() { deleted_help.get() } else { conflict_help.get() }
                    }}
                </p>
                <p>
                    <code>{move || error.get().map(|e| e.code).unwrap_or_default()}</code>
                </p>
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
            </Card>
        </Show>
        <Show when=move || confirm.get()>
            <DeleteConfirmation
                title=trs!(ui.discard_and_reload)
                description=trs!(ui.discard_reload_confirmation)
                confirm_label=trs!(ui.discard_and_reload)
                busy=busy
                message=crate::feedback::Message::new(String::new())
                on_cancel=Callback::new(move |_| confirm.set(false))
                on_confirm=Callback::new(move |_| {
                    confirm.set(false);
                    on_reload.run(());
                })
            />
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

use super::{ActionButton, ActionLink, ButtonStyle, Card};
use leptos::prelude::*;
use ruphish_ui::{api_error::ApiError, tr, trs};

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
            <div>
                <h1>{move || title.get()}</h1>
                <Show when=move || !subtitle.get().is_empty()>
                    <p class:save-state=status role=if status { Some("status") } else { None }>
                        {move || subtitle.get()}
                    </p>
                </Show>
            </div>
            {children.map(|children| view! { <div class="management-actions">{children()}</div> })}
        </div>
    }
}

#[component]
pub(crate) fn FormFooter(
    children: Children,
    #[prop(default = false)] spread: bool,
) -> impl IntoView {
    view! {
        <div class="management-footer" class:management-footer-spread=spread>
            {children()}
        </div>
    }
}

#[component]
pub(crate) fn EmptyState(#[prop(into)] title: Signal<String>, children: Children) -> impl IntoView {
    view! {
        <div class="management-empty">
            <h2>{move || title.get()}</h2>
            {children()}
        </div>
    }
}

/// A page-level state for an unknown route, missing record, or failed read.
/// Features supply the message and an optional action
/// (a navigation link or a retry button) instead of reinventing the layout.
#[component]
pub(crate) fn StatusPage(
    #[prop(into)] title: Signal<String>,
    #[prop(into)] message: Signal<String>,
    #[prop(into, optional)] status: Signal<Option<u16>>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <Card class="status-page">
            <div role="status">
                <h1>
                    {move || match status.get().filter(|status| (400..600).contains(status)) {
                        Some(status) => format!("{status} - {}", title.get()),
                        None => title.get(),
                    }}
                </h1>
                <p>{move || message.get()}</p>
            </div>
            {children.map(|children| view! { <div class="status-page-actions">{children()}</div> })}
        </Card>
    }
}

#[component]
pub(crate) fn NotFoundPage(
    #[prop(into)] back_href: String,
    #[prop(into)] back_label: Signal<String>,
) -> impl IntoView {
    view! {
        <StatusPage
            title=trs!(ui.page_not_found_title)
            message=trs!(ui.page_not_found_body)
            status=Some(404)
        >
            <ActionLink style=ButtonStyle::Primary href=back_href>
                {move || back_label.get()}
            </ActionLink>
        </StatusPage>
    }
}

/// Hide unusable content and its actions, preserving mounted editor drafts while
/// retrying. A failed background refresh with usable data keeps inline feedback.
#[component]
pub(crate) fn ReadBoundary(
    #[prop(into)] failure: Signal<Option<ApiError>>,
    #[prop(into)] has_data: Signal<bool>,
    #[prop(into)] fetching: Signal<bool>,
    retry: Callback<()>,
    #[prop(into)] back_href: String,
    #[prop(into)] back_label: Signal<String>,
    children: Children,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let back_href = StoredValue::new(back_href);
    let blocked = Signal::derive(move || {
        failure.with(|error| {
            error
                .as_ref()
                .is_some_and(|e| e.is_terminal() || !has_data.get())
        })
    });
    let title = Signal::derive(move || {
        failure
            .with(|error| match error.as_ref().map(|e| e.status) {
                Some(404) => leptos_i18n::t_string!(i18n, ui.record_not_found_title),
                Some(401 | 403) => leptos_i18n::t_string!(i18n, ui.access_denied),
                _ => leptos_i18n::t_string!(i18n, ui.page_load_failed),
            })
            .to_owned()
    });
    let message = Signal::derive(move || {
        failure.with(|error| match error {
            Some(error) if error.is_not_found() => {
                leptos_i18n::t_string!(i18n, ui.record_not_found_body).to_owned()
            }
            Some(error) => error.message.clone(),
            None => String::new(),
        })
    });
    let status = Signal::derive(move || failure.with(|error| error.as_ref().map(|e| e.status)));
    view! {
        <Show when=move || blocked.get()>
            <StatusPage title=title message=message status=status>
                <ActionLink href=back_href.get_value()>{move || back_label.get()}</ActionLink>
                <Show when=move || failure.with(|e| e.as_ref().is_some_and(|e| !e.is_terminal()))>
                    <ActionButton disabled=fetching on_click=move |_| retry.run(())>
                        {tr!(ui.retry_loading)}
                    </ActionButton>
                </Show>
            </StatusPage>
        </Show>
        <div class="read-boundary-content" hidden=move || blocked.get()>
            {children()}
        </div>
    }
}

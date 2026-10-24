use super::TextInput;
use super::{ActionButton, EmptyState, Field, FormFooter};
use leptos::prelude::*;
use ruphish_ui::{tr, trs};

/// One toolbar for resources, integrations and account management.
#[component]
pub(crate) fn ListToolbar(
    search: RwSignal<String>,
    #[prop(into)] count: Signal<usize>,
    #[prop(into, default = true.into())] count_known: Signal<bool>,
    #[prop(into, default = trs!(ui.search_by_name))] label: Signal<String>,
    #[prop(into, default = trs!(ui.resources_2))] noun: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="management-toolbar">
            <Field label=label>
                <TextInput
                    input_type="search"
                    value=Signal::derive(move || search.get())
                    on_input=move |event| search.set(event_target_value(&event))
                />
            </Field>
            <span>
                {move || {
                    format!(
                        "{} {}",
                        if count_known.get() { count.get().to_string() } else { "—".into() },
                        noun.get(),
                    )
                }}
            </span>
        </div>
    }
}

/// Common query feedback derives from the cache; it owns no competing error state.
#[component]
pub(crate) fn QueryStatus(
    fetching: Signal<bool>,
    has_data: Signal<bool>,
    error: Signal<Option<String>>,
    noun: Signal<String>,
    refresh: Callback<()>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    view! {
        <Show when=move || fetching.get()>
            <p role="status">
                {move || leptos_i18n::t_string!(i18n, messages.loading_arg0, arg0=noun.get())}
            </p>
        </Show>
        <Show when=move || error.get().is_some()>
            <p class="form-message error has-content" role="alert">
                {move || {
                    let error = error.get().unwrap_or_default();
                    if has_data.get() {
                        leptos_i18n::t_string!(
                            i18n, messages.refresh_failed_displayed_data_may_be_stale_arg0, arg0=error
                        )
                            .to_string()
                    } else {
                        error
                    }
                }}
            </p>
            <ActionButton disabled=fetching on_click=move |_| refresh.run(())>
                {move || leptos_i18n::t_string!(i18n, messages.retry_loading_arg0, arg0=noun.get())}
            </ActionButton>
        </Show>
    }
}

/// Table heading and compact trailing filter controls share one aligned row.
#[component]
pub(crate) fn TableToolbar(
    #[prop(into)] title: Signal<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="management-toolbar management-toolbar--filters">
            <h2>{move || title.get()}</h2>
            {children()}
        </div>
    }
}

/// Empty results are only shown after a successful completed read.
#[component]
pub(crate) fn ListEmptyState(
    #[prop(into)] ready: Signal<bool>,
    #[prop(into)] count: Signal<usize>,
    search: RwSignal<String>,
    #[prop(into, default = trs!(ui.no_resources_found))] title: Signal<String>,
    #[prop(into, default = trs!(ui.create_a_resource_to_get_started))] guidance: Signal<String>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    view! {
        <Show when=move || ready.get() && count.get() == 0>
            <EmptyState title=title>
                <p>
                    {move || {
                        if search.get().trim().is_empty() {
                            guidance.get()
                        } else {
                            leptos_i18n::t_string!(i18n, ui.try_another_search).to_owned()
                        }
                    }}
                </p>
            </EmptyState>
        </Show>
    }
}

#[component]
pub(crate) fn DataTable(
    #[prop(into)] label: Signal<String>,
    headers: Vec<Signal<String>>,
    children: Children,
    #[prop(default = "")] class: &'static str,
    #[prop(default = false)] compact: bool,
    /// Hide the column headers when there are no rows, so their full-width rule
    /// does not overhang a rounded empty-state card rendered below.
    #[prop(into, optional)]
    empty: Signal<bool>,
) -> impl IntoView {
    view! {
        <div class="management-table-wrap">
            <table
                class=format!("management-table {class}")
                class:compact=compact
                class:is-empty=move || empty.get()
                aria-label=move || label.get()
            >
                <thead>
                    <tr>
                        {headers
                            .into_iter()
                            .map(|name| view! { <th scope="col">{move || name.get()}</th> })
                            .collect_view()}
                    </tr>
                </thead>
                <tbody>{children()}</tbody>
            </table>
        </div>
    }
}

/// Shared pagination presentation for numeric and opaque cursor APIs.
#[component]
pub(crate) fn PaginationControls(
    #[prop(into)] disabled: Signal<bool>,
    #[prop(into)] has_previous: Signal<bool>,
    #[prop(into)] has_next: Signal<bool>,
    on_previous: Callback<()>,
    on_next: Callback<()>,
) -> impl IntoView {
    view! {
        <FormFooter>
            <ActionButton
                disabled=Signal::derive(move || disabled.get() || !has_previous.get())
                on_click=move |_| on_previous.run(())
            >
                {tr!(ui.previous_page)}
            </ActionButton>
            <ActionButton
                disabled=Signal::derive(move || disabled.get() || !has_next.get())
                on_click=move |_| on_next.run(())
            >
                {tr!(ui.next_page)}
            </ActionButton>
        </FormFooter>
    }
}

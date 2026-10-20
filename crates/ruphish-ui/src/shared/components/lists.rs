use super::TextInput;
use super::{ActionButton, EmptyState, Field, FormFooter};
use leptos::prelude::*;

/// One toolbar for resources, integrations and account management.
#[component]
pub(crate) fn ListToolbar(
    search: RwSignal<String>,
    #[prop(into)] count: Signal<usize>,
    #[prop(default = "Search by name")] label: &'static str,
    #[prop(default = "resources")] noun: &'static str,
) -> impl IntoView {
    view! {
        <div class="management-toolbar">
            <Field label=label><TextInput input_type="search" value=Signal::derive(move || search.get())
                on_input=move |event| search.set(event_target_value(&event))/></Field>
            <span>{move || format!("{} {noun}", count.get())}</span>
        </div>
    }
}

/// Empty results are only shown after a successful completed read.
#[component]
pub(crate) fn ListEmptyState(
    #[prop(into)] ready: Signal<bool>,
    #[prop(into)] count: Signal<usize>,
    search: RwSignal<String>,
    #[prop(default = "No resources found")] title: &'static str,
    #[prop(default = "Create a resource to get started.")] guidance: &'static str,
) -> impl IntoView {
    view! {
        <Show when=move || ready.get() && count.get() == 0>
            <EmptyState title=title><p>{move || if search.get().trim().is_empty() { guidance } else { "Try another search." }}</p></EmptyState>
        </Show>
    }
}

#[component]
pub(crate) fn DataTable(
    label: &'static str,
    headers: Vec<&'static str>,
    children: Children,
    #[prop(default = "")] class: &'static str,
    /// Hide the column headers when there are no rows, so their full-width rule
    /// does not overhang a rounded empty-state card rendered below.
    #[prop(into, optional)]
    empty: Signal<bool>,
) -> impl IntoView {
    view! { <div class="management-table-wrap"><table class=format!("management-table {class}") class:is-empty=move || empty.get() aria-label=label>
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
        <ActionButton disabled=Signal::derive(move || disabled.get() || !pages.with(CursorPages::has_previous))
            on_click=move |_| {pages.update(CursorPages::previous)}>"Previous page"</ActionButton>
        <ActionButton disabled=Signal::derive(move || disabled.get() || !pages.with(CursorPages::has_next))
            on_click=move |_| {pages.update(CursorPages::advance)}>"Next page"</ActionButton>
    </FormFooter> }
}

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

/// Session-local sort preferences survive route navigation without persisting resource data.
#[derive(Clone, Copy)]
pub(crate) struct TablePreferences(
    RwSignal<std::collections::HashMap<String, ruphish_ui::table_state::Sort>>,
);
impl TablePreferences {
    pub fn provide() {
        provide_context(Self(RwSignal::new(Default::default())));
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TableSorting {
    pub current: Signal<ruphish_ui::table_state::Sort>,
    pub change: Callback<String>,
}
impl TableSorting {
    pub fn new(
        name: impl Into<String>,
        column: &str,
        order: ruphish_domain::reporting::SortDirection,
    ) -> Self {
        let name = StoredValue::new(name.into());
        let initial = StoredValue::new(ruphish_ui::table_state::Sort {
            column: column.into(),
            order,
        });
        let preferences = expect_context::<TablePreferences>();
        let current = Signal::derive(move || {
            preferences
                .0
                .with(|v| v.get(&name.get_value()).cloned())
                .unwrap_or_else(|| initial.get_value())
        });
        Self {
            current,
            change: Callback::new(move |column: String| {
                let next = current.get_untracked().select(&column);
                preferences.0.update(|v| {
                    v.insert(name.get_value(), next);
                });
            }),
        }
    }
    pub fn order(&self) -> String {
        if self.current.get().order == ruphish_domain::reporting::SortDirection::Asc {
            "asc".into()
        } else {
            "desc".into()
        }
    }
    pub fn rows<T>(
        &self,
        rows: Vec<T>,
        value: impl Fn(&T, &str) -> ruphish_ui::table_state::Value,
        identity: impl Fn(&T) -> ruphish_ui::table_state::Value,
    ) -> Vec<T> {
        self.current.get().rows(rows, value, identity)
    }
    pub fn json(&self, rows: Vec<serde_json::Value>, dates: &[&str]) -> Vec<serde_json::Value> {
        self.rows(
            rows,
            |row, column| {
                if dates.contains(&column) {
                    ruphish_ui::table_state::timestamp(row[column].as_str().unwrap_or_default())
                } else {
                    ruphish_ui::table_state::scalar(&row[column])
                }
            },
            |row| ruphish_ui::table_state::scalar(&row["id"]),
        )
    }
}

#[component]
pub(crate) fn DataTable(
    #[prop(into)] label: Signal<String>,
    headers: Vec<Signal<String>>,
    /// Empty keys identify action-only columns.
    columns: Vec<&'static str>,
    sorting: TableSorting,
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
                            .zip(columns)
                            .map(|(name, key)| {
                                if key.is_empty() {
                                    view! { <th scope="col">{move || name.get()}</th> }.into_any()
                                } else {
                                    view! {
                                        <th
                                            scope="col"
                                            aria-sort=move || sorting.current.get().aria(key)
                                        >
                                            <button
                                                type="button"
                                                class="table-sort"
                                                on:click=move |_| sorting.change.run(key.to_owned())
                                            >
                                                <svg
                                                    class="table-sort-icon"
                                                    viewBox="0 0 12 16"
                                                    aria-hidden="true"
                                                    focusable="false"
                                                >
                                                    <path class="table-sort-up" d="M6 1 11 6H1Z" />
                                                    <path class="table-sort-down" d="M1 10H11L6 15Z" />
                                                </svg>
                                                <span>{move || name.get()}</span>
                                            </button>
                                        </th>
                                    }
                                        .into_any()
                                }
                            })
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

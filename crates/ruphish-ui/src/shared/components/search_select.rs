//! Searchable single selection using native radio keyboard behavior and shared panels.
use super::{ActionButton, EmptyState, Field, TextInput, ValidationBinding, info::anchored_nodes};
use leptos::prelude::*;
use ruphish_ui::{tr, trs};

#[component]
pub(crate) fn SearchSelect(
    #[prop(into, optional)] validation_field: String,
    id: &'static str,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] search_label: Signal<String>,
    value: RwSignal<String>,
    #[prop(into)] options: Signal<Vec<(String, String)>>,
    #[prop(into)] disabled: Signal<bool>,
) -> impl IntoView {
    let (trigger, panel) = anchored_nodes();
    let binding = ValidationBinding::new(validation_field);
    binding.register(Callback::new(move |()| {
        if let Some(node) = trigger.get() {
            let _ = node.focus();
        }
    }));
    let search = RwSignal::new(String::new());
    let filtered = Memo::new(move |_| {
        let query = search.get().trim().to_lowercase();
        options
            .get()
            .into_iter()
            .filter(|(value, label)| {
                value.is_empty()
                    || label.to_lowercase().contains(&query)
                    || value.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>()
    });
    let selected_label = move || {
        options.with(|items| {
            items
                .iter()
                .find(|(key, _)| *key == value.get())
                .map(|(_, label)| label.clone())
                .unwrap_or_else(|| value.get())
        })
    };
    view! {
        <div class="multi-select search-select">
            <span class="multi-select-label" id=format!("{id}-label")>
                {move || label.get()}
            </span>
            <button
                node_ref=trigger
                aria-invalid=move || binding.invalid().then_some("true")
                aria-describedby=move || binding.described_by()
                on:blur=move |_| binding.touched()
                type="button"
                class="multi-select-trigger"
                disabled=move || disabled.get()
                aria-haspopup="dialog"
                aria-expanded="false"
                aria-controls=format!("{id}-panel")
                aria-labelledby=format!("{id}-label {id}-value")
            >
                <span id=format!("{id}-value")>{selected_label}</span>
            </button>
            {binding.view()}
            <div
                node_ref=panel
                id=format!("{id}-panel")
                class="multi-select-panel search-select-panel"
                role="dialog"
                aria-label=move || label.get()
                data-panel-width="560"
                hidden
            >
                <Field label=search_label>
                    <TextInput
                        input_type="search"
                        value=search
                        on_input=move |event| search.set(event_target_value(&event))
                    />
                </Field>
                <div class="search-select-options" role="radiogroup" aria-label=move || label.get()>
                    <For
                        each=move || filtered.get()
                        key=|row| row.clone()
                        children=move |(key, text)| {
                            let key = StoredValue::new(key);
                            view! {
                                <label class="search-select-option">
                                    <input
                                        type="radio"
                                        name=id
                                        value=key.get_value()
                                        disabled=move || disabled.get()
                                        data-panel-close
                                        prop:checked=move || value.get() == key.get_value()
                                        on:change=move |_| {
                                            binding.edited();
                                            binding.touched();
                                            value.set(key.get_value());
                                        }
                                    />
                                    <span>{text}</span>
                                </label>
                            }
                        }
                    />
                </div>
                <Show when=move || {
                    filtered.with(|items| items.iter().all(|(value, _)| value.is_empty()))
                }>
                    <EmptyState title=trs!(preferences.no_matches)>
                        <p>{tr!(ui.try_another_search)}</p>
                    </EmptyState>
                </Show>
                <div class="multi-select-footer">
                    <ActionButton close_panel=true>{tr!(ui.done)}</ActionButton>
                </div>
            </div>
        </div>
    }
}

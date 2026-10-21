use super::TextInput;
use super::{ActionButton, Checkbox, Field};
use leptos::prelude::*;
use ruphish_ui::{tr, trs};

/// A scroll-loading, searchable checkbox dropdown. The feature owns option loading.
#[component]
pub(crate) fn MultiSelect(
    #[prop(into)] label: Signal<String>,
    selected: RwSignal<Vec<i64>>,
    search: RwSignal<String>,
    options: Signal<Vec<(i64, String)>>,
    loading: Signal<bool>,
    error: Signal<String>,
    has_more: Signal<bool>,
    on_more: Callback<()>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let (trigger, panel) = super::info::anchored_nodes();
    let list = NodeRef::<leptos::html::Div>::new();
    view! {
        <div class="multi-select">
            <span class="multi-select-label">{move || label.get()}</span>
            <button
                node_ref=trigger
                class="multi-select-trigger"
                type="button"
                aria-expanded="false"
                aria-label=move || label.get()
            >
                {move || {
                    if selected.with(Vec::is_empty) {
                        leptos_i18n::t_string!(i18n, ui.all_recipients).into()
                    } else {
                        leptos_i18n::t_string!(
                            i18n, messages.count_selected, count = selected.with(Vec::len)
                        )
                    }
                }}
            </button>
            <div
                node_ref=panel
                class="multi-select-panel"
                role="group"
                aria-label=move || label.get()
                data-panel-width="440"
                hidden
            >
                <Field label=trs!(ui.find_recipients)>
                    <TextInput
                        input_type="search"
                        value=Signal::derive(move || search.get())
                        on_input=move |event| {
                            search.set(event_target_value(&event));
                        }
                    />
                </Field>
                <div class="multi-select-toolbar">
                    <span>
                        {move || {
                            leptos_i18n::t_string!(
                                i18n, messages.count_selected, count = selected.with(Vec::len)
                            )
                        }}
                    </span>
                    <ActionButton on_click=move |_| {
                        selected.set(Vec::new())
                    }>{tr!(ui.clear_selection)}</ActionButton>
                </div>
                <Show when=move || loading.get()>
                    <p role="status">{tr!(ui.loading_recipients)}</p>
                </Show>
                <Show when=move || !error.get().is_empty()>
                    <p role="alert">{move || error.get()}</p>
                    <ActionButton on_click=move |_| on_retry.run(())>{tr!(ui.retry)}</ActionButton>
                </Show>
                <div
                    node_ref=list
                    on:scroll=move |_| {
                        if let Some(list) = list.get() && has_more.get_untracked()
                            && !loading.get_untracked() && error.get_untracked().is_empty()
                            && list.scroll_height() - list.scroll_top() - list.client_height() < 80
                        {
                            on_more.run(());
                        }
                    }
                    class="multi-select-options"
                    role="group"
                    aria-label=move || label.get()
                    aria-busy=move || loading.get().to_string()
                >
                    <For
                        each=move || options.get()
                        key=|row| row.0
                        children=move |(id, label)| {
                            view! {
                                <Checkbox
                                    label=label
                                    compact=true
                                    checked=Signal::derive(move || {
                                        selected.with(|ids| ids.contains(&id))
                                    })
                                    disabled=Signal::derive(move || {
                                        !selected.with(|ids| ids.contains(&id))
                                            && selected.with(Vec::len) >= 200
                                    })
                                    on_toggle=Callback::new(move |checked| {
                                        selected
                                            .update(|ids| {
                                                if checked {
                                                    if !ids.contains(&id) && ids.len() < 200 {
                                                        ids.push(id);
                                                        ids.sort_unstable();
                                                    }
                                                } else {
                                                    ids.retain(|value| *value != id);
                                                }
                                            });
                                    })
                                />
                            }
                        }
                    />
                </div>
                <Show when=move || {
                    !loading.get() && error.get().is_empty() && options.with(Vec::is_empty)
                }>
                    <p>{tr!(ui.no_matching_recipients)}</p>
                </Show>
                <Show when=move || { selected.with(Vec::len) >= 200 }>
                    <p>
                        {tr!(
                            ui.up_to_200_recipients_can_be_selected_clear_the_selection_to_include_everyone
                        )}
                    </p>
                </Show>
                <div class="multi-select-footer">
                    <span>
                        {move || {
                            if has_more.get() {
                                leptos_i18n::t_string!(i18n, ui.scroll_for_more_matches)
                            } else {
                                ""
                            }
                        }}
                    </span>
                    <ActionButton close_panel=true>{tr!(ui.done)}</ActionButton>
                </div>
            </div>

        </div>
    }
}

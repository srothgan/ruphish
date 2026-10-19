use super::{ActionButton, Field};
use leptos::prelude::*;

/// A scroll-loading, searchable checkbox dropdown. The feature owns option loading.
#[component]
pub(crate) fn MultiSelect(
    label: &'static str,
    selected: RwSignal<Vec<i64>>,
    search: RwSignal<String>,
    options: Signal<Vec<(i64, String)>>,
    loading: Signal<bool>,
    error: Signal<String>,
    has_more: Signal<bool>,
    on_more: Callback<()>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let list = NodeRef::<leptos::html::Div>::new();
    view! {
        <div class="multi-select">
            <span class="multi-select-label">{label}</span>
            <details prop:open=move || open.get() on:keydown=move |event| {
                if event.key() == "Escape" { open.set(false); }
            }>
                <summary on:click=move |event| {event.prevent_default(); open.update(|value| *value = !*value);}>
                    {move || if selected.with(Vec::is_empty) {"All recipients".into()} else {format!("{} selected", selected.with(Vec::len))}}
                </summary>
                <div class="multi-select-panel">
                    <Field label="Find recipients"><input type="search" prop:value=move || search.get()
                        on:input=move |event| {search.set(event_target_value(&event));}/></Field>
                    <div class="multi-select-toolbar">
                        <span>{move || format!("{} selected", selected.with(Vec::len))}</span>
                        <ActionButton on_click=move |_| selected.set(Vec::new())>"Clear selection"</ActionButton>
                    </div>
                    <Show when=move || loading.get()><p role="status">"Loading recipients..."</p></Show>
                    <Show when=move || !error.get().is_empty()>
                        <p role="alert">{move || error.get()}</p><ActionButton on_click=move |_| on_retry.run(())>"Retry"</ActionButton>
                    </Show>
                    <div node_ref=list on:scroll=move |_| {
                        if let Some(list)=list.get() && has_more.get_untracked() && !loading.get_untracked() && error.get_untracked().is_empty()
                            && list.scroll_height()-list.scroll_top()-list.client_height()<80 {on_more.run(());}
                    } class="multi-select-options" role="group" aria-label=label aria-busy=move || loading.get().to_string()>
                        <For each=move || options.get() key=|row|row.0 children=move |(id,label)|view!{
                            <label class="multi-select-option"><input type="checkbox"
                                prop:checked=move || selected.with(|ids| ids.contains(&id))
                                disabled=move || {!selected.with(|ids| ids.contains(&id)) && selected.with(Vec::len) >= 200}
                                on:change=move |event| {let checked = event_target_checked(&event); selected.update(|ids| {
                                    if checked { if !ids.contains(&id) && ids.len() < 200 {ids.push(id); ids.sort_unstable();} }
                                    else {ids.retain(|value| *value != id);}
                                });}/><span>{label}</span></label>
                        }/>
                    </div>
                    <Show when=move || !loading.get() && error.get().is_empty() && options.with(Vec::is_empty)><p>"No matching recipients."</p></Show>
                    <Show when=move || {selected.with(Vec::len) >= 200}><p>"Up to 200 recipients can be selected. Clear the selection to include everyone."</p></Show>
                    <div class="multi-select-footer">
                        <span>{move ||if has_more.get(){"Scroll for more matches"}else{""}}</span>
                        <ActionButton on_click=move |_| open.set(false)>"Done"</ActionButton>
                    </div>
                </div>
            </details>
        </div>
    }
}

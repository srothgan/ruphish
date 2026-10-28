use super::{ActionButton, Card, EmptyState};
use leptos::prelude::*;
use ruphish_ui::{read_state::ReadState, tr, trs};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/query-layout.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = retainQueryHeight)]
    fn retain_height(
        region: &web_sys::HtmlElement,
        content: &web_sys::HtmlElement,
    ) -> js_sys::Function;
}

/// Controls stay available after the first response when a new selection loads.
#[slot]
pub(crate) struct QueryControls {
    children: Children,
}

/// Own loading, content visibility and read failures for one data region.
/// Children remain mounted so background refreshes cannot discard local edits.
#[component]
pub(crate) fn QueryBoundary(
    fetching: Signal<bool>,
    has_data: Signal<bool>,
    error: Signal<Option<String>>,
    #[prop(into, default = trs!(ui.results))] noun: Signal<String>,
    refresh: Callback<()>,
    #[prop(optional)] controls: Option<QueryControls>,
    #[prop(into, optional)] empty: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let state = Signal::derive(move || {
        ReadState::from_query(has_data.get(), fetching.get(), error.get().is_some())
    });
    let initialized = RwSignal::new(has_data.get_untracked());
    Effect::new(move |_| {
        if has_data.get() {
            initialized.set(true);
        }
    });
    let region = NodeRef::<leptos::html::Div>::new();
    let content = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        if let (Some(region), Some(content)) = (region.get(), content.get()) {
            let release = retain_height(&region, &content);
            on_cleanup(move || {
                let _ = release.call0(&JsValue::NULL);
            });
        }
    });
    view! {
        {controls
            .map(|controls| {
                view! {
                    <div class="query-controls" hidden=move || !initialized.get()>
                        {(controls.children)()}
                    </div>
                }
            })}
        <div
            node_ref=region
            class="query-region"
            data-ready=move || state.get().shows_content().to_string()
            aria-busy=move || fetching.get().to_string()
        >
            <Show when=move || state.get() == ReadState::Loading>
                <Card class="query-placeholder">
                    <span class="query-loading-mark" aria-hidden="true"></span>
                    <span role="status">
                        {move || {
                            leptos_i18n::t_string!(i18n, messages.loading_arg0, arg0=noun.get())
                        }}
                    </span>
                </Card>
            </Show>
            <Show when=move || state.get() == ReadState::Failed>
                <Card class="query-failure">
                    <h2>{tr!(ui.page_load_failed)}</h2>
                    <p role="alert">{move || error.get()}</p>
                    <ActionButton disabled=fetching on_click=move |_| refresh.run(())>
                        {tr!(ui.retry_loading)}
                    </ActionButton>
                </Card>
            </Show>
            <Show when=move || state.get() == ReadState::Stale>
                <details class="query-refresh-error">
                    <summary>
                        <span role="status">{tr!(ui.update_failed)}</span>
                    </summary>
                    <div>
                        <p>
                            {move || {
                                leptos_i18n::t_string!(
                                    i18n, messages.refresh_failed_displayed_data_may_be_stale_arg0, arg0=error.get().unwrap_or_default()
                                )
                            }}
                        </p>
                        <ActionButton disabled=fetching on_click=move |_| refresh.run(())>
                            {tr!(ui.retry_loading)}
                        </ActionButton>
                    </div>
                </details>
            </Show>
            <div node_ref=content class="query-content" hidden=move || !state.get().shows_content()>
                <Show when=move || empty.get()>
                    <EmptyState title=trs!(ui.no_matching_results)>
                        <p>{tr!(ui.change_or_clear_the_filters_to_see_more_activity)}</p>
                    </EmptyState>
                </Show>
                <div hidden=move || empty.get()>{children()}</div>
            </div>
        </div>
    }
}

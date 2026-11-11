//! Consistent read-only presentation; features retain their data and actions.
use super::Card;
use crate::trs;
use leptos::prelude::*;

#[component]
pub(crate) fn DetailCard(children: Children) -> impl IntoView {
    view! { <Card class="resource-read-panel detail-card">{children()}</Card> }
}

#[component]
pub(crate) fn DetailSection(
    #[prop(into)] title: Signal<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <section class="detail-section" aria-label=move || title.get()>
            <h2>{move || title.get()}</h2>
            {children()}
        </section>
    }
}

#[component]
pub(crate) fn DetailGrid(children: Children) -> impl IntoView {
    view! { <dl class="resource-facts">{children()}</dl> }
}

#[component]
pub(crate) fn DetailField(
    #[prop(into)] label: Signal<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div>
            <dt>{move || label.get()}</dt>
            <dd>{children()}</dd>
        </div>
    }
}

pub(crate) fn detail_text(label: Signal<String>, value: String) -> impl IntoView {
    view! { <DetailText label=label value=value /> }
}

#[component]
pub(crate) fn DetailText(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    #[prop(into, default = trs!(ui.not_set))] fallback: Signal<String>,
) -> impl IntoView {
    view! {
        <DetailField label=label>
            <ReadValue value=value fallback=fallback />
        </DetailField>
    }
}

/// Read-only text and table cells share absence handling; features choose its meaning.
#[component]
pub(crate) fn ReadValue(
    #[prop(into)] value: Signal<String>,
    #[prop(into, default = trs!(ui.not_set))] fallback: Signal<String>,
) -> impl IntoView {
    view! {
        {move || {
            let value = value.get();
            if value.trim().is_empty() { fallback.get() } else { value }
        }}
    }
}

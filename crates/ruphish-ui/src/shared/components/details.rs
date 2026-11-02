//! Consistent read-only presentation; features retain their data and actions.
use super::Card;
use leptos::prelude::*;
use ruphish_ui::tr;

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
) -> impl IntoView {
    view! {
        <DetailField label=label>
            {move || {
                let value = value.get();
                if value.is_empty() { tr!(ui.not_set)() } else { value }
            }}
        </DetailField>
    }
}

use leptos::prelude::*;

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
            <div><h1>{move || title.get()}</h1>
                <Show when=move || !subtitle.get().is_empty()>
                    <p class:save-state=status role=if status {Some("status")} else {None}>{move || subtitle.get()}</p>
                </Show>
            </div>
            {children.map(|children|view!{<div class="management-actions">{children()}</div>})}
        </div>
    }
}

#[component]
pub(crate) fn FormFooter(
    children: Children,
    #[prop(default = false)] spread: bool,
) -> impl IntoView {
    view! { <div class="management-footer" class:management-footer-spread=spread>{children()}</div> }
}

#[component]
pub(crate) fn EmptyState(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! { <div class="management-empty"><h2>{title}</h2>{children()}</div> }
}

/// Shared readable progress state with decorative, static skeleton lines.
#[component]
pub(crate) fn LoadingState(label: &'static str) -> impl IntoView {
    view! {<div class="loading-state" role="status"><span>{label}</span><div class="loading-skeleton" aria-hidden="true"><span></span><span></span><span></span></div></div>}
}

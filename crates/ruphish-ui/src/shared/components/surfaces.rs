use leptos::prelude::*;

/// A quiet, full-width surface. Feature classes may control layout, not recreate
/// its border, background, radius or padding.
#[component]
pub(crate) fn Card(
    children: Children,
    #[prop(default = "")] class: &'static str,
    #[prop(optional)] label: Option<&'static str>,
) -> impl IntoView {
    view! { <section class=format!("card {class}") aria-label=label>{children()}</section> }
}

/// A single parent link, rather than an invented multi-level hierarchy.
#[component]
pub(crate) fn BackLink(#[prop(into)] href: Signal<String>, children: Children) -> impl IntoView {
    view! { <a class="breadcrumb" href=move || href.get()>{children()}</a> }
}

#[derive(Clone, Copy, Default)]
pub(crate) enum StatusTone {
    #[default]
    Neutral,
    Positive,
    Warning,
    Negative,
    Active,
}

impl StatusTone {
    fn class(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Positive => "positive",
            Self::Warning => "warning",
            Self::Negative => "negative",
            Self::Active => "active",
        }
    }
}

#[component]
pub(crate) fn StatusBadge(
    #[prop(into)] tone: Signal<StatusTone>,
    children: Children,
) -> impl IntoView {
    view! { <span class=move || format!("status-badge {}", tone.get().class())>{children()}</span> }
}

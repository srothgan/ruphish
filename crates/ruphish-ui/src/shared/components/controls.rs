//! Shared form controls. One owner for selects and checkboxes so their markup,
//! labelling and styling cannot drift across features.
use super::{ButtonStyle, Field};
use leptos::prelude::*;

/// Labelled dropdown. Features supply the options and react to the chosen value;
/// appearance (chevron, height, focus) is owned centrally.
#[component]
pub(crate) fn Select(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    on_change: Callback<String>,
    children: Children,
    #[prop(into, optional)] disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <Field label=label>
            <span class="select-control">
            <select prop:value=move || value.get() disabled=move || disabled.get()
                on:change=move |event| on_change.run(event_target_value(&event))>
                {children()}
            </select>
            </span>
        </Field>
    }
}

/// A disclosure that groups related actions behind one trigger, so a row of
/// equal-weight buttons (e.g. several export formats) collapses to a single
/// menu. Children are the action buttons; the panel closes after a choice.
#[component]
pub(crate) fn ActionMenu(
    #[prop(into)] label: Signal<String>,
    children: Children,
    #[prop(default = ButtonStyle::Secondary)] style: ButtonStyle,
    #[prop(into, optional)] disabled: Signal<bool>,
) -> impl IntoView {
    let (trigger, panel) = super::info::anchored_nodes();
    view! {
        <div class="action-menu">
            <button node_ref=trigger type="button" class=style.class() disabled=move || disabled.get() aria-expanded="false">
                <span>{move || label.get()}</span>
                <svg class="action-menu-chevron" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>
            </button>
            <div node_ref=panel class="action-menu-panel" role="group" aria-label=move || label.get() data-close-after-action hidden>{children()}</div>
        </div>
    }
}

/// Labelled checkbox with the brand accent. The label text follows the control.
#[component]
pub(crate) fn Checkbox(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] checked: Signal<bool>,
    on_toggle: Callback<bool>,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(default = false)] compact: bool,
) -> impl IntoView {
    view! {
        <label class="checkbox-label" class:compact=compact>
            <input type="checkbox" prop:checked=move || checked.get() disabled=move || disabled.get()
                on:change=move |event| on_toggle.run(event_target_checked(&event))/>
            <span>{move || label.get()}</span>
        </label>
    }
}

use leptos::{html, prelude::*};

pub(crate) struct Tab {
    pub id: &'static str,
    pub label: &'static str,
    pub disabled: Signal<bool>,
}

/// Automatic activation with roving focus. Disabled tabs are skipped; panels
/// remain mounted so changing tabs never destroys the user's draft.
#[component]
pub(crate) fn Tabs(
    label: &'static str,
    tabs: Vec<Tab>,
    selected: RwSignal<usize>,
) -> impl IntoView {
    let nodes = StoredValue::new(
        tabs.iter()
            .map(|_| NodeRef::<html::Button>::new())
            .collect::<Vec<_>>(),
    );
    let disabled = StoredValue::new(tabs.iter().map(|tab| tab.disabled).collect::<Vec<_>>());
    let keyboard = move |event: leptos::ev::KeyboardEvent| {
        let enabled = disabled.with_value(|tabs| {
            tabs.iter()
                .enumerate()
                .filter_map(|(i, d)| (!d.get_untracked()).then_some(i))
                .collect::<Vec<_>>()
        });
        let Some(next) =
            ruphish_ui::tabs::next_tab(&enabled, selected.get_untracked(), &event.key())
        else {
            return;
        };
        event.prevent_default();
        selected.set(next);
        nodes.with_value(|nodes| {
            if let Some(node) = nodes.get(next).and_then(NodeRef::get) {
                let _ = node.focus();
            }
        });
    };
    view! {
        <div class="content-tabs" role="tablist" aria-label=label on:keydown=keyboard>
            {tabs.into_iter().enumerate().map(|(index, tab)| {
                let node = nodes.with_value(|nodes| nodes[index]);
                view! { <button node_ref=node id=format!("{}-tab", tab.id) type="button" role="tab"
                    aria-controls=format!("{}-panel", tab.id)
                    aria-selected=move || (selected.get() == index).to_string()
                    tabindex=move || if selected.get() == index { 0 } else { -1 }
                    disabled=move || tab.disabled.get()
                    on:click=move |_| selected.set(index)>{tab.label}</button> }
            }).collect_view()}
        </div>
    }
}

#[component]
pub(crate) fn TabPanel(
    id: &'static str,
    #[prop(into)] selected: Signal<bool>,
    #[prop(into, optional)] busy: Signal<bool>,
    #[prop(default = "")] class: &'static str,
    children: Children,
) -> impl IntoView {
    view! { <div id=format!("{id}-panel") class=class role="tabpanel" aria-labelledby=format!("{id}-tab")
    hidden=move || !selected.get() aria-busy=move || busy.get().to_string()>{children()}</div> }
}

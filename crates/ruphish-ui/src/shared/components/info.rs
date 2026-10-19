use leptos::{html, prelude::*};
use wasm_bindgen::{JsCast, prelude::*};

#[wasm_bindgen(module = "/web/anchored-panel.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = bindAnchoredPanel)]
    fn bind_panel(trigger: &web_sys::HtmlElement, panel: &web_sys::HtmlElement)
    -> js_sys::Function;
}

#[component]
pub(crate) fn InfoPopover(
    id: String,
    label: &'static str,
    explanation: &'static str,
) -> impl IntoView {
    let trigger = NodeRef::<html::Button>::new();
    let panel = NodeRef::<html::Div>::new();
    let remove = StoredValue::new_local(None::<js_sys::Function>);
    Effect::new(move |_| {
        if let (Some(trigger), Some(panel)) = (trigger.get(), panel.get()) {
            remove.set_value(Some(bind_panel(
                trigger.unchecked_ref(),
                panel.unchecked_ref(),
            )));
        }
    });
    on_cleanup(move || {
        remove.with_value(|remove| {
            if let Some(remove) = remove {
                let _ = remove.call0(&JsValue::NULL);
            }
        });
    });
    let heading_id = format!("{id}-heading");
    view! {
        <button node_ref=trigger type="button" class="info-trigger" aria-label=format!("About {label}") aria-haspopup="dialog" aria-expanded="false" aria-controls=id.clone()>
            <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><circle cx="10" cy="10" r="7.5"/><path d="M10 9v5"/><circle cx="10" cy="6" r=".7" fill="currentColor" stroke="none"/></svg>
        </button>
        <div node_ref=panel id=id class="info-popover" role="dialog" aria-labelledby=heading_id.clone() hidden>
            <div class="info-popover-heading"><strong id=heading_id.clone()>{label}</strong><button type="button" data-panel-close aria-label="Close explanation"><svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="m5 5 10 10M15 5 5 15"/></svg></button></div>
            <p>{explanation}</p>
        </div>
    }
}

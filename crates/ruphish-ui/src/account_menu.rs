//! Account entry point anchored to the sidebar footer.
use super::{Api, Message};
use leptos::{html, prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use wasm_bindgen::{JsCast, prelude::*};

#[wasm_bindgen(module = "/web/account-menu.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = bindAccountMenu)]
    fn bind_menu(trigger: &web_sys::HtmlElement, panel: &web_sys::HtmlElement) -> js_sys::Function;
    #[wasm_bindgen(catch, js_name = copyAccountText)]
    pub(super) async fn copy_text(text: &str) -> Result<(), JsValue>;
}

pub(super) fn menu(api: Api, token: String, username: String, role: String) -> impl IntoView {
    let trigger = NodeRef::<html::Button>::new();
    let panel = NodeRef::<html::Div>::new();
    let remove = StoredValue::new_local(None::<js_sys::Function>);
    Effect::new(move |_| {
        if let (Some(trigger), Some(panel)) = (trigger.get(), panel.get()) {
            let cleanup = bind_menu(trigger.unchecked_ref(), panel.unchecked_ref());
            remove.set_value(Some(cleanup));
        }
    });
    on_cleanup(move || {
        remove.with_value(|remove| {
            if let Some(remove) = remove {
                let _ = remove.call0(&JsValue::NULL);
            }
        });
    });
    let token = StoredValue::new(token);
    let message = Message::new(String::new());
    let busy = RwSignal::new(false);
    let can_leave = crate::navigation::confirm_leave_action();
    let logout = move |_| {
        if busy.get_untracked() || !can_leave() {
            return;
        }
        busy.set(true);
        message.clear();
        spawn_local(async move {
            match api.form("/logout", &token.get_value(), &[]).await {
                Ok(_) => api.monitor.update(|m| m.signed_out()),
                Err(error) => {
                    message.error(error);
                    busy.set(false);
                }
            }
        });
    };
    view! {<div class="account-footer">
        <button node_ref=trigger class="account-trigger" aria-haspopup="dialog" aria-expanded="false" aria-controls="account-popover">
            <svg class="account-person" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><circle cx="12" cy="8" r="4"/><path d="M4 22v-2a8 8 0 0 1 16 0v2"/></svg>
            <span>{username.clone()}</span><svg class="account-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><path d="m7 14 5-5 5 5"/></svg>
        </button>
        <div node_ref=panel id="account-popover" class="account-popover" role="dialog" aria-labelledby="account-popover-name" hidden>
            <div class="account-popover-identity"><strong id="account-popover-name">{username}</strong><span>{role}</span></div>
            <a href="/settings">"My account"</a>
            <button class="text-danger" disabled=move||busy.get() on:click=logout>{move||if busy.get(){"Signing out..."}else{"Sign out"}}</button>
            {message.view()}
        </div>
    </div>}
}

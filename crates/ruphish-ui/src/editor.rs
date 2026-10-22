//! Shared source and visual editor with explicit draft ownership.

use crate::feedback::Message;
use leptos::{html, prelude::*};
use ruphish_domain::template::Field;
use wasm_bindgen::{JsCast, prelude::*};

#[wasm_bindgen(module = "/web/generated/editor.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = mountEditor)]
    fn mount_editor(
        parent: &web_sys::HtmlElement,
        source: &str,
        on_change: &js_sys::Function,
        variables: &str,
        on_visual_state: &js_sys::Function,
        readonly: bool,
        locale: &str,
        allow_forms: bool,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = setEditorLocale)]
    fn set_editor_locale(editor: &JsValue, locale: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = replaceSource)]
    fn replace_source(editor: &JsValue, source: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = destroyEditor)]
    fn destroy_editor(editor: &JsValue) -> Result<(), JsValue>;
}

type EditorHandle = (
    JsValue,
    Closure<dyn FnMut(String)>,
    Closure<dyn FnMut(bool)>,
);

pub(super) fn download_template(bytes: &[u8], name: &str) -> Result<(), JsValue> {
    let i18n = ruphish_ui::localization::use_i18n();

    let window = web_sys::window().ok_or_else(|| {
        JsValue::from_str(leptos_i18n::t_string!(i18n, ui.browser_window_unavailable))
    })?;
    let document = window.document().ok_or_else(|| {
        JsValue::from_str(leptos_i18n::t_string!(
            i18n,
            ui.browser_document_unavailable
        ))
    })?;
    let body = document.body().ok_or_else(|| {
        JsValue::from_str(leptos_i18n::t_string!(
            i18n,
            ui.browser_document_has_no_body
        ))
    })?;
    let anchor = document
        .create_element("a")?
        .dyn_into::<web_sys::HtmlElement>()?;
    let filename = if name.trim().is_empty() {
        "template.json".to_owned()
    } else {
        format!("{}.json", name.trim())
    };
    anchor.set_attribute("download", &filename)?;
    anchor.set_attribute("hidden", "")?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json;charset=utf-8");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let started = (|| -> Result<(), JsValue> {
        anchor.set_attribute("href", &url)?;
        body.append_child(&anchor)?;
        anchor.click();
        anchor.remove();
        Ok(())
    })();
    if let Err(error) = started {
        web_sys::Url::revoke_object_url(&url)?;
        return Err(error);
    }
    // Keep the Blob available until the browser has processed the download click.
    let cleanup = Closure::once_into_js(move || {
        if let Err(error) = web_sys::Url::revoke_object_url(&url) {
            web_sys::console::error_1(&error);
        }
    });
    if let Err(error) =
        window.set_timeout_with_callback_and_timeout_and_arguments_0(cleanup.unchecked_ref(), 1_000)
    {
        // Calling the one-shot closure also releases its Rust captures on failure.
        cleanup
            .unchecked_ref::<js_sys::Function>()
            .call0(&JsValue::NULL)?;
        return Err(error);
    }
    Ok(())
}

/// Embed the established source/visual editor without giving it a competing draft.
pub(super) fn content_editor(
    source: Signal<String>,
    changed: Callback<String>,
    epoch: RwSignal<u64>,
    message: Message,
    visual_pending: RwSignal<bool>,
    readonly: bool,
    allow_forms: bool,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let node = NodeRef::<html::Div>::new();
    let handle = StoredValue::new_local(None::<EditorHandle>);
    Effect::new(move |_| {
        let Some(parent) = node.get() else {
            return;
        };
        if handle.with_value(Option::is_some) {
            return;
        }
        let callback = Closure::<dyn FnMut(String)>::new(move |value| changed.run(value));
        let visual_callback = Closure::<dyn FnMut(bool)>::new(move |value| {
            let _ = visual_pending.try_set(value);
        });
        let variables = match serde_json::to_string(
            &Field::ALL
                .iter()
                .map(|field| field.name())
                .collect::<Vec<_>>(),
        ) {
            Ok(v) => v,
            Err(e) => {
                message.error(e.to_string());
                return;
            }
        };
        match mount_editor(
            &parent,
            &source.get_untracked(),
            callback.as_ref().unchecked_ref(),
            &variables,
            visual_callback.as_ref().unchecked_ref(),
            readonly,
            leptos_i18n::Locale::as_str(i18n.get_locale_untracked()),
            allow_forms,
        ) {
            Ok(editor) => handle.set_value(Some((editor, callback, visual_callback))),
            Err(e) => message.error(leptos_i18n::t_string!(
                i18n,
                messages.could_not_open_the_content_editor_arg0,
                arg0 = format!("{:?}", e)
            )),
        }
    });
    Effect::new(move |_| {
        let locale = i18n.get_locale();
        handle.with_value(|value| {
            if let Some((editor, _, _)) = value {
                let _ = set_editor_locale(editor, leptos_i18n::Locale::as_str(locale));
            }
        });
    });
    Effect::new(move |_| {
        epoch.get();
        handle.with_value(|value| {
            if let Some((editor, _, _)) = value
                && let Err(e) = replace_source(editor, &source.get_untracked())
            {
                message.error(leptos_i18n::t_string!(
                    i18n,
                    messages.could_not_load_the_document_arg0,
                    arg0 = format!("{:?}", e)
                ));
            }
        });
    });
    on_cleanup(move || {
        handle.update_value(|value| {
            if let Some((editor, callback, visual_callback)) = value.take() {
                let _ = destroy_editor(&editor);
                drop(callback);
                drop(visual_callback);
            }
        })
    });
    view! { <div class="content-editor" node_ref=node></div> }
}

//! Local template workflow used to prove the browser editor boundary before API wiring.

use leptos::{html, prelude::*};
use ruphish_contracts::legacy::EmailTemplate;
use ruphish_ui::{Draft, VARIABLES, preview_document};
use wasm_bindgen::{JsCast, prelude::*};

#[wasm_bindgen(module = "/web/generated/editor.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = mountEditor)]
    fn mount_editor(
        parent: &web_sys::HtmlElement,
        source: &str,
        on_change: &js_sys::Function,
        variables: &str,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = replaceSource)]
    fn replace_source(editor: &JsValue, source: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = destroyEditor)]
    fn destroy_editor(editor: &JsValue) -> Result<(), JsValue>;
}

type EditorHandle = (JsValue, Closure<dyn FnMut(String)>);

fn download_template(bytes: &[u8], name: &str) -> Result<(), JsValue> {
    let window =
        web_sys::window().ok_or_else(|| JsValue::from_str("Browser window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("Browser document unavailable"))?;
    let body = document
        .body()
        .ok_or_else(|| JsValue::from_str("Browser document has no body"))?;
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

pub(super) fn editor_workbench() -> impl IntoView {
    let draft = RwSignal::new(Draft::new(EmailTemplate {
        name: "Welcome message".into(),
        subject: "Hello {{.FirstName}}".into(),
        html: "<!doctype html>\n<html><head><style>body { font-family: sans-serif; padding: 24px; }</style></head>\n<body><h1>Hello {{.FirstName}}</h1><p>Edit your message here.</p></body></html>".into(),
        ..EmailTemplate::default()
    }));
    let message = RwSignal::new(String::new());
    let importing = RwSignal::new(false);
    let epoch = RwSignal::new(0_u64);
    let editor_node = NodeRef::<html::Div>::new();
    let import_node = NodeRef::<html::Input>::new();
    let handle = StoredValue::new_local(None::<EditorHandle>);

    Effect::new(move |_| {
        let Some(parent) = editor_node.get() else {
            return;
        };
        if handle.with_value(Option::is_some) {
            return;
        }
        let callback = Closure::<dyn FnMut(String)>::new(move |source| {
            draft.update(|value| value.edit(|template| template.html = source));
        });
        let variables = match serde_json::to_string(VARIABLES) {
            Ok(value) => value,
            Err(error) => {
                message.set(error.to_string());
                return;
            }
        };
        match mount_editor(
            &parent,
            &draft.get_untracked().value().html,
            callback.as_ref().unchecked_ref(),
            &variables,
        ) {
            Ok(editor) => handle.set_value(Some((editor, callback))),
            Err(error) => message.set(format!("Could not open the source editor: {error:?}")),
        }
    });
    Effect::new(move |_| {
        epoch.get();
        handle.with_value(|value| {
            if let Some((editor, _)) = value
                && let Err(error) = replace_source(editor, &draft.get_untracked().value().html)
            {
                message.set(format!("Could not load the document: {error:?}"));
            }
        });
    });
    on_cleanup(move || {
        handle.update_value(|value| {
            if let Some((editor, callback)) = value.take() {
                // Destroy the view before releasing the Rust callback it owns.
                if let Err(error) = destroy_editor(&editor) {
                    web_sys::console::error_1(&error);
                }
                drop(callback);
            }
        })
    });

    let add_files = move |event| {
        let input = event_target::<web_sys::HtmlInputElement>(&event);
        let Some(files) = input.files() else {
            return;
        };
        let opened_epoch = epoch.get_untracked();
        wasm_bindgen_futures::spawn_local(async move {
            for index in 0..files.length() {
                let Some(file) = files.get(index) else {
                    continue;
                };
                let read = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                if epoch.try_get_untracked() != Some(opened_epoch) {
                    return;
                }
                match read {
                    Ok(buffer) => {
                        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                        draft.update(|value| value.attach(file.name(), file.type_(), &bytes));
                    }
                    Err(error) => message.set(format!("Could not read {}: {error:?}", file.name())),
                }
            }
            input.set_value("");
        });
    };

    let import_file = move |event| {
        let input = event_target::<web_sys::HtmlInputElement>(&event);
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            return;
        };
        input.set_value("");
        let opened_epoch = epoch.get_untracked();
        let opened_value = draft.get_untracked().value().clone();
        importing.set(true);
        message.set(String::new());
        wasm_bindgen_futures::spawn_local(async move {
            let read = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
            let Some(current_epoch) = epoch.try_get_untracked() else {
                return;
            };
            importing.set(false);
            if current_epoch != opened_epoch || draft.get_untracked().value() != &opened_value {
                message.set("The draft changed while the file was opening. Import again to replace it; current edits were kept.".into());
                return;
            }
            match read {
                Ok(buffer) => {
                    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                    draft.update(|value| match value.open(&bytes) {
                        Ok(()) => {
                            epoch.update(|value| *value += 1);
                            message.set(format!("Imported {}.", file.name()));
                        }
                        Err(error) => message.set(format!(
                            "Could not import {}; current edits were kept. {error}",
                            file.name()
                        )),
                    });
                }
                Err(error) => message.set(format!(
                    "Could not read {}; current edits were kept. {error:?}",
                    file.name()
                )),
            }
        });
    };

    view! {
        <main>
            <header><span class="brand">"Ruphish"</span><span>"Editor workbench"</span></header>
            <div class="page-heading"><div><h1>"Email template"</h1><p>"Write your message, format its content and check the result."</p></div><div class="page-actions">
                <span class="draft-status" role="status">{move || if draft.get().is_dirty() { "Draft changed" } else { "Unchanged" }}</span>
                <input node_ref=import_node type="file" accept=".json,application/json" hidden on:change=import_file/>
                <button disabled=move || importing.get() on:click=move |_| { if let Some(input) = import_node.get() { input.click(); } }>{move || if importing.get() { "Opening…" } else { "Import JSON" }}</button>
                <button class="primary" on:click=move |_| {
                    let current = draft.get_untracked();
                    match current.export() {
                        Ok(bytes) => match download_template(&bytes, &current.value().name) {
                            Ok(()) => message.set("JSON download started.".into()),
                            Err(error) => message.set(format!("Could not download the template: {error:?}")),
                        },
                        Err(error) => message.set(format!("Could not export the template: {error}")),
                    }
                }>"Export JSON"</button>
            </div></div>
            <p class="scope">"Functional test workspace. Import opens a JSON file; Export downloads the current draft. Apply visual edits before exporting. Server saving and sending are not connected."</p>
            <p role="alert">{move || message.get()}</p>
            <div class="fields">
                <label>"Template name"<input prop:value=move || draft.get().value().name.clone() on:input=move |event| draft.update(|value| value.edit(|template| template.name = event_target_value(&event)))/></label>
                <label>"Subject"<input prop:value=move || draft.get().value().subject.clone() on:input=move |event| draft.update(|value| value.edit(|template| template.subject = event_target_value(&event)))/></label>
            </div>
            <section class="content-panel">
                <h2>"Message content"</h2>
                <div node_ref=editor_node></div>
                <details><summary>"Preview current HTML (read-only)"</summary><p>"This view cannot be edited. Visual edits appear here after Apply. External resources and interaction are disabled; recipient fields are shown as written."</p>
                    <iframe class="preview" title="Read-only template preview" sandbox="" referrerpolicy="no-referrer" tabindex="-1" inert="" srcdoc=move || preview_document(&draft.get().value().html)></iframe>
                </details>
            </section>
            <details><summary>"Plain-text alternative"</summary><p>"Text for mail clients that do not display HTML. Edit it independently from the formatted message."</p><label>"Plain-text message"<textarea rows="4" prop:value=move || draft.get().value().text.clone() on:input=move |event| draft.update(|value| value.edit(|template| template.text = event_target_value(&event)))></textarea></label></details>
            <details><summary>"Envelope sender"</summary><label>"SMTP envelope sender"<input prop:value=move || draft.get().value().envelope_sender.clone() on:input=move |event| draft.update(|value| value.edit(|template| template.envelope_sender = event_target_value(&event)))/></label></details>
            <section class="attachments"><h2>"Attachments"</h2><label class="file-label">"Add files"<input type="file" multiple on:change=add_files/></label>
                <ul>{move || draft.get().value().attachments.clone().unwrap_or_default().into_iter().enumerate().map(|(index, file)| view! {
                    <li><span>{file.name}</span><button on:click=move |_| draft.update(|value| { value.remove_attachment(index); })>"Remove"</button></li>
                }).collect_view()}</ul>
            </section>
            <footer><button on:click=move |_| { draft.update(Draft::reset); epoch.update(|value| *value += 1); message.set("Changes discarded.".into()); }>"Discard all changes"</button><span>"Restores the last imported template, including its attachments."</span></footer>
        </main>
    }
}

//! Request construction shared by the browser UI and executable WASM tests.
use gloo_net::http::Request;
use serde_json::Value;
use wasm_bindgen::prelude::*;

#[cfg_attr(not(feature = "demo"), wasm_bindgen(module = "/web/http.mjs"))]
#[cfg_attr(feature = "demo", wasm_bindgen(module = "/web/generated/demo.js"))]
extern "C" {
    #[wasm_bindgen(js_name = startRequest)]
    fn start_request(
        request: web_sys::Request,
        milliseconds: u32,
        on_headers: &js_sys::Function,
        download: bool,
    ) -> JsValue;
    #[wasm_bindgen(js_name = saveDownload, catch)]
    fn save_download(result: &JsValue, filename: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(js_name = requestPromise)]
    fn request_promise(request: &JsValue) -> js_sys::Promise;
    #[wasm_bindgen(js_name = cancelRequest)]
    fn cancel_request(request: &JsValue);
}

struct Pending(JsValue);
impl Drop for Pending {
    fn drop(&mut self) {
        cancel_request(&self.0);
    }
}

/// A completely received HTTP response; no unbounded body read remains.
#[derive(serde::Deserialize)]
pub struct Received {
    /// HTTP status, available even when application JSON is invalid.
    pub status: u16,
    /// Optional optimistic concurrency revision.
    pub etag: Option<String>,
    /// Response body, decoded only after session status is processed.
    pub body: String,
}

/// Bound reads to 15 seconds and operations to 60; never automatically retry writes.
pub async fn send(
    request: Request,
    on_headers: impl FnMut(u16) + 'static,
) -> Result<Received, String> {
    let read = matches!(request.method().as_str(), "GET" | "HEAD");
    let on_headers = Closure::wrap(Box::new(on_headers) as Box<dyn FnMut(u16)>);
    let pending = Pending(start_request(
        request.into(),
        if read { 15_000 } else { 60_000 },
        on_headers.as_ref().unchecked_ref(),
        false,
    ));
    let value = wasm_bindgen_futures::JsFuture::from(request_promise(&pending.0)).await.map_err(|_| {
        if read {
            leptos_i18n::td_string!(crate::localization::current_locale(), ui.the_request_failed_or_timed_out_try_again).to_owned()
        } else {
            leptos_i18n::td_string!(crate::localization::current_locale(), ui.the_request_failed_or_timed_out_its_outcome_is_unconfirmed_refresh_the_saved_sta).to_owned()
        }
    })?;
    let json = js_sys::JSON::stringify(&value)
        .ok()
        .and_then(|v| v.as_string())
        .ok_or(leptos_i18n::td_string!(
            crate::localization::current_locale(),
            ui.invalid_transport_response
        ))?;
    serde_json::from_str(&json).map_err(|_| {
        leptos_i18n::td_string!(
            crate::localization::current_locale(),
            ui.invalid_transport_response
        )
        .into()
    })
}

/// Reject broken success payloads rather than interpreting them as empty saved data.
pub fn decode(status: u16, body: &str) -> Result<Value, String> {
    if status == 204 && body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(body).map_err(|_| {
        leptos_i18n::td_string!(
            crate::localization::current_locale(),
            ui.the_server_returned_an_invalid_json_response_refresh_before_retrying_an_operatio
        )
        .into()
    })
}

/// Launch retries retain the reviewed body and owner-scoped idempotency key.
pub fn launch_request(
    path: &str,
    token: &str,
    key: &str,
    value: &Value,
) -> Result<Request, String> {
    let request = resource_request("POST", path, token, Some(value), None)?;
    request.headers().set("idempotency-key", key);
    Ok(request)
}

/// A missing payload means no body, including for GET/HEAD where Fetch forbids one.
pub fn resource_request(
    method: &str,
    path: &str,
    token: &str,
    value: Option<&Value>,
    revision: Option<&str>,
) -> Result<Request, String> {
    let mut builder = match method {
        "GET" => Request::get(path),
        "HEAD" => Request::get(path).method("HEAD".parse().map_err(|_| {
            leptos_i18n::td_string!(
                crate::localization::current_locale(),
                ui.invalid_request_method
            )
        })?),
        "POST" => Request::post(path),
        "PUT" => Request::put(path),
        "DELETE" => Request::delete(path),
        _ => {
            return Err(leptos_i18n::td_string!(
                crate::localization::current_locale(),
                ui.unsupported_request_method
            )
            .into());
        }
    }
    .header("x-ruphish-client", "browser")
    .header("accept", "application/json")
    .header("x-csrf-token", token);
    if let Some(revision) = revision {
        builder = builder.header("if-match", revision);
    }
    let request = if let Some(value) = value {
        builder.json(value)
    } else {
        builder.build()
    };
    request.map_err(|_| {
        leptos_i18n::td_string!(
            crate::localization::current_locale(),
            messages.cannot_prepare_method_request,
            method = method.to_owned()
        )
    })
}

/// A bounded binary transfer; callers process session/error status before saving it.
pub async fn download(
    request: Request,
    on_headers: impl FnMut(u16) + 'static,
) -> Result<(Received, JsValue), String> {
    let on_headers = Closure::wrap(Box::new(on_headers) as Box<dyn FnMut(u16)>);
    let pending = Pending(start_request(
        request.into(),
        60_000,
        on_headers.as_ref().unchecked_ref(),
        true,
    ));
    let value = wasm_bindgen_futures::JsFuture::from(request_promise(&pending.0))
        .await
        .map_err(|_| {
            leptos_i18n::td_string!(
                crate::localization::current_locale(),
                ui.the_export_failed_or_timed_out_narrow_the_filters_and_try_again
            )
        })?;
    let serialized = js_sys::JSON::stringify(&value)
        .ok()
        .and_then(|v| v.as_string())
        .ok_or(leptos_i18n::td_string!(
            crate::localization::current_locale(),
            ui.invalid_export_response
        ))?;
    let received = serde_json::from_str(&serialized).map_err(|_| {
        leptos_i18n::td_string!(
            crate::localization::current_locale(),
            ui.invalid_export_response
        )
    })?;
    Ok((received, value))
}
/// Save a completed successful transfer through the native browser download workflow.
pub fn save_file(value: &JsValue, filename: &str) -> Result<(), String> {
    save_download(value, filename).map_err(|_| {
        leptos_i18n::td_string!(
            crate::localization::current_locale(),
            ui.the_browser_could_not_save_this_export
        )
        .into()
    })
}

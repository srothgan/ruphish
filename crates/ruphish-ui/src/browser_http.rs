//! Request construction shared by the browser UI and executable WASM tests.
use gloo_net::http::Request;
use serde_json::Value;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/http.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = startRequest)]
    fn start_request(
        request: web_sys::Request,
        milliseconds: u32,
        on_headers: &js_sys::Function,
    ) -> JsValue;
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
    ));
    let value = wasm_bindgen_futures::JsFuture::from(request_promise(&pending.0)).await.map_err(|_| {
        if read {
            "The request failed or timed out. Try again.".to_owned()
        } else {
            "The request failed or timed out; its outcome is unconfirmed. Refresh the saved state before retrying.".to_owned()
        }
    })?;
    let json = js_sys::JSON::stringify(&value)
        .ok()
        .and_then(|v| v.as_string())
        .ok_or("Invalid transport response")?;
    serde_json::from_str(&json).map_err(|_| "Invalid transport response".into())
}

/// Reject broken success payloads rather than interpreting them as empty saved data.
pub fn decode(status: u16, body: &str) -> Result<Value, String> {
    if status == 204 && body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(body).map_err(|_| {
        "The server returned an invalid JSON response. Refresh before retrying an operation.".into()
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
        "HEAD" => Request::get(path).method("HEAD".parse().map_err(|_| "Invalid request method")?),
        "POST" => Request::post(path),
        "PUT" => Request::put(path),
        "DELETE" => Request::delete(path),
        _ => return Err("Unsupported request method".into()),
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
    request.map_err(|_| format!("Cannot prepare {method} request"))
}

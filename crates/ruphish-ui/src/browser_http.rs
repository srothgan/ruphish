//! Request construction shared by the browser UI and executable WASM tests.
use gloo_net::http::Request;
use serde_json::Value;

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

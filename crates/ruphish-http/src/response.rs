//! Transport behavior that must remain compatible when application routes are wired.

use axum::{
    body::Body,
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use ruphish_contracts::legacy::{self, Envelope};
use serde::Serialize;

/// Existing JSON content type, pretty formatting, escaping, and API CORS header.
pub fn json<T: Serialize>(status: StatusCode, value: &T) -> Response {
    match legacy::json(value) {
        Ok(bytes) => (
            status,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Error creating JSON response\n",
        )
            .into_response(),
    }
}

/// Existing structured error envelope with an explicit null data field.
pub fn error(status: StatusCode, message: &str) -> Response {
    json(
        status,
        &Envelope::<()> {
            success: false,
            message: message.into(),
            data: None,
        },
    )
}

/// Existing unknown-route/public-error response, including its final newline.
pub fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "404 page not found\n",
    )
        .into_response()
}

/// Gorilla's strict-slash redirect uses 301, including for writes.
///
/// The destination comes from registered route normalization; authentication redirects
/// require their own destination policy. Header validation happens before response creation.
pub fn slash_redirect(
    method: &Method,
    location: &str,
) -> Result<Response, axum::http::header::InvalidHeaderValue> {
    let destination = HeaderValue::from_str(location)?;
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::MOVED_PERMANENTLY;
    response.headers_mut().insert(header::LOCATION, destination);
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    if matches!(*method, Method::GET | Method::HEAD) {
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        if method == Method::GET {
            let escaped = location
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&#34;")
                .replace('\'', "&#39;");
            *response.body_mut() =
                Body::from(format!("<a href=\"{escaped}\">Moved Permanently</a>.\n\n"));
        }
    }
    Ok(response)
}

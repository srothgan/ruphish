//! Unauthenticated operational probes, outside session and campaign middleware.

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use ruphish_application::health::Service;

/// Mount on every enabled listener; share one service for the whole process.
pub fn router(service: Service) -> Router {
    Router::new()
        .route(
            "/health/live",
            get(|| async { status(StatusCode::OK, r#"{"status":"live"}"#) }),
        )
        .route("/health/ready", get(ready))
        .layer(middleware::map_response(|mut response: Response| async {
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-store"),
            );
            response
        }))
        .with_state(service)
}

async fn ready(State(service): State<Service>) -> Response {
    if service.ready().await {
        status(StatusCode::OK, r#"{"status":"ready"}"#)
    } else {
        status(StatusCode::SERVICE_UNAVAILABLE, r#"{"status":"not_ready"}"#)
    }
}

fn status(code: StatusCode, body: &'static str) -> Response {
    (code, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

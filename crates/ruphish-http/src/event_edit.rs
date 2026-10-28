//! Native event mutations use the existing authentication and CSRF boundary.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{error, response},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::StatusCode,
    response::Response,
    routing::post,
};
use ruphish_application::event_edit::Change;
use ruphish_contracts::errors::ErrorCode;

pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route(
            "/api/campaigns/{campaign}/recipients/{recipient}/events",
            post(change),
        )
        .layer(DefaultBodyLimit::max(4096))
}
async fn change(
    c: Context,
    Path((campaign, recipient)): Path<(i64, i64)>,
    body: Bytes,
) -> Result<Response, Rejection> {
    let user = c.api(None).await?;
    user.require("modify_objects")
        .map_err(|e| error(e.into()))?;
    let request: Change = serde_json::from_slice(&body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid event change structure",
        )
    })?;
    let result = c
        .campaigns()
        .change_event(
            user.identity,
            campaign,
            recipient,
            request,
            ruphish_application::datetime::now().as_second(),
        )
        .await
        .map_err(error)?;
    response(StatusCode::OK, &result, None)
}

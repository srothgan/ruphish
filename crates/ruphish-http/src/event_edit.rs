//! Native correction commands use the existing authentication and CSRF boundary.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{error, response},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query},
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use ruphish_application::event_edit::Change;
use ruphish_contracts::errors::ErrorCode;

pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route(
            "/api/campaigns/{campaign}/recipients/{recipient}/events",
            post(change),
        )
        .route(
            "/api/campaigns/{campaign}/recipients/{recipient}/events/{event}",
            get(inspect),
        )
        .layer(DefaultBodyLimit::max(4096))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    before: Option<i64>,
}
async fn inspect(
    c: Context,
    Path((campaign, recipient, event)): Path<(i64, i64, i64)>,
    Query(page): Query<Page>,
) -> Result<Response, Rejection> {
    let user = c.api(None).await?;
    let result = c
        .campaigns()
        .inspect_event(user.identity, campaign, recipient, event, page.before)
        .await
        .map_err(error)?;
    response(StatusCode::OK, &result, None)
}
async fn change(
    c: Context,
    Path((campaign, recipient)): Path<(i64, i64)>,
    body: Bytes,
) -> Result<Response, Rejection> {
    let user = c.api(None).await?;
    user.require("modify_objects")
        .map_err(|e| error(e.into()))?;
    let actor = c.event_actor(user.identity).await?;
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
            actor,
            campaign,
            recipient,
            request,
            ruphish_application::datetime::now().as_second(),
        )
        .await
        .map_err(error)?;
    response(StatusCode::OK, &result, None)
}

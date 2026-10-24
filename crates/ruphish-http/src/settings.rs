//! Administrator policy resource and explicitly confirmed installation-wide actions.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{error, response},
};
use axum::{
    Router,
    body::Bytes,
    extract::DefaultBodyLimit,
    http::{StatusCode, header},
    response::Response,
    routing::{get, post},
};
use ruphish_contracts::{errors::ErrorCode, settings as wire};

pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/settings", get(read).put(save))
        .route("/api/campaigns/complete-all/preview", post(preview))
        .route("/api/campaigns/complete-all", post(close))
        .layer(DefaultBodyLimit::max(8192))
}
async fn read(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let settings = context.settings().get(user.identity).await.map_err(error)?;
    response(
        StatusCode::OK,
        &serde_json::json!(settings),
        Some(&settings.revision.to_string()),
    )
}
async fn save(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    user.require("modify_system").map_err(|e| error(e.into()))?;
    let revision = context
        .headers
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim_matches('"').parse::<i64>().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            failure(
                StatusCode::PRECONDITION_REQUIRED,
                ErrorCode::RevisionRequired,
                "settings_revision_required",
            )
        })?;
    let input: wire::Edit = ruphish_contracts::domains::decode_request(&body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "settings_invalid_request",
        )
    })?;
    let settings = context
        .settings()
        .save(
            user.identity,
            revision,
            input.policy,
            ruphish_application::datetime::now().as_second(),
        )
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &serde_json::json!(settings),
        Some(&settings.revision.to_string()),
    )
}
async fn preview(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let preview = context
        .settings()
        .preview_closure(
            user.identity,
            ruphish_application::datetime::now().as_second(),
        )
        .await
        .map_err(error)?;
    response(StatusCode::OK, &serde_json::json!(preview), None)
}
async fn close(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    user.require("modify_system").map_err(|e| error(e.into()))?;
    let input: wire::CloseCampaigns =
        ruphish_contracts::domains::decode_request(&body).map_err(|_| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::RequestInvalid,
                "closure_invalid_request",
            )
        })?;
    if input.confirmation != "CLOSE ALL" {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "closure_confirmation_required",
        ));
    }
    let result = context
        .settings()
        .close_campaigns(
            user.identity,
            &input.token,
            ruphish_application::datetime::now().as_second(),
        )
        .await
        .map_err(error)?;
    response(StatusCode::OK, &serde_json::json!(result), None)
}

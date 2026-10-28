//! Offline, immutable starter content. Creation uses the normal resource APIs.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::response,
};
use axum::{Router, extract::Path, http::StatusCode, response::Response, routing::get};
use ruphish_contracts::{errors::ErrorCode, starter_templates::*};

mod source;
use source::Source;

// Generated from the shared manifest; files remain embedded in the executable.
include!(concat!(env!("OUT_DIR"), "/starter_catalog.rs"));

pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/starter-templates/", get(list))
        .route("/api/starter-templates/{id}", get(detail))
}

async fn authorize(context: &Context) -> Result<(), Rejection> {
    let user = context.api(None).await?;
    if !user.permits("modify_system") {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::AuthForbidden,
            "Forbidden",
        ));
    }
    Ok(())
}

async fn list(context: Context) -> Result<Response, Rejection> {
    authorize(&context).await?;
    response(
        StatusCode::OK,
        &CATALOG.iter().map(Source::summary).collect::<Vec<_>>(),
        None,
    )
}

async fn detail(context: Context, Path(id): Path<String>) -> Result<Response, Rejection> {
    authorize(&context).await?;
    let source = CATALOG.iter().find(|entry| entry.id == id).ok_or_else(|| {
        failure(
            StatusCode::NOT_FOUND,
            ErrorCode::ResourceNotFound,
            "Starter template not found",
        )
    })?;
    response(StatusCode::OK, &source.entry(), None)
}

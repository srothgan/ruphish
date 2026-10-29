//! Administrator webhooks and owner-scoped reporting mailbox resources.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{error, response},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use ruphish_application::integrations as app;
use ruphish_contracts::{errors::ErrorCode, integrations as wire};
use serde::Deserialize;
use serde_json::json;
fn runs(history: Vec<app::Run>) -> Vec<wire::Run> {
    history
        .into_iter()
        .map(|run| wire::Run {
            id: run.id,
            event_id: run.event_id,
            attempt: run.attempt,
            started: run.started,
            finished: run.finished,
            status: run.status,
            code: run.code,
            matched: run.matched,
            rejected: run.rejected,
        })
        .collect()
}
fn webhook(endpoint: app::Webhook) -> wire::Webhook {
    wire::Webhook {
        id: endpoint.id,
        name: endpoint.name,
        url: endpoint.url,
        is_active: endpoint.active,
        revision: endpoint.revision,
        has_secret: endpoint.has_secret,
        history: runs(endpoint.history),
    }
}
fn mailbox(mailbox: app::Mailbox) -> wire::Mailbox {
    wire::Mailbox {
        id: mailbox.id,
        name: mailbox.name,
        enabled: mailbox.enabled,
        host: mailbox.host,
        port: mailbox.port,
        username: mailbox.username,
        has_password: mailbox.has_password,
        tls: mailbox.tls,
        ignore_cert_errors: mailbox.ignore_cert_errors,
        folder: mailbox.folder,
        restrict_domain: mailbox.restrict_domain,
        delete_reported: mailbox.delete_reported,
        frequency: i64::from(mailbox.frequency),
        revision: mailbox.revision,
        last_check: mailbox.last_check,
        last_success: mailbox.last_success,
        next_check: mailbox.next_check,
        running: mailbox.running,
        code: mailbox.code,
        history: runs(mailbox.history),
    }
}
pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/webhooks/", get(hooks).post(create_hook))
        .route(
            "/api/webhooks/{id}",
            get(read_hook).put(update_hook).delete(delete_hook),
        )
        .route("/api/webhooks/{id}/validate", post(validate_hook))
        .route(
            "/api/reporting-mailboxes/",
            get(mailboxes).post(create_mailbox),
        )
        .route(
            "/api/reporting-mailboxes/{id}",
            get(read_mailbox).put(update_mailbox).delete(delete_mailbox),
        )
        .route(
            "/api/reporting-mailboxes/{id}/validate",
            post(validate_mailbox),
        )
        .route("/api/reporting-mailboxes/{id}/check", post(check_mailbox))
        .route("/api/reporting-mailboxes/{id}/resume", post(resume_mailbox))
        .layer(DefaultBodyLimit::max(16 * 1024))
}
async fn hooks(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    response(
        StatusCode::OK,
        &context
            .integrations()
            .webhooks(user.identity)
            .await
            .map_err(error)?
            .into_iter()
            .map(webhook)
            .collect::<Vec<_>>(),
        None,
    )
}
async fn mailboxes(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    response(
        StatusCode::OK,
        &context
            .integrations()
            .mailboxes(user.identity)
            .await
            .map_err(error)?
            .into_iter()
            .map(mailbox)
            .collect::<Vec<_>>(),
        None,
    )
}
async fn read_hook(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    response(
        StatusCode::OK,
        &webhook(
            context
                .integrations()
                .webhook(user.identity, id)
                .await
                .map_err(error)?,
        ),
        None,
    )
}
async fn read_mailbox(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    mailbox_response(&context, user.identity, id, StatusCode::OK).await
}
async fn mailbox_response(
    context: &Context,
    identity: ruphish_application::auth::Identity,
    id: i64,
    status: StatusCode,
) -> Result<Response, Rejection> {
    response(
        status,
        &mailbox(
            context
                .integrations()
                .mailbox(identity, id)
                .await
                .map_err(error)?,
        ),
        None,
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HookEdit {
    name: String,
    url: String,
    secret: Option<String>,
    #[serde(default)]
    is_active: bool,
    revision: Option<i64>,
    #[serde(rename = "id")]
    _id: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MailEdit {
    name: String,
    enabled: bool,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    tls: bool,
    #[serde(default)]
    ignore_cert_errors: bool,
    folder: String,
    #[serde(default)]
    restrict_domain: String,
    #[serde(default)]
    delete_reported: bool,
    frequency: u32,
    revision: Option<i64>,
}
fn decode<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Rejection> {
    serde_json::from_slice(body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "integration_invalid_request",
        )
    })
}
async fn save_hook(context: Context, id: Option<i64>, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let edit: HookEdit = decode(&body)?;
    let revision = if edit.revision.is_none() {
        if let Some(id) = id {
            Some(
                context
                    .integrations()
                    .webhook(user.identity, id)
                    .await
                    .map_err(error)?
                    .revision,
            )
        } else {
            None
        }
    } else {
        edit.revision
    };
    let endpoint = context
        .integrations()
        .save_webhook(
            user.identity,
            id,
            app::WebhookEdit {
                name: edit.name,
                url: edit.url,
                secret: edit.secret,
                active: edit.is_active,
                revision,
            },
        )
        .await
        .map_err(error)?;
    response(
        if id.is_some() {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        &webhook(endpoint),
        None,
    )
}
async fn save_mailbox(
    context: Context,
    id: Option<i64>,
    body: Bytes,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let edit: MailEdit = decode(&body)?;
    let mailbox_value = context
        .integrations()
        .save_mailbox(
            user.identity,
            id,
            app::MailboxEdit {
                name: edit.name,
                enabled: edit.enabled,
                host: edit.host,
                port: edit.port,
                username: edit.username,
                password: edit.password,
                tls: edit.tls,
                ignore_cert_errors: edit.ignore_cert_errors,
                folder: edit.folder,
                restrict_domain: edit.restrict_domain,
                delete_reported: edit.delete_reported,
                frequency: edit.frequency,
                revision: edit.revision,
            },
        )
        .await
        .map_err(error)?;
    response(
        if id.is_some() {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        &mailbox(mailbox_value),
        None,
    )
}
async fn create_hook(context: Context, body: Bytes) -> Result<Response, Rejection> {
    save_hook(context, None, body).await
}
async fn update_hook(
    context: Context,
    Path(id): Path<i64>,
    body: Bytes,
) -> Result<Response, Rejection> {
    save_hook(context, Some(id), body).await
}
async fn create_mailbox(context: Context, body: Bytes) -> Result<Response, Rejection> {
    save_mailbox(context, None, body).await
}
async fn update_mailbox(
    context: Context,
    Path(id): Path<i64>,
    body: Bytes,
) -> Result<Response, Rejection> {
    save_mailbox(context, Some(id), body).await
}
fn expected_revision(headers: &HeaderMap) -> Result<Option<i64>, Rejection> {
    headers
        .get("if-match")
        .map(|value| {
            value
                .to_str()
                .ok()
                .and_then(|value| value.trim_matches('"').parse().ok())
                .ok_or_else(|| {
                    failure(
                        StatusCode::BAD_REQUEST,
                        ErrorCode::RevisionRequired,
                        "integration_invalid_revision",
                    )
                })
        })
        .transpose()
}
fn success() -> Result<Response, Rejection> {
    response(StatusCode::OK, &json!({"success":true}), None)
}
async fn delete_hook(
    context: Context,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let revision = expected_revision(&headers)?;
    context
        .integrations()
        .delete_webhook(user.identity, id, revision)
        .await
        .map_err(error)?;
    success()
}
async fn delete_mailbox(
    context: Context,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let revision = expected_revision(&headers)?;
    context
        .integrations()
        .delete_mailbox(user.identity, id, revision)
        .await
        .map_err(error)?;
    success()
}
async fn validate_hook(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    context
        .integrations()
        .validate_webhook(user.identity, id)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &webhook(
            context
                .integrations()
                .webhook(user.identity, id)
                .await
                .map_err(error)?,
        ),
        None,
    )
}
async fn validate_mailbox(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    context
        .integrations()
        .validate_mailbox(user.identity, id)
        .await
        .map_err(error)?;
    mailbox_response(&context, user.identity, id, StatusCode::OK).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    revision: i64,
}
async fn resume_mailbox(
    context: Context,
    Path(id): Path<i64>,
    body: Bytes,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let expected: Revision = decode(&body)?;
    context
        .integrations()
        .resume_mailbox(user.identity, id, expected.revision)
        .await
        .map_err(error)?;
    mailbox_response(&context, user.identity, id, StatusCode::OK).await
}
async fn check_mailbox(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    context
        .integrations()
        .check_mailbox(user.identity, id)
        .await
        .map_err(error)?;
    mailbox_response(&context, user.identity, id, StatusCode::ACCEPTED).await
}

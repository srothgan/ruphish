//! Additive draft API. The legacy campaign API continues to represent executions.
use super::*;
use app::drafts::{Configuration, Draft};
use serde::Deserialize;

pub(super) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/campaign-drafts", get(list))
        .route("/api/campaign-drafts/prepare", post(prepare_draft))
        .route(
            "/api/campaign-drafts/{id}",
            get(read).put(save).delete(remove),
        )
        .route("/api/campaign-drafts/{id}/launch", post(launch))
}
fn draft_error(value: Error) -> Rejection {
    match value {
        Error::Invalid(message) => {
            failure(StatusCode::BAD_REQUEST, ErrorCode::DraftInvalid, &message)
        }
        other => error(other),
    }
}
fn decode_body<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Rejection> {
    serde_json::from_slice(body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::DraftInvalid,
            "Invalid draft structure",
        )
    })
}
fn encode(d: &Draft) -> Value {
    json!(reads::Draft {
        id: d.id.clone(),
        revision: d.revision,
        configuration: d.configuration.clone(),
        step: d.step,
        created: datetime::utc_seconds(d.created).ok(),
        modified: datetime::utc_seconds(d.modified).ok(),
        campaign_id: d.campaign_id,
        status: if d.campaign_id.is_some() {
            "Launched".into()
        } else {
            "Draft".into()
        }
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    revision: Option<i64>,
    configuration: Configuration,
    step: i64,
}
async fn save(c: Context, Path(id): Path<String>, body: Bytes) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let edit: Edit = decode_body(&body)?;
    let saved = c
        .campaigns()
        .save_draft(
            caller.identity,
            &id,
            edit.revision,
            edit.configuration,
            edit.step,
            now(),
        )
        .await
        .map_err(draft_error)?;
    response(StatusCode::OK, &encode(&saved), None)
}
async fn read(c: Context, Path(id): Path<String>) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let saved = c
        .campaigns()
        .draft(caller.identity, &id)
        .await
        .map_err(draft_error)?;
    response(StatusCode::OK, &encode(&saved), None)
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Page {
    after: String,
}
async fn list(
    c: Context,
    page: Result<Query<Page>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let Query(page) =
        page.map_err(|_| draft_error(Error::Invalid("Invalid draft query".into())))?;
    let mut rows = c
        .campaigns()
        .drafts(caller.identity, &page.after, 51)
        .await
        .map_err(draft_error)?;
    let more = rows.len() > 50;
    rows.truncate(50);
    let next = more
        .then(|| rows.last().map(|row| row.id.clone()))
        .flatten();
    response(
        StatusCode::OK,
        &json!({"items":rows.iter().map(encode).collect::<Vec<_>>(),"next_after":next}),
        None,
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    revision: i64,
}
async fn remove(
    c: Context,
    Path(id): Path<String>,
    revision: Result<Query<Revision>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let Query(revision) =
        revision.map_err(|_| draft_error(Error::Invalid("Invalid draft revision".into())))?;
    c.campaigns()
        .delete_draft(caller.identity, &id, revision.revision)
        .await
        .map_err(draft_error)?;
    response(StatusCode::OK, &json!({"success":true}), None)
}
async fn launch(c: Context, Path(id): Path<String>, body: Bytes) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let request: wire::LaunchDraft = decode_body(&body)?;
    let mut input = super::command(&c, request.campaign)?;
    // Draft identity itself owns idempotency; no independent key can bypass it.
    input.idempotency = None;
    let campaign = c
        .campaigns()
        .launch_draft(caller.identity, &id, request.revision, input, now())
        .await
        .map_err(draft_error)?;
    response(StatusCode::OK, &super::encode_campaign(&campaign), None)
}

async fn prepare_draft(c: Context, body: Bytes) -> Result<Response, Rejection> {
    super::prepare_inner(c, body, true).await
}

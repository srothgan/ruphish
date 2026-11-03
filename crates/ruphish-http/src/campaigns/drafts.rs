//! Additive draft API. The legacy campaign API continues to represent executions.
use super::*;
use app::drafts::Draft;

pub(super) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/campaign-drafts", get(list).post(create))
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
fn encode(d: &Draft, lookup: &Lookup) -> reads::Draft {
    reads::Draft {
        attribution: lookup.resolve(&d.attribution),
        id: d.id,
        revision: d.revision,
        configuration: (&d.configuration).into(),
        step: d.step,
        created: datetime::utc_seconds(d.created).ok(),
        modified: datetime::utc_seconds(d.modified).ok(),
        campaign_id: d.campaign_id,
        status: app::drafts::Status::from_campaign_id(d.campaign_id),
    }
}
use ruphish_contracts::campaign::request::DraftWrite as Edit;
async fn create(c: Context, body: Bytes) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let input: ruphish_contracts::campaign::request::DraftCreate = decode_body(&body)?;
    let key = super::creation_key(&c, &input)?.ok_or_else(|| {
        draft_error(Error::Invalid(
            "Draft creation requires an Idempotency-Key".into(),
        ))
    })?;
    let saved = c
        .campaigns()
        .create_draft(caller.identity, key, input.configuration, input.step, now())
        .await
        .map_err(draft_error)?;
    let lookup = Lookup::from_identities(saved.identities);
    response(StatusCode::CREATED, &encode(&saved.record, &lookup), None)
}
async fn save(c: Context, Path(id): Path<i64>, body: Bytes) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let edit: Edit = decode_body(&body)?;
    let saved = c
        .campaigns()
        .save_draft(
            caller.identity,
            id,
            edit.revision,
            edit.configuration,
            edit.step,
            now(),
        )
        .await
        .map_err(draft_error)?;
    let lookup = Lookup::from_identities(saved.identities);
    let saved = saved.record;
    response(StatusCode::OK, &encode(&saved, &lookup), None)
}
async fn read(c: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let saved = c
        .campaigns()
        .draft(caller.identity, id)
        .await
        .map_err(draft_error)?;
    let lookup = Lookup::load(&c, [&saved.attribution]).await?;
    response(StatusCode::OK, &encode(&saved, &lookup), None)
}
use ruphish_contracts::campaign::request::DraftPage as Page;
async fn list(
    c: Context,
    page: Result<Query<Page>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let Query(page) =
        page.map_err(|_| draft_error(Error::Invalid("Invalid draft query".into())))?;
    if page.after < 0 {
        return Err(draft_error(Error::Invalid("Invalid draft cursor".into())));
    }
    let mut rows = c
        .campaigns()
        .drafts(caller.identity, page.after, 51)
        .await
        .map_err(draft_error)?;
    let more = rows.len() > 50;
    rows.truncate(50);
    let next = more.then(|| rows.last().map(|row| row.id)).flatten();
    let lookup = Lookup::load(&c, rows.iter().map(|v| &v.attribution)).await?;
    response(
        StatusCode::OK,
        &reads::Drafts {
            items: rows.iter().map(|v| encode(v, &lookup)).collect(),
            next_after: next,
        },
        None,
    )
}
use ruphish_contracts::common::Revision;
async fn remove(
    c: Context,
    Path(id): Path<i64>,
    revision: Result<Query<Revision>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let Query(revision) =
        revision.map_err(|_| draft_error(Error::Invalid("Invalid draft revision".into())))?;
    c.campaigns()
        .delete_draft(caller.identity, id, revision.revision)
        .await
        .map_err(draft_error)?;
    response(StatusCode::OK, &reads::DraftRemoval { success: true }, None)
}
async fn launch(c: Context, Path(id): Path<i64>, body: Bytes) -> Result<Response, Rejection> {
    let caller = c.api(None).await?;
    let request: wire::LaunchDraft = decode_body(&body)?;
    let mut input = super::command(&c, request.campaign)?;
    // Draft identity itself owns idempotency; no independent key can bypass it.
    input.idempotency = None;
    let campaign = c
        .campaigns()
        .launch_draft(caller.identity, id, request.revision, input, now())
        .await
        .map_err(draft_error)?;
    let lookup = Lookup::from_identities(campaign.identities);
    let campaign = campaign.record;
    response(
        StatusCode::OK,
        &super::encode_campaign(&campaign, &lookup)?,
        None,
    )
}

async fn prepare_draft(c: Context, body: Bytes) -> Result<Response, Rejection> {
    super::prepare_inner(c, body, true).await
}

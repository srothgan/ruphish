//! Authenticated domain resource API; campaign eligibility is server-derived.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{error, response},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::{StatusCode, header},
    response::Response,
    routing::{get, post},
};
use ruphish_application::domains as app;
use ruphish_contracts::domains as wire;
use ruphish_contracts::errors::ErrorCode;

fn now() -> i64 {
    ruphish_application::datetime::now().as_second()
}
fn encode(d: app::Domain) -> wire::Domain {
    let observed_at = now();
    let eligible = app::eligibility(d.enabled, d.eligible_until, observed_at).is_ok();
    wire::Domain {
        verification: wire::Verification {
            required: d.verification_required,
            status: if !d.verification_required {
                "not_required"
            } else {
                app::verification_status(d.eligible_until, d.checked_at, observed_at)
            }
            .into(),
            expires_at: d.expires_at,
            next_check_at: d.verification_required.then_some(d.next_check_at),
        },
        id: d.id,
        name: d.name,
        origin: d.origin,
        hostname: d.hostname.clone(),
        enabled: d.enabled,
        revision: d.revision,
        txt_name: format!("_ruphish-verify.{}", d.hostname),
        txt_value: format!("ruphish-verification={}", d.challenge),
        challenge_expires: d.challenge_expires,
        checked_at: d.checked_at,
        verified_until: d.verified_until,
        control: d.control,
        dns: d.dns,
        routing: d.routing,
        tls: d.tls,
        code: d.code,
        status: app::verification_status(d.verified_until, d.checked_at, observed_at).into(),
        eligible,
        active_check: d.active_check.map(run),
        history: d.history.into_iter().map(run).collect(),
        checking: d.checking,
        next_check_at: d.next_check_at,
        campaigns: d.campaigns,
    }
}
fn run(r: app::VerificationRun) -> wire::VerificationRun {
    wire::VerificationRun {
        id: r.id,
        state: r.state,
        scheduled_at: r.scheduled_at,
        started_at: r.started_at,
        finished_at: r.finished_at,
        code: r.code,
    }
}
pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/domains/", get(list).post(create))
        .route("/api/domains/validated/", get(validated))
        .route("/api/domains/{id}", get(read).put(update).delete(delete))
        .route("/api/domains/{id}/verify", post(verify))
        .layer(DefaultBodyLimit::max(8192))
}
async fn listing(context: Context, only_validated: bool) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let items: Vec<_> = context
        .domains()
        .list(user.identity)
        .await
        .map_err(error)?
        .into_iter()
        .map(encode)
        .filter(|d| !only_validated || d.eligible)
        .collect();
    response(StatusCode::OK, &items, None)
}
async fn list(context: Context) -> Result<Response, Rejection> {
    listing(context, false).await
}
async fn validated(context: Context) -> Result<Response, Rejection> {
    listing(context, true).await
}
async fn read(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let domain = encode(
        context
            .domains()
            .get(user.identity, id)
            .await
            .map_err(error)?,
    );
    response(StatusCode::OK, &domain, Some(&domain.revision.to_string()))
}
async fn save(context: Context, id: Option<i64>, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let input: wire::Edit = wire::decode_request(&body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "domain_invalid_request",
        )
    })?;
    let result = context
        .domains()
        .save(
            user.identity,
            id,
            app::Edit {
                name: input.name,
                origin: input.origin,
                enabled: input.enabled,
                revision: input.revision,
            },
            now(),
        )
        .await
        .map_err(error)?;
    let domain = encode(result);
    response(
        if id.is_some() {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        &domain,
        Some(&domain.revision.to_string()),
    )
}
async fn create(context: Context, body: Bytes) -> Result<Response, Rejection> {
    save(context, None, body).await
}
async fn update(context: Context, Path(id): Path<i64>, body: Bytes) -> Result<Response, Rejection> {
    save(context, Some(id), body).await
}
async fn verify(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let domain = encode(
        context
            .domains()
            .request_check(user.identity, id, now())
            .await
            .map_err(error)?,
    );
    response(StatusCode::ACCEPTED, &domain, None)
}
async fn delete(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let revision = context
        .headers
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim_matches('"').parse().ok())
        .ok_or_else(|| {
            failure(
                StatusCode::PRECONDITION_REQUIRED,
                ErrorCode::RevisionRequired,
                "domain_revision_required",
            )
        })?;
    context
        .domains()
        .delete(user.identity, id, revision)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &serde_json::json!({"success":true,"message":"Domain deleted","data":null}),
        None,
    )
}

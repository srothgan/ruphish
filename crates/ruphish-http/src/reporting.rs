//! Authorized reporting endpoints and generated download responses.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{error, response},
};
use axum::{
    Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use ruphish_application::reporting as app;
use ruphish_contracts::reporting::Filter;
use ruphish_contracts::{campaign_reads as reads, errors::ErrorCode};
use serde_json::json;
fn filter(f: &Filter) -> Result<app::Filter, Rejection> {
    let recipients = if f.recipients.is_empty() {
        Vec::new()
    } else {
        if f.recipients.len() > 4200 {
            return Err(failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::AnalysisFilterInvalid,
                "analysis_invalid_filter",
            ));
        }
        f.recipients
            .split(',')
            .map(str::parse::<i64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                failure(
                    StatusCode::BAD_REQUEST,
                    ErrorCode::AnalysisFilterInvalid,
                    "analysis_invalid_filter",
                )
            })?
    };
    Ok(app::Filter {
        recipient: f.recipient,
        recipients,
        from: f.from,
        until: f.until,
        event: f.event.clone(),
        search: f.search.clone(),
        source: f.source.clone(),
        offset: f.offset,
        limit: f.limit,
    })
}
pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/campaigns/{id}/analysis", get(read))
        .route("/api/campaigns/{id}/export", get(export))
}
async fn read(
    c: Context,
    Path(id): Path<i64>,
    query: Result<Query<Filter>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let user = c.api(None).await?;
    let Query(f) = query.map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::AnalysisFilterInvalid,
            "analysis_invalid_filter",
        )
    })?;
    let a = c
        .reporting()
        .analysis(user.identity, id, filter(&f)?)
        .await
        .map_err(error)?;
    let value = reads::Analysis {
        id: a.id,
        name: a.name,
        status: a.status,
        as_of: a.as_of,
        audience: a.audience,
        unique: a.unique,
        progression: a.progression,
        totals: a.totals,
        recipient_total: a.recipient_total,
        event_total: a.event_total,
        bucket_seconds: a.bucket_seconds,
        recipients: a
            .recipients
            .into_iter()
            .map(|r| reads::AnalysisRecipient {
                id: r.id,
                email: r.email,
                name: r.name,
                delivery: r.delivery,
                counts: r.counts,
                historical_reported: r.historical_reported,
            })
            .collect(),
        events: a
            .events
            .into_iter()
            .map(|e| reads::AnalysisEvent {
                id: e.id,
                recipient_id: e.recipient_id,
                email: e.email,
                time: e.time,
                kind: e.kind,
                source: e.source,
                password_present: e.password_present,
            })
            .collect(),
        buckets: a
            .buckets
            .into_iter()
            .map(|b| reads::Bucket {
                time: b.time,
                counts: b.counts,
            })
            .collect(),
    };
    response(StatusCode::OK, &json!(value), None)
}
async fn export(
    c: Context,
    Path(id): Path<i64>,
    query: Result<Query<Filter>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let user = c.api(None).await?;
    let Query(f) = query.map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::AnalysisFilterInvalid,
            "analysis_invalid_filter",
        )
    })?;
    let d = c
        .reporting()
        .export(user.identity, id, filter(&f)?, &f.format, &f.scope)
        .await
        .map_err(error)?;
    Ok((
        [
            (header::CONTENT_TYPE, d.content_type.to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"campaign-{id}.{}\"", d.extension),
            ),
            (header::CACHE_CONTROL, "no-store".into()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        ],
        d.bytes,
    )
        .into_response())
}

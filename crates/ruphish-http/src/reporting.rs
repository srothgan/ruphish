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
    let invalid = || {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::AnalysisFilterInvalid,
            "analysis_invalid_filter",
        )
    };
    let mut event = Vec::new();
    if !f.event.is_empty() {
        if f.event.len() > 160 {
            return Err(invalid());
        }
        for key in f.event.split(',') {
            let kind = app::Kind::from_key(key).ok_or_else(invalid)?;
            if !event.contains(&kind) {
                event.push(kind);
            }
        }
    }
    let source = if f.source.is_empty() {
        None
    } else {
        Some(app::Source::from_stored(&f.source).ok_or_else(invalid)?)
    };
    Ok(app::Filter {
        recipient: f.recipient,
        recipients,
        from: f.from,
        until: f.until,
        event,
        search: f.search.clone(),
        source,
        event_sort: f.event_sort,
        event_order: f.event_order,
        sort: f.sort,
        order: f.order,
        offset: f.offset,
        limit: f.limit,
    })
}
pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/campaigns/{id}/results", get(read))
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
    let value = reads::Results {
        first_observed: a.first_observed.into(),
        last_observed: a.last_observed.into(),
        id: a.id,
        name: a.name,
        status: a.status,
        as_of: a.as_of,
        audience: a.audience,
        unique: a.unique.into(),
        progression: a.progression.into(),
        totals: a.totals.into(),
        recipient_total: a.recipient_total,
        event_total: a.event_total,
        bucket_seconds: a.bucket_seconds,
        recipients: a
            .recipients
            .into_iter()
            .map(|r| reads::ResultRecipient {
                id: r.id,
                email: r.email,
                name: r.name,
                delivery: r.delivery,
                counts: r.counts.into(),
                historical_reported: r.historical_reported,
            })
            .collect(),
        events: a
            .events
            .into_iter()
            .map(|e| reads::ResultEvent {
                revision: e.revision,
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
                counts: b.counts.into(),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod filter_tests {
    use super::*;
    #[test]
    fn multiple_event_types_are_a_validated_set() {
        let parsed = filter(&Filter {
            event: "email_opened,link_clicked,email_opened".into(),
            ..Default::default()
        })
        .ok()
        .unwrap();
        assert_eq!(
            parsed.event,
            vec![app::Kind::EmailOpened, app::Kind::LinkClicked]
        );
        for event in [
            "email_opened,unknown",
            "email_opened,",
            ",",
            "Email Opened",
            "link_clicked OR 1=1",
        ] {
            assert!(
                filter(&Filter {
                    event: event.into(),
                    ..Default::default()
                })
                .is_err()
            );
        }
        assert!(
            filter(&Filter {
                event: "email_opened,".repeat(100),
                ..Default::default()
            })
            .is_err()
        );
    }
}

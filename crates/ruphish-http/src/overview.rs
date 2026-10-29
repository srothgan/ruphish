//! Owner-scoped dashboard reads, independently refreshable by purpose.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{error, response},
};
use axum::{
    Router,
    extract::{Query, rejection::QueryRejection},
    http::StatusCode,
    response::Response,
    routing::get,
};
use ruphish_application::overview as app;
use ruphish_contracts::errors::ErrorCode;
use ruphish_contracts::overview as wire;
use serde::Deserialize;

pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/overview", get(snapshot))
        .route("/api/overview/posture", get(posture))
}

fn counts(p: app::Posture) -> wire::Posture {
    wire::Posture {
        delivered: p.delivered,
        opened: p.opened,
        clicked: p.clicked,
        reported: p.reported,
        credentials_entered: p.credentials_entered,
    }
}
async fn snapshot(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let s = context
        .overview()
        .snapshot(
            user.identity,
            ruphish_application::datetime::now().as_second(),
        )
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &wire::Snapshot {
            as_of: s.as_of,
            recent: s
                .recent
                .into_iter()
                .map(|c| wire::RecentCampaign {
                    id: c.id,
                    name: c.name,
                    status: c.status,
                    launch: crate::response::stored_date(&c.launch),
                    recipients: c.recipients,
                    outcomes: counts(c.outcomes),
                })
                .collect(),
            inventory: wire::Inventory {
                templates: s.inventory.templates,
                pages: s.inventory.pages,
                groups: s.inventory.groups,
                profiles: s.inventory.profiles,
                domains: s.inventory.domains,
                domains_verified: s.inventory.domains_verified,
                reporting_mailboxes: s.inventory.reporting_mailboxes,
            },
            attention: wire::Attention {
                unverified_enabled_domains: s.attention.unverified_enabled_domains,
                delivery_problems: s.attention.delivery_problems,
                campaign_ids: s.attention.campaign_ids,
                next_launch: s.attention.next_launch.map(|n| wire::NextLaunch {
                    id: n.id,
                    name: n.name,
                    launch: n.launch,
                }),
            },
        },
        None,
    )
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct WindowQuery {
    window: String,
    // Authentication consumes this existing API query parameter before validation.
    #[serde(rename = "api_key")]
    _api_key: Option<String>,
}

async fn posture(
    context: Context,
    query: Result<Query<WindowQuery>, QueryRejection>,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let Query(q) = query.map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::OverviewWindowInvalid,
            "overview_invalid_window",
        )
    })?;
    let window = if q.window.is_empty() {
        app::Window::All
    } else {
        app::Window::parse(&q.window).ok_or_else(|| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::OverviewWindowInvalid,
                "overview_invalid_window",
            )
        })?
    };
    let now = ruphish_application::datetime::now().as_second();
    let p = context
        .overview()
        .posture(user.identity, window, now)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &wire::PostureResponse {
            window: window.as_str().into(),
            since: window.since(now),
            as_of: now,
            counts: counts(p.counts),
            previous: p.previous.map(|previous| wire::PreviousPeriod {
                since: previous.since,
                until: previous.until,
                counts: wire::InteractionCounts {
                    opened: previous.counts.opened,
                    clicked: previous.counts.clicked,
                    reported: previous.counts.reported,
                    credentials_entered: previous.counts.credentials_entered,
                },
            }),
        },
        None,
    )
}

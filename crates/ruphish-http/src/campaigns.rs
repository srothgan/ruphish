//! Legacy campaign routes with explicit mutation permissions and additive UI metadata.
mod directory;
mod drafts;
use crate::response::stored_date as date;
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::{encode, error, response},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use ruphish_application::datetime;
use ruphish_application::{
    campaign as app,
    resources::{Error, Kind},
};
use ruphish_contracts::{campaign as wire, campaign_workflow as workflow, legacy::Timestamp};
use ruphish_contracts::{campaign_details as details, resource_details as resource};
use ruphish_contracts::{campaign_reads as reads, errors::ErrorCode};
fn now() -> i64 {
    ruphish_application::datetime::now().as_second()
}
pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .merge(drafts::routes())
        .route("/api/campaigns/directory", get(directory::list))
        .route("/api/campaigns", get(slash).post(slash))
        .route("/api/campaigns/", get(list).post(create))
        .route("/api/campaigns/prepare", post(prepare))
        .route("/api/campaigns/test", post(test))
        .route("/api/campaigns/{id}/draft", get(duplicate))
        .route("/api/campaigns/summary", get(summaries))
        .route("/api/campaigns/overview", get(overview))
        .route("/api/campaigns/{id}", get(read).delete(delete))
        .route("/api/campaigns/{id}/summary", get(summary))
        .route("/api/campaigns/{id}/context", get(context))
        .route("/api/campaigns/{id}/complete", post(complete))
        .route("/api/campaigns/{id}/delivery", get(deliveries))
        .route(
            "/api/campaigns/{id}/recipients/{result}/history",
            get(recipient_history),
        )
        .route("/api/campaigns/{id}/delivery/{result}", post(resolve))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}
async fn slash(context: Context) -> Result<Response, Rejection> {
    context.api(None).await?;
    Ok((
        StatusCode::MOVED_PERMANENTLY,
        [(
            header::LOCATION,
            format!(
                "/api/campaigns/{}",
                context
                    .uri
                    .query()
                    .map(|s| format!("?{s}"))
                    .unwrap_or_default()
            ),
        )],
    )
        .into_response())
}
fn encode_result(r: &app::RecipientResult) -> details::Recipient {
    details::Recipient {
        id: r.rid.clone(),
        recipient: crate::resources::recipient(&r.recipient),
        status: r.status.clone(),
        ip: r.ip.clone(),
        latitude: r.latitude,
        longitude: r.longitude,
        reported: r.reported,
        send_date: date(&r.send_date),
        modified_date: date(&r.modified_date),
    }
}
fn events(c: &app::Campaign) -> Vec<details::Event> {
    c.events
        .iter()
        .map(|e| details::Event {
            campaign_id: c.id,
            email: e.email.clone(),
            time: date(&e.time),
            message: e.message.clone(),
            details: e.details.clone(),
        })
        .collect()
}
fn metadata(c: &app::Campaign) -> details::Metadata {
    details::Metadata {
        id: c.id,
        name: c.name.clone(),
        status: c.status,
        created_date: date(&c.created),
        launch_date: date(&c.launch),
        send_by_date: date(&c.send_by),
        completed_date: date(&c.completed),
    }
}
fn encode_campaign(c: &app::Campaign) -> Result<details::Campaign, Rejection> {
    let (mut template, mut page, mut smtp) = (None, None, None);
    for r in &c.resources {
        let resource::Resource { metadata, content } = encode(r);
        match content {
            resource::Content::Template(content) => {
                template = Some(resource::Resource { metadata, content })
            }
            resource::Content::Page(content) => {
                page = Some(resource::Resource { metadata, content })
            }
            resource::Content::Profile(content) => {
                smtp = Some(resource::Resource { metadata, content })
            }
            _ => return Err(error(Error::Internal)),
        }
    }
    Ok(details::Campaign {
        metadata: metadata(c),
        url: c.url.clone(),
        template: template.ok_or_else(|| error(Error::Internal))?,
        page: page.ok_or_else(|| error(Error::Internal))?,
        smtp: smtp.ok_or_else(|| error(Error::Internal))?,
        results: c.results.iter().map(encode_result).collect(),
        timeline: events(c),
        groups: None,
    })
}
fn encode_summary(c: &app::Campaign) -> details::Summary {
    let [sent, opened, clicked, submitted, reported] = c.progression();
    let failed = c.results.iter().filter(|r| r.status == "Error").count();
    details::Summary {
        metadata: metadata(c),
        stats: details::Counts {
            total: c.results.len(),
            sent,
            opened,
            clicked,
            submitted_data: submitted,
            email_reported: reported,
            error: failed,
        },
    }
}
fn decode(context: &Context, bytes: &[u8]) -> Result<app::Create, Rejection> {
    let input: wire::Create = serde_json::from_slice(bytes).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid JSON structure",
        )
    })?;
    command(context, input)
}
fn command(context: &Context, input: wire::Create) -> Result<app::Create, Rejection> {
    use ruphish_application::validation::{Code, Report};
    use sha2::{Digest, Sha256};
    let mut report = Report::default();
    for (conflict, absolute, local) in [
        (
            input.launch_date.is_some() && input.launch_local.is_some(),
            "launch_date",
            "launch_local.local",
        ),
        (
            input.send_by_date.is_some() && input.send_by_local.is_some(),
            "send_by_date",
            "send_by_local.local",
        ),
    ] {
        if conflict {
            for field in [absolute, local] {
                report.reject(
                    field,
                    Code::Invalid,
                    "Supply either an absolute or a local time for each schedule boundary",
                );
            }
        }
    }
    report.finish().map_err(|r| error(Error::Validation(r)))?;
    let fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(&input).map_err(
        |_| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::CampaignInvalid,
                "Invalid campaign request",
            )
        },
    )?));
    let select = |s: wire::ReviewedResource| app::Selection {
        id: s.id,
        revision: s.revision,
    };
    let timestamp = |t: Option<Timestamp>| t.map(|t| t.0.as_second());
    let p = input.policy.unwrap_or_default();
    let mut result = app::Create {
        name: input.name,
        groups: input.selection.groups.into_iter().map(select).collect(),
        template: select(input.selection.template),
        page: select(input.selection.page),
        profile: select(input.selection.smtp),
        url: input.url,
        launch: timestamp(input.launch_date),
        send_by: timestamp(input.send_by_date),
        policy: app::Overrides {
            redirect_url: p.redirect_url,
            learning_page_id: p.learning_page_id,
        },
        idempotency: None,
    };
    for (local, target, field) in [
        (input.launch_local, &mut result.launch, "launch_local.local"),
        (
            input.send_by_local,
            &mut result.send_by,
            "send_by_local.local",
        ),
    ] {
        if let Some(local) = local {
            *target = Some(
                context
                    .campaigns()
                    .resolve_time(&local.local, &local.time_zone)
                    .map_err(|e| error(e.at(field)))?,
            );
        }
    }
    if let Some(key) = context.headers.get("idempotency-key") {
        let key = key.to_str().map_err(|_| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::LaunchKeyInvalid,
                "Invalid launch idempotency key",
            )
        })?;
        result.idempotency = Some((key.into(), fingerprint));
    }
    Ok(result)
}
fn launch_error(e: Error) -> Rejection {
    match e {
        Error::NotFound(kind) => {
            let message = if kind == "SMTP" {
                "Sending profile not found".into()
            } else {
                format!("{kind} not found")
            };
            let mut report = ruphish_application::validation::Report::default();
            let field = match kind {
                "SMTP" => "selection.smtp.id",
                "Template" => "selection.template.id",
                "Page" => "selection.page.id",
                "Group" => "selection.groups",
                "Learning page" => "policy.learning_page_id",
                _ => "url",
            };
            report.reject(
                field,
                ruphish_application::validation::Code::Unavailable,
                "Resource not found",
            );
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::ResourceNotFound,
                &message,
            )
            .with_validation(&report)
        }
        other => error(other),
    }
}
async fn prepare(context: Context, body: Bytes) -> Result<Response, Rejection> {
    prepare_inner(context, body, false).await
}
async fn prepare_inner(context: Context, body: Bytes, draft: bool) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let input = decode(&context, &body)?;
    if draft && input.launch.is_some_and(|time| time < now()) {
        return Err(error(Error::DraftSchedulePast));
    }

    let p = context
        .campaigns()
        .prepare(user.identity, &input, now())
        .await
        .map_err(launch_error)?;
    let selected = |r: &ruphish_application::resources::Resource| workflow::ReviewedResource {
        id: r.id,
        name: r.name.clone(),
        revision: r.modified_date.clone(),
    };
    let attributes =
        |r: &ruphish_application::resources::Recipient| workflow::RecipientAttributes {
            first_name: r.first_name.clone(),
            last_name: r.last_name.clone(),
            position: r.position.clone(),
        };
    let total: usize = p.groups.iter().map(|g| g.recipient_count).sum();
    let selection = workflow::Selection {
        groups: p
            .groups
            .iter()
            .map(|g| workflow::ReviewedResource {
                id: g.id,
                name: g.name.clone(),
                revision: g.modified_date.clone(),
            })
            .collect(),
        template: selected(&p.template),
        page: selected(&p.page),
        smtp: selected(&p.profile),
    };
    let conflicts = p
        .conflicts
        .iter()
        .map(|(kept, discarded)| workflow::Conflict {
            email: kept.email.clone(),
            kept: attributes(kept),
            discarded: attributes(discarded),
        })
        .collect();
    let count = p.recipients.len();
    let conflict_count = p.conflict_count;
    let launch_date = p.launch;
    let policy = read_policy(&p.policy);
    let warnings = if let ruphish_application::resources::Content::Page(page) = &p.page.content {
        crate::resources::warnings(
            context
                .resources()
                .page_warnings(&page.html)
                .await
                .map_err(error)?,
        )
    } else {
        Vec::new()
    };
    let preview = context
        .delivery()
        .review_campaign(p, input.url)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &workflow::Review {
            warnings,
            total: count,
            duplicate_count: total - count,
            launch_date,
            send_by_date: input.send_by,
            selection,
            policy,
            mailer_enabled: context.mailer_enabled(),
            conflict_count,
            conflicts,
            preview: workflow::Preview {
                subject: preview.subject,
                html: preview.html,
                text: preview.text,
                visible_sender: preview.visible_sender,
                transport_sender: preview.transport_sender,
            },
        },
        None,
    )
}

async fn create(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let input = decode(&context, &body)?;
    let campaign = context
        .campaigns()
        .create(user.identity, input, now())
        .await
        .map_err(launch_error)?;
    let mut value = encode_campaign(&campaign)?;
    value.timeline.clear();
    value.groups = Some(
        campaign
            .groups
            .iter()
            .map(|group| {
                let resource::Resource { metadata, content } = encode(group);
                match content {
                    resource::Content::Group(content) => {
                        Ok(resource::Resource { metadata, content })
                    }
                    _ => Err(error(Error::Internal)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    response(StatusCode::CREATED, &value, None)
}
async fn list(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let permit = context.campaign_list_permit()?;
    let rows = context
        .campaigns()
        .list(user.identity)
        .await
        .map_err(error)?;
    let response = response(
        StatusCode::OK,
        &rows
            .iter()
            .map(encode_campaign)
            .collect::<Result<Vec<_>, _>>()?,
        None,
    )?;
    // Retain admission until the body is consumed or the client disconnects.
    // Slow readers must not accumulate unlimited already-serialized lists.
    use futures_util::StreamExt;
    let (parts, body) = response.into_parts();
    let stream = body.into_data_stream().map(move |frame| {
        let _keep_alive = &permit;
        frame
    });
    Ok(Response::from_parts(
        parts,
        axum::body::Body::from_stream(stream),
    ))
}
async fn read(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let c = context
        .campaigns()
        .get(user.identity, id)
        .await
        .map_err(error)?;
    response(StatusCode::OK, &encode_campaign(&c)?, None)
}
async fn context(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let campaign = context
        .campaigns()
        .header(user.identity, id)
        .await
        .map_err(error)?;
    response(StatusCode::OK, &encode_header(&campaign), None)
}
async fn summary(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let c = context
        .campaigns()
        .get(user.identity, id)
        .await
        .map_err(error)?;
    response(StatusCode::OK, &encode_summary(&c), None)
}
async fn summaries(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let rows = context
        .campaigns()
        .list(user.identity)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &details::Summaries {
            total: rows.len(),
            campaigns: rows.iter().map(encode_summary).collect(),
        },
        None,
    )
}
async fn complete(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    context
        .campaigns()
        .complete(user.identity, id, now())
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &ruphish_contracts::legacy::Envelope::<()> {
            success: true,
            message: "Campaign completed successfully!".into(),
            data: None,
        },
        None,
    )
}
async fn delete(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    context
        .campaigns()
        .delete(user.identity, id)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &ruphish_contracts::legacy::Envelope::<()> {
            success: true,
            message: "Campaign deleted successfully!".into(),
            data: None,
        },
        None,
    )
}
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Page {
    after: i64,
    limit: Option<i64>,
    completed: Option<bool>,
}
fn read_policy(p: &app::InteractionPolicy) -> reads::Policy {
    reads::Policy {
        redirect_url: p.redirect_url.clone(),
        learning_page_id: p.learning_page_id,
    }
}
fn encode_header(c: &app::CampaignHeader) -> reads::Header {
    let resource = |r: &app::CampaignResource| reads::Resource {
        id: r.id,
        name: r.name.clone(),
    };
    reads::Header {
        id: c.id,
        name: c.name.clone(),
        status: c.status,
        launch_date: date(&c.launch),
        send_by_date: date(&c.send_by),
        url: c.url.clone(),
        template: resource(&c.template),
        page: resource(&c.page),
        smtp: resource(&c.profile),
        subject: c.subject.clone(),
        profile_sender: c.profile_sender.clone(),
        template_sender: c.template_sender.clone(),
        total: c.total,
        policy: c.policy.as_ref().map(read_policy),
        domain_id: c.domain_id,
        domain_block: c.domain_block.clone(),
    }
}
async fn overview(context: Context, Query(page): Query<Page>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let rows = context
        .campaigns()
        .overview_filtered(
            user.identity,
            page.after,
            page.limit.unwrap_or(50),
            page.completed,
        )
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &reads::Overview {
            items: rows.items.iter().map(encode_header).collect(),
            next_after: rows.next_after,
            mailer_enabled: context.mailer_enabled(),
        },
        None,
    )
}
#[derive(Default, serde::Deserialize)]
#[serde(default)]
struct DeliveryPage {
    after: i64,
    limit: Option<i64>,
    sort: Option<ruphish_application::pagination::DeliverySort>,
    order: Option<ruphish_application::reporting::SortDirection>,
    cursor: String,
}
async fn deliveries(
    context: Context,
    Path(id): Path<i64>,
    query: Result<Query<DeliveryPage>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let invalid = || error(Error::Invalid("Invalid delivery page query".into()));
    let Query(page) = query.map_err(|_| invalid())?;
    let sorting = if let Some(sort) = page.sort {
        if page.after != 0 || page.limit.is_some_and(|limit| !(1..=200).contains(&limit)) {
            return Err(invalid());
        }
        Some(directory::sorted_page(
            sort,
            page.order.unwrap_or_default(),
            format!("delivery:{}:{id}", user.identity.id),
            &page.cursor,
        )?)
    } else {
        if !page.cursor.is_empty() || page.order.is_some() {
            return Err(invalid());
        }
        None
    };
    let view = context
        .campaigns()
        .deliveries_sorted(
            user.identity,
            id,
            page.after,
            page.limit.unwrap_or(50),
            sorting,
        )
        .await
        .map_err(error)?;
    let campaign = &view.campaign;
    let value = reads::Delivery {
        campaign: encode_header(campaign),
        counts: view.counts.into_iter().collect(),
        items: view
            .items
            .iter()
            .map(|r| reads::DeliveryRow {
                id: r.id,
                rid: r.rid.clone(),
                email: r.email.clone(),
                attempt_count: r.attempt_count,
                state: r.state,
                due: r.due,
                retries: r.retries,
                generation: r.generation,
                code: r.code.clone(),
                attempts: r.attempts.iter().map(encode_attempt).collect(),
            })
            .collect(),
        next_after: view.next_after,
        next_cursor: view.next_cursor,
        policy: campaign.policy.as_ref().map(read_policy),
        mailer_enabled: context.mailer_enabled(),
        activity: view
            .activity
            .iter()
            .map(|e| reads::Activity {
                id: e.id,
                time: date(&e.time),
                email: e.email.clone(),
                message: e.message.clone(),
                details: e.details.clone(),
            })
            .collect(),
    };
    response(StatusCode::OK, &value, None)
}
#[derive(serde::Deserialize)]
struct HistoryPage {
    before_attempt: Option<i64>,
    before_event: Option<i64>,
}
async fn recipient_history(
    context: Context,
    Path((campaign, result)): Path<(i64, i64)>,
    Query(page): Query<HistoryPage>,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let history = context
        .campaigns()
        .recipient_history(
            user.identity,
            campaign,
            result,
            page.before_attempt,
            page.before_event,
        )
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &reads::History {
            result_id: history.result_id,
            email: history.email,
            attempts: history.attempts.iter().map(encode_attempt).collect(),
            events: history
                .events
                .iter()
                .map(|e| {
                    Ok(reads::HistoryEvent {
                        record: e.record.clone().ok_or_else(|| {
                            error(ruphish_application::resources::Error::Internal)
                        })?,
                        id: e.id,
                        time: date(&e.time),
                        at: datetime::parse_stored(&e.time)
                            .ok()
                            .filter(|t| *t != datetime::ZERO_TIMESTAMP)
                            .map(|t| t.as_second()),
                        message: e.message.clone(),
                        details: e.details.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Rejection>>()?,
            next_attempt: history.next_attempt,
            next_event: history.next_event,
        },
        None,
    )
}
fn encode_attempt(a: &app::Attempt) -> reads::Attempt {
    reads::Attempt {
        generation: a.generation,
        started: a.started,
        finished: a.finished,
        state: a.state,
        stage: a.stage.clone(),
        smtp_code: a.smtp_code,
        message_id: a.message_id.clone(),
    }
}

#[derive(serde::Deserialize)]
struct Resolve {
    generation: i64,
    action: String,
}
async fn resolve(
    context: Context,
    Path((campaign, result)): Path<(i64, i64)>,
    body: Bytes,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let r: Resolve = serde_json::from_slice(&body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::DeliveryActionInvalid,
            "Invalid delivery action",
        )
    })?;
    context
        .campaigns()
        .resolve(
            user.identity,
            campaign,
            result,
            r.generation,
            &r.action,
            now(),
        )
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &ruphish_contracts::legacy::Envelope::<()> {
            success: true,
            message: "Delivery updated".into(),
            data: None,
        },
        None,
    )
}

async fn test(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let request: wire::Test = serde_json::from_slice(&body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid test request",
        )
    })?;
    let input = command(&context, request.campaign)?;
    let recipient = ruphish_application::resources::Recipient {
        email: request.email,
        first_name: "Sample".into(),
        last_name: "Recipient".into(),
        position: "Training".into(),
    };
    let prepared = context
        .campaigns()
        .prepare(user.identity, &input, now())
        .await
        .map_err(launch_error)?;
    context
        .delivery()
        .test_campaign(user.identity, prepared, recipient, input.url)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &ruphish_contracts::legacy::Envelope::<()> {
            success: true,
            message: "Accepted by SMTP server".into(),
            data: None,
        },
        None,
    )
}
async fn duplicate(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let campaign = context
        .campaigns()
        .get(user.identity, id)
        .await
        .map_err(error)?;
    let mut selection = workflow::Selection {
        groups: campaign
            .groups
            .iter()
            .map(|r| workflow::Reference { id: r.id })
            .collect(),
        template: workflow::Reference { id: 0 },
        page: workflow::Reference { id: 0 },
        smtp: workflow::Reference { id: 0 },
    };
    for resource in &campaign.resources {
        let selected = match resource.kind() {
            Kind::Template => &mut selection.template,
            Kind::Page => &mut selection.page,
            Kind::Profile => &mut selection.smtp,
            Kind::Group | Kind::LearningPage => continue,
        };
        selected.id = resource.id;
    }
    response(
        StatusCode::OK,
        &workflow::Duplicate {
            name: format!("{} (copy)", campaign.name),
            url: campaign.url,
            selection,
            policy: campaign.policy.as_ref().map(read_policy),
        },
        None,
    )
}

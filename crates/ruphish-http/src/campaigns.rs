//! Legacy campaign routes with explicit mutation permissions and additive UI metadata.
mod directory;
mod drafts;
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
use ruphish_contracts::{campaign_reads as reads, errors::ErrorCode};
use serde_json::{Value, json};
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
        .route("/api/campaigns/{id}/results", get(results))
        .route(
            "/api/campaigns/{id}/complete",
            get(complete_legacy).post(complete),
        )
        .route("/api/campaigns/{id}/delivery", get(deliveries))
        .route(
            "/api/campaigns/{id}/delivery/{result}/history",
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
fn date(value: &str) -> String {
    if value.is_empty() {
        return "0001-01-01T00:00:00Z".into();
    }
    datetime::parse_stored(value)
        .map(datetime::utc)
        .unwrap_or_else(|_| value.replacen(' ', "T", 1))
}
fn encode_result(r: &app::RecipientResult) -> Value {
    json!({"id":r.rid,"email":r.recipient.email,"first_name":r.recipient.first_name,"last_name":r.recipient.last_name,"position":r.recipient.position,"status":r.status,"ip":r.ip,"latitude":r.latitude,"longitude":r.longitude,"reported":r.reported,"send_date":date(&r.send_date),"modified_date":date(&r.modified_date)})
}
fn events(c: &app::Campaign) -> Vec<Value> {
    c.events.iter().map(|e|json!({"campaign_id":c.id,"email":e.email,"time":date(&e.time),"message":e.message,"details":e.details})).collect()
}
fn base(c: &app::Campaign) -> Value {
    json!({"id":c.id,"name":c.name,"status":c.status,"created_date":date(&c.created),"launch_date":date(&c.launch),"send_by_date":date(&c.send_by),"completed_date":date(&c.completed)})
}
fn encode_campaign(c: &app::Campaign) -> Value {
    let mut v = base(c);
    v["url"] = json!(c.url);
    for r in &c.resources {
        v[match r.kind() {
            Kind::Template => "template",
            Kind::Page => "page",
            Kind::Profile => "smtp",
            Kind::Group => "group",
            Kind::LearningPage => "learning_page",
        }] = encode(r);
    }
    if !c.results.is_empty() {
        v["results"] = json!(c.results.iter().map(encode_result).collect::<Vec<_>>());
    }
    if !c.events.is_empty() {
        v["timeline"] = json!(events(c));
    }
    v
}
fn encode_summary(c: &app::Campaign) -> Value {
    let mut v = base(c);
    let [sent, opened, clicked, submitted, reported] = c.progression();
    let failed = c.results.iter().filter(|r| r.status == "Error").count();
    v["stats"] = json!({"total":c.results.len(),"sent":sent,"opened":opened,"clicked":clicked,"submitted_data":submitted,"email_reported":reported,"error":failed});
    v
}
fn decode(context: &Context, bytes: &[u8]) -> Result<app::Create, Rejection> {
    let input: wire::Create = ruphish_contracts::legacy::decode_object(bytes).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid JSON structure",
        )
    })?;
    use sha2::{Digest, Sha256};
    let fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(&input).map_err(
        |_| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::CampaignInvalid,
                "Invalid campaign request",
            )
        },
    )?));
    let select = |s: wire::NamedResource| app::Selection {
        name: s.name.unwrap_or_default(),
        ..Default::default()
    };
    let timestamp = |t: Option<Timestamp>| {
        t.filter(|t| *t != Timestamp::default())
            .map(|t| t.0.as_second())
    };
    let p = input.policy.unwrap_or_default();
    let mut result = app::Create {
        name: input.name.unwrap_or_default(),
        groups: input
            .groups
            .unwrap_or_default()
            .into_iter()
            .map(select)
            .collect(),
        template: select(input.template.unwrap_or_default()),
        page: select(input.page.unwrap_or_default()),
        profile: select(input.smtp.unwrap_or_default()),
        url: input.url.unwrap_or_default(),
        launch: timestamp(input.launch_date),
        send_by: timestamp(input.send_by_date),
        policy: app::Overrides {
            capture_credentials: Some(true),
            capture_passwords: Some(true),
            redirect_url: p.redirect_url,
            learning_page_id: p.learning_page_id,
        },
        idempotency: None,
    };
    if let Some(s) = input.selection {
        let stable = |s: wire::ReviewedResource| app::Selection {
            id: Some(s.id),
            revision: s.revision,
            name: String::new(),
        };
        result.groups = s.groups.into_iter().map(stable).collect();
        result.template = stable(s.template);
        result.page = stable(s.page);
        result.profile = stable(s.smtp);
    }
    for (local, target) in [
        (input.launch_local, &mut result.launch),
        (input.send_by_local, &mut result.send_by),
    ] {
        if let Some(local) = local {
            *target = Some(
                context
                    .campaigns()
                    .resolve_time(&local.local, &local.time_zone)
                    .map_err(error)?,
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
        Error::NotFound("SMTP") => failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::ResourceNotFound,
            "Sending profile not found",
        ),
        Error::NotFound(kind) => failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::ResourceNotFound,
            &format!("{kind} not found"),
        ),
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
    let total: usize = p
        .groups
        .iter()
        .map(|g| match &g.content {
            ruphish_application::resources::Content::Group(rows) => rows.len(),
            _ => 0,
        })
        .sum();
    let selection = workflow::Selection {
        groups: p.groups.iter().map(selected).collect(),
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
        context
            .resources()
            .page_warnings(&page.html)
            .map_err(error)?
            .into_iter()
            .map(|warning| ruphish_contracts::resource_details::PageWarning {
                code: warning.code.into(),
                control: warning.control,
            })
            .collect()
    } else {
        Vec::new()
    };
    let preview = context
        .delivery()
        .review_campaign(p, input.url)
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
    let mut value = encode_campaign(&campaign);
    if let Some(object) = value.as_object_mut() {
        object.remove("timeline");
    }
    value["groups"] = json!(campaign.groups.iter().map(encode).collect::<Vec<_>>());
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
        &json!(rows.iter().map(encode_campaign).collect::<Vec<_>>()),
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
    response(StatusCode::OK, &encode_campaign(&c), None)
}
async fn results(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let c = context
        .campaigns()
        .get(user.identity, id)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!({"id":c.id,"name":c.name,"status":c.status,"results":c.results.iter().map(encode_result).collect::<Vec<_>>(),"timeline":events(&c)}),
        None,
    )
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
        &json!({"total":rows.len(),"campaigns":rows.iter().map(encode_summary).collect::<Vec<_>>()}),
        None,
    )
}
async fn complete_legacy(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    if !context.explicit_api_key() {
        return Err(failure(
            StatusCode::METHOD_NOT_ALLOWED,
            ErrorCode::MethodNotAllowed,
            "Use POST to complete a campaign",
        ));
    }
    complete(context, Path(id)).await
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
        &json!({"success":true,"message":"Campaign completed successfully!","data":null}),
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
        &json!({"success":true,"message":"Campaign deleted successfully!","data":null}),
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
        capture_credentials: true,
        capture_passwords: true,
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
        status: c.status.clone(),
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
        &json!({"items":rows.items.iter().map(encode_header).collect::<Vec<_>>(),"next_after":rows.next_after,"mailer_enabled":context.mailer_enabled()}),
        None,
    )
}
async fn deliveries(
    context: Context,
    Path(id): Path<i64>,
    Query(page): Query<Page>,
) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let view = context
        .campaigns()
        .deliveries(user.identity, id, page.after, page.limit.unwrap_or(50))
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
                state: r.state.clone(),
                due: r.due,
                retries: r.retries,
                generation: r.generation,
                code: r.code.clone(),
                attempts: r.attempts.iter().map(encode_attempt).collect(),
            })
            .collect(),
        next_after: view.next_after,
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
    response(StatusCode::OK, &json!(value), None)
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
        &json!(reads::History {
            result_id: history.result_id,
            email: history.email,
            attempts: history.attempts.iter().map(encode_attempt).collect(),
            events: history
                .events
                .iter()
                .map(|e| reads::HistoryEvent {
                    id: e.id,
                    time: date(&e.time),
                    at: datetime::parse_stored(&e.time).ok().map(|t| t.as_second()),
                    message: e.message.clone(),
                    details: e.details.clone()
                })
                .collect(),
            next_attempt: history.next_attempt,
            next_event: history.next_event,
        }),
        None,
    )
}
fn encode_attempt(a: &app::Attempt) -> reads::Attempt {
    reads::Attempt {
        generation: a.generation,
        started: a.started,
        finished: a.finished,
        state: a.state.clone(),
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
        &json!({"success":true,"message":"Delivery updated","data":null}),
        None,
    )
}

async fn test(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid test request",
        )
    })?;
    let campaign = serde_json::to_vec(&value["campaign"]).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::CampaignInvalid,
            "Invalid campaign",
        )
    })?;
    let input = decode(&context, &campaign)?;
    let recipient = ruphish_application::resources::Recipient {
        email: value["email"].as_str().unwrap_or_default().into(),
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
        &json!({"success":true,"message":"Accepted by SMTP server","data":null}),
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

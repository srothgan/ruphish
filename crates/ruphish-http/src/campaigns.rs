//! Legacy campaign routes with explicit mutation permissions and additive UI metadata.
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
use ruphish_contracts::{campaign as wire, legacy::Timestamp};
use serde_json::{Value, json};
fn now() -> i64 {
    ruphish_application::datetime::now().as_second()
}
pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
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
fn policy(p: &app::CapturePolicy) -> Value {
    json!({"capture_credentials":p.capture_credentials,"capture_passwords":p.capture_passwords,"redirect_url":p.redirect_url})
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
    let input: wire::Create = ruphish_contracts::legacy::decode_object(bytes)
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid JSON structure"))?;
    use sha2::{Digest, Sha256};
    let fingerprint = hex::encode(Sha256::digest(
        serde_json::to_vec(&input)
            .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid campaign request"))?,
    ));
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
            capture_credentials: p.capture_credentials,
            capture_passwords: p.capture_passwords,
            redirect_url: p.redirect_url,
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
        let key = key
            .to_str()
            .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid launch idempotency key"))?;
        result.idempotency = Some((key.into(), fingerprint));
    }
    Ok(result)
}
fn launch_error(e: Error) -> Rejection {
    match e {
        Error::NotFound("SMTP") => failure(StatusCode::BAD_REQUEST, "Sending profile not found"),
        Error::NotFound(kind) => failure(StatusCode::BAD_REQUEST, &format!("{kind} not found")),
        other => error(other),
    }
}
async fn prepare(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let input = decode(&context, &body)?;
    let p = context
        .campaigns()
        .prepare(user.identity, &input, now())
        .await
        .map_err(launch_error)?;
    let selected = |r: &ruphish_application::resources::Resource| json!({"id":r.id,"name":r.name,"revision":r.modified_date});
    let total: usize = p
        .groups
        .iter()
        .map(|g| match &g.content {
            ruphish_application::resources::Content::Group(rows) => rows.len(),
            _ => 0,
        })
        .sum();
    let mut value = json!({"total":p.recipients.len(),"duplicate_count":total-p.recipients.len(),"launch_date":p.launch,"send_by_date":input.send_by,"selection":{"groups":p.groups.iter().map(selected).collect::<Vec<_>>(),"template":selected(&p.template),"page":selected(&p.page),"smtp":selected(&p.profile)},"policy":policy(&p.policy),"mailer_enabled":context.mailer_enabled()});
    value["conflict_count"] = json!(p.conflict_count);
    value["conflicts"] = json!(p.conflicts.iter().map(|(kept,discarded)|json!({"email":kept.email,"kept":{"first_name":kept.first_name,"last_name":kept.last_name,"position":kept.position},"discarded":{"first_name":discarded.first_name,"last_name":discarded.last_name,"position":discarded.position}})).collect::<Vec<_>>());
    let preview = context
        .delivery()
        .review_campaign(p, input.url)
        .map_err(error)?;
    value["preview"] = json!({"subject":preview.subject,"html":preview.html,"text":preview.text,"visible_sender":preview.visible_sender,"transport_sender":preview.transport_sender});
    response(StatusCode::OK, &value, None)
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
    let rows = context
        .campaigns()
        .list(user.identity)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!(rows.iter().map(encode_campaign).collect::<Vec<_>>()),
        None,
    )
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
}
fn encode_header(c: &app::CampaignHeader) -> Value {
    let resource = |r: &app::CampaignResource| json!({"id":r.id,"name":r.name});
    json!({"id":c.id,"name":c.name,"status":c.status,"launch_date":date(&c.launch),"send_by_date":date(&c.send_by),"url":c.url,"template":resource(&c.template),"page":resource(&c.page),"smtp":resource(&c.profile),"subject":c.subject,"profile_sender":c.profile_sender,"template_sender":c.template_sender,"total":c.total,"policy":c.policy.as_ref().map(policy),"domain_id":c.domain_id,"domain_block":c.domain_block})
}
async fn overview(context: Context, Query(page): Query<Page>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let rows = context
        .campaigns()
        .overview(user.identity, page.after, page.limit.unwrap_or(50))
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
    let counts: serde_json::Map<String, Value> = view
        .counts
        .into_iter()
        .map(|(state, count)| (state, json!(count)))
        .collect();
    let items:Vec<_>=view.items.iter().map(|r|json!({"id":r.id,"rid":r.rid,"email":r.email,"attempt_count":r.attempt_count,"state":r.state,"due":r.due,"retries":r.retries,"generation":r.generation,"code":r.code,"attempts":r.attempts.iter().map(encode_attempt).collect::<Vec<_>>()})).collect();
    response(
        StatusCode::OK,
        &json!({"campaign":encode_header(campaign),"counts":counts,"items":items,"next_after":view.next_after,"policy":campaign.policy.as_ref().map(policy),"mailer_enabled":context.mailer_enabled(),"activity":view.activity.iter().map(|e|json!({"id":e.id,"time":date(&e.time),"email":e.email,"message":e.message,"details":e.details})).collect::<Vec<_>>()}),
        None,
    )
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
        &json!({
            "result_id":history.result_id,"email":history.email,
            "attempts":history.attempts.iter().map(encode_attempt).collect::<Vec<_>>(),
            "events":history.events.iter().map(|e|json!({"id":e.id,"time":date(&e.time),"at":datetime::parse_stored(&e.time).ok().map(|t|t.as_second()),"message":e.message,"details":e.details})).collect::<Vec<_>>(),
            "next_attempt":history.next_attempt,"next_event":history.next_event
        }),
        None,
    )
}
fn encode_attempt(a: &app::Attempt) -> Value {
    json!({"generation":a.generation,"started":a.started,"finished":a.finished,"state":a.state,"stage":a.stage,"smtp_code":a.smtp_code,"message_id":a.message_id})
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
    let r: Resolve = serde_json::from_slice(&body)
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid delivery action"))?;
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
    let value: Value = serde_json::from_slice(&body)
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid test request"))?;
    let campaign = serde_json::to_vec(&value["campaign"])
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid campaign"))?;
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
    let mut selection = json!({"groups":campaign.groups.iter().map(|r|json!({"id":r.id})).collect::<Vec<_>>(),"template":{"id":0},"page":{"id":0},"smtp":{"id":0}});
    for resource in campaign.resources {
        let key = match resource.kind() {
            Kind::Template => "template",
            Kind::Page => "page",
            Kind::Profile => "smtp",
            Kind::Group => continue,
        };
        selection[key] = json!({"id":resource.id});
    }
    response(
        StatusCode::OK,
        &json!({"name":format!("{} (copy)",campaign.name),"url":campaign.url,"selection":selection,"policy":campaign.policy.as_ref().map(policy).unwrap_or_else(||json!({}))}),
        None,
    )
}

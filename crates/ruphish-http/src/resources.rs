//! Legacy resource API mapping; authorization and CSRF share the account boundary.
use crate::auth::{AuthState, Context, Rejection, failure};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use ruphish_application::resources::{self as app, Content, Kind, Resource};
use ruphish_contracts::errors::ErrorCode;
use ruphish_contracts::legacy as wire;
use serde_json::{Value, json};

pub(crate) fn routes() -> Router<AuthState> {
    let mut routes = Router::new();
    for family in ["groups", "templates", "pages", "learning-pages", "smtp"] {
        routes = routes
            .route(&format!("/api/{family}"), get(slash).post(slash))
            .route(&format!("/api/{family}/"), get(list).post(create))
            .route(&format!("/api/{family}/summary"), get(summaries))
            .route(
                &format!("/api/{family}/{{id}}"),
                get(read).put(update).delete(delete),
            );
    }
    routes
        .route("/api/pages/preview", axum::routing::post(preview_landing))
        .route("/api/pages/validate", axum::routing::post(validate_page))
        .route(
            "/api/learning-pages/validate",
            axum::routing::post(preview_learning),
        )
        .route(
            "/api/learning-pages/preview",
            axum::routing::post(preview_learning),
        )
        .route("/api/groups/{id}/summary", get(group_summary))
        .route("/api/import/group", axum::routing::post(import_group))
        .route(
            "/api/import/group/review",
            axum::routing::post(review_group),
        )
        .route("/api/import/email", axum::routing::post(import_email))
        .route("/api/import/site", axum::routing::post(import_site))
        .route("/api/util/preview", axum::routing::post(preview))
        .route("/api/util/send_test_email", axum::routing::post(send_test))
        .route(
            "/api/util/test_connection",
            axum::routing::post(test_connection),
        )
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
}

async fn editor_identity(
    context: &Context,
) -> Result<ruphish_application::auth::Identity, Rejection> {
    let user = context.api(None).await?;
    user.require("modify_objects")
        .map_err(|e| error(e.into()))?;
    Ok(user.identity)
}
fn parse<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Rejection> {
    serde_json::from_slice(body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid JSON structure",
        )
    })
}
fn recipients(rows: &[app::Recipient]) -> Value {
    json!(rows.iter().map(|r|json!({"email":r.email,"first_name":r.first_name,"last_name":r.last_name,"position":r.position})).collect::<Vec<_>>())
}
async fn import_group(
    context: Context,
    mut multipart: axum::extract::Multipart,
) -> Result<Response, Rejection> {
    editor_identity(&context).await?;
    let mut targets = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::CsvInvalid,
            "Invalid CSV upload",
        )
    })? {
        if field.file_name().is_none() {
            continue;
        }
        let bytes = field.bytes().await.map_err(|_| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::CsvInvalid,
                "Cannot read CSV upload",
            )
        })?;
        let report = context
            .resources()
            .tools()
            .csv(&bytes, None)
            .map_err(error)?;
        if !report.errors.is_empty() {
            return Err(failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::CsvRowsInvalid,
                "CSV contains invalid rows. Use import review to inspect them.",
            ));
        }
        targets.extend(report.recipients);
    }
    response(StatusCode::OK, &recipients(&targets), None)
}
#[derive(serde::Deserialize)]
struct CsvRequest {
    content: String,
    columns: Option<[String; 4]>,
}
async fn review_group(context: Context, body: Bytes) -> Result<Response, Rejection> {
    editor_identity(&context).await?;
    let request: CsvRequest = parse(&body)?;
    let report = context
        .resources()
        .tools()
        .csv(request.content.as_bytes(), request.columns)
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!({"targets":recipients(&report.recipients),"columns":report.columns,"errors":report.errors.iter().map(|e|json!({"row":e.row,"code": match e.kind { app::RowErrorKind::Encoding => ErrorCode::CsvRowEncoding, app::RowErrorKind::Columns => ErrorCode::CsvRowColumns, app::RowErrorKind::Email => ErrorCode::CsvRowEmail },"message":e.message,"fields":e.fields})).collect::<Vec<_>>()}),
        None,
    )
}
#[derive(serde::Deserialize)]
struct EmailRequest {
    content: String,
    #[serde(default)]
    convert_links: bool,
}
async fn import_email(context: Context, body: Bytes) -> Result<Response, Rejection> {
    editor_identity(&context).await?;
    let request: EmailRequest = parse(&body)?;
    let imported = context
        .resources()
        .tools()
        .email(request.content.as_bytes(), request.convert_links)
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!({"subject":imported.subject,"text":imported.text,"html":imported.html}),
        None,
    )
}
#[derive(serde::Deserialize)]
struct SiteRequest {
    url: String,
}
async fn import_site(context: Context, body: Bytes) -> Result<Response, Rejection> {
    editor_identity(&context).await?;
    let request: SiteRequest = parse(&body)?;
    let html = context
        .resources()
        .tools()
        .site(&request.url)
        .await
        .map_err(error)?;
    editor_identity(&context).await?;
    response(StatusCode::OK, &json!({"html":html}), None)
}
#[derive(serde::Deserialize)]
struct PreviewRequest {
    html: String,
    #[serde(default)]
    recipient: wire::Recipient,
}
async fn preview_learning(context: Context, body: Bytes) -> Result<Response, Rejection> {
    preview_page(context, body, true).await
}
async fn preview_landing(context: Context, body: Bytes) -> Result<Response, Rejection> {
    preview_page(context, body, false).await
}
async fn preview_page(
    context: Context,
    body: Bytes,
    learning: bool,
) -> Result<Response, Rejection> {
    editor_identity(&context).await?;
    let request: PreviewRequest = parse(&body)?;
    let r = request.recipient;
    let recipient = app::Recipient {
        email: r.email,
        first_name: r.first_name,
        last_name: r.last_name,
        position: r.position,
    };
    let mode = if learning {
        app::PagePreview::Learning
    } else {
        app::PagePreview::Landing(&recipient)
    };
    let (html, warnings) = context
        .resources()
        .page_preview(&request.html, mode)
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!({"html":html,"warnings":warnings}),
        None,
    )
}
async fn validate_page(context: Context, body: Bytes) -> Result<Response, Rejection> {
    editor_identity(&context).await?;
    let request: PreviewRequest = parse(&body)?;
    let warnings = context
        .resources()
        .page_warnings(&request.html)
        .map_err(error)?;
    response(StatusCode::OK, &json!({"warnings":warnings}), None)
}
async fn preview(context: Context, body: Bytes) -> Result<Response, Rejection> {
    editor_identity(&context).await?;
    let request: PreviewRequest = parse(&body)?;
    let r = request.recipient;
    let html = context
        .resources()
        .tools()
        .preview(
            &request.html,
            &app::Recipient {
                email: r.email,
                first_name: r.first_name,
                last_name: r.last_name,
                position: r.position,
            },
        )
        .map_err(error)?;
    let warnings = context
        .resources()
        .page_warnings(&request.html)
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!({"html":html,"warnings":warnings}),
        None,
    )
}
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct TestRequest {
    template: wire::EmailTemplate,
    page: wire::LandingPage,
    smtp: wire::SendingProfile,
    url: String,
    email: String,
    first_name: String,
    last_name: String,
    position: String,
}
fn profile(p: wire::SendingProfile) -> app::Profile {
    app::Profile {
        interface_type: p.interface_type,
        host: p.host,
        username: p.username,
        password: p.password,
        from_address: p.from_address,
        ignore_cert_errors: p.ignore_cert_errors,
        headers: p
            .headers
            .unwrap_or_default()
            .into_iter()
            .map(|h| app::Header {
                key: h.key,
                value: h.value,
            })
            .collect(),
    }
}
async fn send_test(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let identity = editor_identity(&context).await?;
    let request: TestRequest = parse(&body)?;
    let name = request.smtp.name.clone();
    context
        .delivery()
        .send(
            identity,
            ruphish_application::delivery::TestRequest {
                recipient: app::Recipient {
                    email: request.email,
                    first_name: request.first_name,
                    last_name: request.last_name,
                    position: request.position,
                },
                template_name: request.template.name,
                page_name: request.page.name,
                profile_name: name,
                profile: profile(request.smtp),
                url: request.url,
            },
        )
        .await
        .map_err(|failure_value| match failure_value {
            app::Error::NotFound(_) => failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::DeliveryFailed,
                &failure_value.to_string(),
            ),
            other => error(other),
        })?;
    response(
        StatusCode::OK,
        &json!({"success":true,"message":"Email Sent","data":null}),
        None,
    )
}
async fn test_connection(context: Context, body: Bytes) -> Result<Response, Rejection> {
    let identity = editor_identity(&context).await?;
    let request: wire::SendingProfile = parse(&body)?;
    let name = request.name.clone();
    context
        .delivery()
        .connect(identity, &name, profile(request))
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!({"success":true,"message":"SMTP connection verified","data":null}),
        None,
    )
}
fn kind(context: &Context) -> Result<Kind, Rejection> {
    match context.uri.path().split('/').nth(2) {
        Some("groups") => Ok(Kind::Group),
        Some("templates") => Ok(Kind::Template),
        Some("pages") => Ok(Kind::Page),
        Some("learning-pages") => Ok(Kind::LearningPage),
        Some("smtp") => Ok(Kind::Profile),
        _ => Err(failure(
            StatusCode::NOT_FOUND,
            ErrorCode::ResourceNotFound,
            "Resource not found",
        )),
    }
}
pub(crate) fn resource_code(error: &app::Error) -> ErrorCode {
    match error {
        app::Error::Field { source, .. } => resource_code(source),
        app::Error::Policy(error) => crate::auth::policy_code(error),
        app::Error::NotFound(_) => ErrorCode::ResourceNotFound,
        app::Error::Conflict(_) => ErrorCode::ResourceNameTaken,
        app::Error::Changed => ErrorCode::ResourceChanged,
        app::Error::InUse => ErrorCode::ResourceInUse,
        app::Error::Invalid(_) | app::Error::Validation(_) => ErrorCode::ResourceInvalid,
        app::Error::DraftSchedulePast => ErrorCode::DraftSchedulePast,
        app::Error::FeatureDisabled(_) => ErrorCode::FeatureDisabled,
        app::Error::Delivery(_) => ErrorCode::DeliveryFailed,
        app::Error::Storage(_) | app::Error::Internal => ErrorCode::InternalError,
    }
}
fn error_status(error: &app::Error) -> StatusCode {
    match error {
        app::Error::Field { source, .. } => error_status(source),
        app::Error::NotFound(_) => StatusCode::NOT_FOUND,
        app::Error::Conflict(_) | app::Error::InUse => StatusCode::CONFLICT,
        app::Error::Changed => StatusCode::PRECONDITION_FAILED,
        app::Error::FeatureDisabled(_) => StatusCode::CONFLICT,
        app::Error::Internal | app::Error::Storage(_) | app::Error::Delivery(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        app::Error::Policy(ruphish_application::auth::AccountError::Unauthorized) => {
            StatusCode::UNAUTHORIZED
        }
        app::Error::Policy(_) => StatusCode::FORBIDDEN,
        app::Error::Invalid(_) | app::Error::Validation(_) | app::Error::DraftSchedulePast => {
            StatusCode::BAD_REQUEST
        }
    }
}
pub(crate) fn error(error: app::Error) -> Rejection {
    let status = error_status(&error);
    let rejection = failure(status, resource_code(&error), &error.to_string());
    match &error {
        app::Error::Field { field, .. } => {
            let mut report = ruphish_application::validation::Report::default();
            report.reject(
                field,
                ruphish_application::validation::Code::Invalid,
                "Invalid field",
            );
            rejection.with_validation(&report)
        }
        app::Error::Validation(report) => rejection.with_validation(report),
        app::Error::Conflict(_) => {
            let mut report = ruphish_application::validation::Report::default();
            report.reject(
                "name",
                ruphish_application::validation::Code::Conflict,
                "Name already in use",
            );
            rejection.with_validation(&report)
        }
        app::Error::DraftSchedulePast => {
            let mut report = ruphish_application::validation::Report::default();
            report.reject(
                "launch_local.local",
                ruphish_application::validation::Code::SchedulePast,
                "Draft schedule is in the past",
            );
            rejection.with_validation(&report)
        }
        _ => rejection,
    }
}
fn expected(context: &Context) -> Result<Option<String>, Rejection> {
    context
        .headers
        .get(header::IF_MATCH)
        .map(|h| {
            let value = h.to_str().map_err(|_| {
                failure(
                    StatusCode::BAD_REQUEST,
                    ErrorCode::RevisionRequired,
                    "Invalid If-Match header",
                )
            })?;
            value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .filter(|v| !v.contains('"'))
                .map(str::to_owned)
                .ok_or_else(|| {
                    failure(
                        StatusCode::BAD_REQUEST,
                        ErrorCode::RevisionRequired,
                        "Use the ETag returned when loading this resource",
                    )
                })
        })
        .transpose()
}
pub(crate) fn response<T: serde::Serialize>(
    status: StatusCode,
    value: &T,
    revision: Option<&str>,
) -> Result<Response, Rejection> {
    let bytes = wire::json(value).map_err(|_| error(app::Error::Internal))?;
    let mut response =
        (status, [(header::CONTENT_TYPE, "application/json")], bytes).into_response();
    if let Some(revision) = revision {
        response.headers_mut().insert(
            header::ETAG,
            format!("\"{revision}\"")
                .parse()
                .map_err(|_| error(app::Error::Internal))?,
        );
    }
    Ok(response)
}
pub(crate) fn encode(r: &Resource) -> Value {
    let mut value = json!({"id":r.id,"name":r.name,"modified_date":r.modified_date});
    let fields = match &r.content {
        Content::Group(targets) => {
            json!({"targets":targets.iter().map(|t|json!({"email":t.email,"first_name":t.first_name,"last_name":t.last_name,"position":t.position})).collect::<Vec<_>>()})
        }
        Content::Template(t) => {
            json!({"subject":t.subject,"text":t.text,"html":t.html,"envelope_sender":t.envelope_sender,"attachments":t.attachments.iter().map(|a|json!({"name":a.name,"type":a.mime_type,"content":a.content})).collect::<Vec<_>>()})
        }
        Content::LearningPage(p) => json!({"html":p.html,"public_id":p.public_id}),
        Content::Page(p) => {
            json!({"html":p.html,"redirect_url":p.redirect_url})
        }
        Content::Profile(p) => {
            let mut v = json!({"interface_type":p.interface_type,"host":p.host,"from_address":p.from_address,"ignore_cert_errors":p.ignore_cert_errors,"headers":p.headers.iter().map(|h|json!({"key":h.key,"value":h.value})).collect::<Vec<_>>()});
            if !p.username.is_empty() {
                v["username"] = json!(p.username);
            }
            if !p.password.is_empty() {
                v["password"] = json!(p.password);
            }
            v
        }
    };
    if let (Some(v), Some(f)) = (value.as_object_mut(), fields.as_object()) {
        v.extend(f.clone());
    }
    value
}
fn decode(kind: Kind, bytes: &[u8]) -> Result<Resource, Rejection> {
    let invalid = |_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid JSON structure",
        )
    };
    match kind {
        Kind::Group => {
            let v: wire::Group = serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Resource {
                id: v.id,
                name: v.name,
                modified_date: String::new(),
                content: Content::Group(
                    v.targets
                        .unwrap_or_default()
                        .into_iter()
                        .map(|t| app::Recipient {
                            email: t.email,
                            first_name: t.first_name,
                            last_name: t.last_name,
                            position: t.position,
                        })
                        .collect(),
                ),
            })
        }
        Kind::Template => {
            let v: wire::EmailTemplate = serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Resource {
                id: v.id,
                name: v.name,
                modified_date: String::new(),
                content: Content::Template(app::Template {
                    subject: v.subject,
                    text: v.text,
                    html: v.html,
                    envelope_sender: v.envelope_sender,
                    attachments: v
                        .attachments
                        .unwrap_or_default()
                        .into_iter()
                        .map(|a| app::Attachment {
                            name: a.name,
                            mime_type: a.mime_type,
                            content: a.content,
                        })
                        .collect(),
                }),
            })
        }
        Kind::LearningPage => {
            let v: ruphish_contracts::resources::LearningPageInput =
                serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Resource {
                id: v.id,
                name: v.name,
                modified_date: String::new(),
                content: Content::LearningPage(app::LearningPage {
                    html: v.html,
                    public_id: String::new(),
                }),
            })
        }
        Kind::Page => {
            let v: wire::LandingPage = serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Resource {
                id: v.id,
                name: v.name,
                modified_date: String::new(),
                content: Content::Page(app::Page {
                    html: v.html,
                    redirect_url: v.redirect_url,
                }),
            })
        }
        Kind::Profile => {
            let v: wire::SendingProfile = serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Resource {
                id: v.id,
                name: v.name,
                modified_date: String::new(),
                content: Content::Profile(app::Profile {
                    interface_type: v.interface_type,
                    host: v.host,
                    username: v.username,
                    password: v.password,
                    from_address: v.from_address,
                    ignore_cert_errors: v.ignore_cert_errors,
                    headers: v
                        .headers
                        .unwrap_or_default()
                        .into_iter()
                        .map(|h| app::Header {
                            key: h.key,
                            value: h.value,
                        })
                        .collect(),
                }),
            })
        }
    }
}
async fn slash(context: Context) -> Result<Response, Rejection> {
    context.api(None).await?;
    Ok((
        StatusCode::MOVED_PERMANENTLY,
        [(
            header::LOCATION,
            format!(
                "{}/{}",
                context.uri.path(),
                context
                    .uri
                    .query()
                    .map(|q| format!("?{q}"))
                    .unwrap_or_default()
            ),
        )],
    )
        .into_response())
}
async fn list(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let values = context
        .resources()
        .list(user.identity, kind(&context)?)
        .await
        .map_err(error)?;
    response(
        StatusCode::OK,
        &json!(values.iter().map(encode).collect::<Vec<_>>()),
        None,
    )
}
async fn read(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let value = context
        .resources()
        .get(user.identity, kind(&context)?, id)
        .await
        .map_err(error)?;
    let mut wire = encode(&value);
    if let Content::Page(page) = &value.content {
        wire["warnings"] = json!(
            context
                .resources()
                .page_warnings(&page.html)
                .map_err(error)?
        );
    }
    if let Content::LearningPage(page) = &value.content {
        let (html, warnings) = context
            .resources()
            .page_preview(&page.html, app::PagePreview::Learning)
            .map_err(error)?;
        wire["preview_html"] = json!(html);
        wire["warnings"] = json!(warnings);
    }
    response(StatusCode::OK, &wire, Some(&value.modified_date))
}
async fn create(context: Context, body: Bytes) -> Result<Response, Rejection> {
    save(context, None, body).await
}
async fn update(context: Context, Path(id): Path<i64>, body: Bytes) -> Result<Response, Rejection> {
    save(context, Some(id), body).await
}
async fn save(context: Context, id: Option<i64>, body: Bytes) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    user.require("modify_objects")
        .map_err(|e| error(e.into()))?;
    let kind = kind(&context)?;
    if let Some(id) = id {
        context
            .resources()
            .get(user.identity, kind, id)
            .await
            .map_err(error)?;
    }
    let value = decode(kind, &body)?;
    let nullable = match kind {
        Kind::Template => Some("attachments"),
        Kind::Profile => Some("headers"),
        _ => None,
    };
    let preserve_nil = nullable.is_some_and(|field| {
        serde_json::from_slice::<Value>(&body).is_ok_and(|v| v[field].is_null())
    });
    if id.is_some_and(|id| value.id != id) {
        return Err(failure(
            if kind == Kind::Group {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::BAD_REQUEST
            },
            ErrorCode::ResourceIdMismatch,
            match kind {
                Kind::Group => "Error: /:id and group_id mismatch",
                Kind::Template => "Error: /:id and template_id mismatch",
                Kind::Page | Kind::LearningPage => "/:id and /:page_id mismatch",
                Kind::Profile => "/:id and /:smtp_id mismatch",
            },
        ));
    }
    let warnings = if let Content::LearningPage(page) = &value.content {
        Some(
            context
                .resources()
                .page_preview(&page.html, app::PagePreview::Learning)
                .map_err(|e| error(e.at("html")))?
                .1,
        )
    } else if let Content::Page(page) = &value.content {
        Some(
            context
                .resources()
                .page_warnings(&page.html)
                .map_err(|e| error(e.at("html")))?,
        )
    } else {
        None
    };
    let saved = context
        .resources()
        .save(user.identity, value, id, expected(&context)?)
        .await
        .map_err(error)?;
    let mut wire = encode(&saved);
    if let Some(warnings) = warnings {
        wire["warnings"] = json!(warnings);
    }
    if preserve_nil && let Some(field) = nullable {
        wire[field] = Value::Null;
    }
    response(
        if id.is_some() {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        &wire,
        Some(&saved.modified_date),
    )
}
async fn delete(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let kind = kind(&context)?;
    context
        .resources()
        .delete(user.identity, kind, id, expected(&context)?)
        .await
        .map_err(error)?;
    let message = match kind {
        Kind::Group => "Group deleted successfully!",
        Kind::Template => "Template deleted successfully!",
        Kind::Page => "Page Deleted Successfully",
        Kind::LearningPage => "Learning page deleted successfully",
        Kind::Profile => "SMTP Deleted Successfully",
    };
    response(
        StatusCode::OK,
        &json!({"success":true,"message":message,"data":null}),
        None,
    )
}
fn summary(s: &app::Summary, kind: Kind) -> Value {
    if kind == Kind::Group {
        json!({"id":s.id,"name":s.name,"modified_date":s.modified_date,"num_targets":s.num_targets})
    } else {
        json!({"id":s.id,"name":s.name,"modified_date":s.modified_date,"active_campaigns":s.active_campaigns})
    }
}
async fn summaries(context: Context) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let kind = kind(&context)?;
    let values = context
        .resources()
        .summaries(user.identity, kind)
        .await
        .map_err(error)?;
    let rows: Vec<_> = values.iter().map(|s| summary(s, kind)).collect();
    let value = if kind == Kind::Group {
        json!({"total":rows.len(),"groups":rows})
    } else {
        json!({"total":rows.len(),"items":rows})
    };
    response(StatusCode::OK, &value, None)
}
async fn group_summary(context: Context, Path(id): Path<i64>) -> Result<Response, Rejection> {
    let user = context.api(None).await?;
    let values = context
        .resources()
        .summaries(user.identity, Kind::Group)
        .await
        .map_err(error)?;
    let value = values
        .iter()
        .find(|v| v.id == id)
        .ok_or_else(|| error(app::Error::NotFound("Group")))?;
    response(StatusCode::OK, &summary(value, Kind::Group), None)
}

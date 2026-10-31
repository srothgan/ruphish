//! Public transport accepts only closed event messages and trusted host metadata.
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
};
use ruphish_application::{
    delivery::Previews,
    domains::Repository,
    interactions::{Action, Output, Public},
};
use std::{net::SocketAddr, sync::Arc, time::Duration};

/// Public listener dependencies and installation-owned proxy trust.
pub struct Services {
    /// Bounded, immutable public file snapshot; never admin assets.
    pub assets: super::public_assets::Assets,
    /// Domain resource and verification repository.
    pub domains: Arc<dyn Repository>,
    /// Typed public interaction application.
    pub interactions: Arc<dyn Public>,
    /// Non-tracking preview store.
    pub previews: Arc<dyn Previews>,
    /// Latest HTTPS observation.
    pub tls: bool,
    /// Proxy networks authorized to supply overwritten forwarding headers.
    pub trusted_proxies: Vec<ipnet::IpNet>,
    /// Configured transparency contact address.
    pub contact: String,
}
fn transport_origin(scheme: &str, authority: &str) -> Option<String> {
    authority.parse::<axum::http::uri::Authority>().ok()?;
    if authority.contains(',') {
        return None;
    }
    ruphish_application::domains::origin(&format!("{scheme}://{authority}"), false)
        .ok()
        .map(|o| o.value)
}
fn request_origin(request: &Request<Body>, services: &Services) -> Option<String> {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()?
        .0
        .ip();
    if services
        .trusted_proxies
        .iter()
        .any(|network| network.contains(&peer))
    {
        if request
            .headers()
            .get_all("x-forwarded-proto")
            .iter()
            .count()
            != 1
        {
            return None;
        }
        let scheme = request.headers().get("x-forwarded-proto")?.to_str().ok()?;
        if !matches!(scheme, "http" | "https") {
            return None;
        }
        let host = request.headers().get("x-forwarded-host")?.to_str().ok()?;
        if request.headers().get_all("x-forwarded-host").iter().count() != 1 {
            return None;
        }
        return transport_origin(scheme, host);
    }
    let scheme = if services.tls { "https" } else { "http" };
    if request
        .uri()
        .scheme_str()
        .is_some_and(|value| value != scheme)
        || request.headers().get_all(header::HOST).iter().count() > 1
    {
        return None;
    }
    let uri = request.uri().authority().map(|a| a.as_str());
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if uri
        .zip(host)
        .is_some_and(|(u, h)| !u.eq_ignore_ascii_case(h))
    {
        return None;
    }
    transport_origin(scheme, uri.or(host)?)
}
fn empty(status: StatusCode) -> Response {
    if status == StatusCode::NOT_FOUND {
        let mut response = super::response::not_found();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );
        response.headers_mut().insert(
            header::REFERRER_POLICY,
            header::HeaderValue::from_static("no-referrer"),
        );
        return response;
    }
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
    )
        .into_response()
}

/// No admin routes, session cookies, raw access logging or arbitrary static file root.
pub fn router(services: Services) -> Router {
    let state = Arc::new(services);
    let capacity = Arc::new(tokio::sync::Semaphore::new(32));
    Router::new()
        .fallback(move |request: Request<Body>| {
            let state = state.clone();
            let capacity = capacity.clone();
            async move {
                let Ok(_permit) = capacity.try_acquire_owned() else {
                    return empty(StatusCode::TOO_MANY_REQUESTS);
                };
                let mut response =
                    match tokio::time::timeout(Duration::from_secs(10), handle(request, &state))
                        .await
                    {
                        Ok(response) => response,
                        Err(_) => empty(StatusCode::REQUEST_TIMEOUT),
                    };
                // Sandboxed landing pages have an opaque origin. Public responses expose
                // no administrative credentials and never permit credentialed CORS.
                response.headers_mut().insert(
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    header::HeaderValue::from_static("*"),
                );
                response
            }
        })
        .layer(middleware::from_fn(super::server::access_log))
}
async fn handle(request: Request<Body>, state: &Services) -> Response {
    let now = ruphish_application::datetime::now().as_second();
    let method = request.method().clone();
    if !matches!(
        method,
        Method::GET | Method::HEAD | Method::POST | Method::OPTIONS
    ) {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let path = request.uri().path().to_owned();
    if path == "/robots.txt" && matches!(method, Method::GET | Method::HEAD) {
        return (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            if method == Method::HEAD {
                ""
            } else {
                "User-agent: *\nDisallow: /\n"
            },
        )
            .into_response();
    }
    let origin = request_origin(&request, state);
    if let Some(public_id) = path.strip_prefix("/learn/") {
        if !matches!(method, Method::GET | Method::HEAD) {
            return empty(StatusCode::METHOD_NOT_ALLOWED);
        }
        let Some(origin) = origin else {
            return empty(StatusCode::NOT_FOUND);
        };
        return match state.interactions.learning_page(&origin, public_id).await {
            Ok(Output::Html { html, csp }) => (
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                    (header::CONTENT_SECURITY_POLICY, csp.as_str()),
                    (header::CACHE_CONTROL, "no-store"),
                    (header::REFERRER_POLICY, "no-referrer"),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                ],
                if method == Method::HEAD {
                    String::new()
                } else {
                    html
                },
            )
                .into_response(),
            Err(
                ruphish_application::resources::Error::Internal
                | ruphish_application::resources::Error::Storage(_),
            ) => empty(StatusCode::SERVICE_UNAVAILABLE),
            _ => empty(StatusCode::NOT_FOUND),
        };
    }
    if path.starts_with("/static/") {
        let Some(origin) = origin else {
            return empty(StatusCode::NOT_FOUND);
        };
        if !state.domains.serves(&origin).await.unwrap_or(false) {
            return empty(StatusCode::NOT_FOUND);
        }
        return state.assets.response(&path, &method);
    }
    if let Some(challenge) = path.strip_prefix("/.well-known/ruphish/") {
        if method != Method::GET {
            return empty(StatusCode::METHOD_NOT_ALLOWED);
        }
        let Some(origin) = origin else {
            return empty(StatusCode::NOT_FOUND);
        };
        return match state.domains.routing_proof(&origin, challenge, now).await {
            Ok(Some(proof)) => (
                [
                    (header::CONTENT_TYPE, "text/plain"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                proof,
            )
                .into_response(),
            _ => empty(StatusCode::NOT_FOUND),
        };
    }
    let query = request.uri().query().unwrap_or("").to_owned();
    if query.len() > 4096 {
        return empty(StatusCode::BAD_REQUEST);
    }
    let Ok(pairs) = serde_urlencoded::from_str::<Vec<(String, String)>>(&query) else {
        return empty(StatusCode::BAD_REQUEST);
    };
    let mut ids = pairs
        .iter()
        .filter(|(name, _)| name == "rid")
        .map(|(_, value)| value.as_str());
    let Some(rid) = ids.next() else {
        return empty(StatusCode::NOT_FOUND);
    };
    if ids.next().is_some() {
        return empty(StatusCode::BAD_REQUEST);
    }
    let inspect = rid.ends_with(['+', ' ']);
    let rid = if inspect { &rid[..rid.len() - 1] } else { rid };
    if rid.starts_with("preview-") && method == Method::GET {
        if !state.previews.contains(rid).await.unwrap_or(false) {
            return empty(StatusCode::NOT_FOUND);
        }
        if path.ends_with("/track") {
            return pixel();
        }
        if path.ends_with("/report") {
            return empty(StatusCode::NO_CONTENT);
        }
        return match state.previews.render(rid).await {
            Ok(html) => (
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                    (header::CACHE_CONTROL, "no-store"),
                    (
                        header::CONTENT_SECURITY_POLICY,
                        ruphish_contracts::landing::response_policy().as_str(),
                    ),
                    (header::REFERRER_POLICY, "no-referrer"),
                ],
                html,
            )
                .into_response(),
            Err(_) => empty(StatusCode::NOT_FOUND),
        };
    }
    let Some(origin) = origin else {
        return empty(StatusCode::NOT_FOUND);
    };
    if method == Method::OPTIONS {
        return (
            StatusCode::NO_CONTENT,
            [
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, POST"),
                (header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type"),
            ],
        )
            .into_response();
    }
    let action = if method == Method::POST {
        if inspect || path.ends_with("/track") || path.ends_with("/report") {
            return empty(StatusCode::METHOD_NOT_ALLOWED);
        }
        if request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(';').next().unwrap_or("").trim())
            != Some("application/json")
        {
            return empty(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }
        let Ok(body) = to_bytes(request.into_body(), 512).await else {
            return empty(StatusCode::PAYLOAD_TOO_LARGE);
        };
        let Ok(input) = ruphish_contracts::domains::decode_request::<
            ruphish_contracts::domains::Submission,
        >(&body) else {
            return empty(StatusCode::BAD_REQUEST);
        };
        Action::Submit {
            event_id: input.event_id,
            password_present: input.password_present,
        }
    } else if method == Method::HEAD {
        Action::Head
    } else if inspect {
        Action::Inspect
    } else if path.ends_with("/track") {
        Action::Open
    } else if path.ends_with("/report") {
        Action::Report
    } else {
        Action::View
    };
    let mut response = match state.interactions.interact(&origin,rid,action,now).await {
        Ok(Output::Html{html,csp})=>([(header::CONTENT_TYPE,"text/html; charset=utf-8"),(header::CACHE_CONTROL,"no-store"),(header::CONTENT_SECURITY_POLICY,csp.as_str()),(header::REFERRER_POLICY,"no-referrer"),(header::X_CONTENT_TYPE_OPTIONS,"nosniff"),(header::HeaderName::from_static("x-server"),"gophish")],html).into_response(),
        Ok(Output::Pixel)=>pixel(),
        Ok(Output::Empty) if method == Method::HEAD => (StatusCode::OK,[(header::CACHE_CONTROL,"no-store"),(header::CONTENT_TYPE,if path.ends_with("/track") {"image/png"} else {"text/html; charset=utf-8"})]).into_response(),
        Ok(Output::Empty)=>(StatusCode::NO_CONTENT,[(header::ACCESS_CONTROL_ALLOW_ORIGIN,"*"),(header::CACHE_CONTROL,"no-store")]).into_response(),
        Ok(Output::Transparency{send_date})=>([(header::CACHE_CONTROL,"no-store")],axum::Json(serde_json::json!({"server":"gophish","contact_address":state.contact,"send_date":send_date}))).into_response(),
        Ok(Output::Submitted{redirect})=>([(header::CACHE_CONTROL,"no-store")],axum::Json(serde_json::json!({"success":true,"redirect":redirect}))).into_response(),
        Err(ruphish_application::resources::Error::Invalid(code)) if code == "interaction_limit_reached" => empty(StatusCode::TOO_MANY_REQUESTS),
        Err(ruphish_application::resources::Error::Invalid(_))=>empty(StatusCode::BAD_REQUEST),
        Err(ruphish_application::resources::Error::Internal | ruphish_application::resources::Error::Storage(_))=>empty(StatusCode::SERVICE_UNAVAILABLE),
        Err(_)=>empty(StatusCode::NOT_FOUND),
    };
    if path.ends_with("/report") {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            header::HeaderValue::from_static("*"),
        );
    }
    response
}
fn pixel() -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_bytes!("../assets/preview-pixel.png").as_slice(),
    )
        .into_response()
}

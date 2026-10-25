//! Browser sessions, CSRF and legacy account API boundaries.
use super::rate::LoginLimiter;
use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, FromRef, FromRequestParts, Path, Request},
    http::{HeaderMap, Method, StatusCode, Uri, header, request::Parts},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use axum_csrf::{CsrfConfig, CsrfToken};
use governor::clock::DefaultClock;
use ruphish_application::auth::{Account, AccountError, Edit, Error, Identity, Service};
use ruphish_application::datetime;
use ruphish_contracts::errors::ErrorCode;
use ruphish_contracts::{
    auth::{BrowserSession, BrowserUser, Role, User, UserEdit},
    legacy::{Envelope, Timestamp},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use time::{Duration, OffsetDateTime};
use tower_sessions::{
    Session, SessionManagerLayer, SessionStore,
    cookie::{Key, SameSite},
    session::Expiry,
};

#[derive(Clone)]
pub(crate) struct AuthState {
    service: Service,
    resources: ruphish_application::resources::Service,
    delivery: ruphish_application::delivery::Service,
    campaigns: Arc<dyn ruphish_application::campaign::Repository>,
    campaign_list_slots: Arc<tokio::sync::Semaphore>,
    domains: Arc<dyn ruphish_application::domains::Repository>,
    reporting: Arc<dyn ruphish_application::reporting::Repository>,
    overview: Arc<dyn ruphish_application::overview::Repository>,
    integrations: Arc<dyn ruphish_application::integrations::Repository>,
    settings: Arc<dyn ruphish_application::settings::Repository>,
    mailer_enabled: bool,
    csrf: CsrfConfig,
    index: Arc<String>,
    secure: bool,
    trusted_origins: Arc<Vec<String>>,
    limiter: Arc<LoginLimiter<DefaultClock>>,
}
impl FromRef<AuthState> for CsrfConfig {
    fn from_ref(state: &AuthState) -> Self {
        state.csrf.clone()
    }
}

/// Compose validated assets and account routes with persistent, signed browser sessions.
pub fn router<S: SessionStore + Clone>(
    assets: super::server::AdminAssets,
    services: Services,
    store: S,
    key: &[u8; 64],
    secure: bool,
    trusted_origins: Vec<String>,
    csrf_key: String,
) -> Router {
    let state = AuthState {
        service: services.accounts,
        resources: services.resources,
        delivery: services.delivery,
        campaigns: services.campaigns,
        campaign_list_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        domains: services.domains,
        reporting: services.reporting,
        overview: services.overview,
        integrations: services.integrations,
        settings: services.settings,
        mailer_enabled: services.mailer_enabled,
        index: Arc::new(assets.index),
        secure,
        trusted_origins: Arc::new(trusted_origins),
        csrf: CsrfConfig::default()
            .with_key(Some(axum_csrf::Key::from(key)))
            .with_salt(if csrf_key.is_empty() {
                "ruphish-browser-csrf-v1".to_owned()
            } else {
                csrf_key
            })
            .with_cookie_name("ruphish.csrf")
            .with_prefix_with_host(secure)
            .with_secure(secure)
            .with_http_only(true)
            .with_cookie_same_site(axum_csrf::SameSite::Strict)
            .with_lifetime(Duration::hours(12)),
        limiter: Arc::new(LoginLimiter::new(DefaultClock::default())),
    };
    let session = SessionManagerLayer::new(store)
        .with_name(if secure {
            "__Host-ruphish.sid"
        } else {
            "ruphish.sid"
        })
        .with_secure(secure)
        .with_http_only(true)
        .with_same_site(SameSite::Strict)
        .with_expiry(Expiry::OnInactivity(Duration::hours(12)))
        .with_signed(Key::from(key));
    let routes = Router::new()
        .route("/", get(page))
        .route("/login", get(page).post(login))
        .route("/logout", get(page).post(logout))
        .route("/reset_password", get(page).post(password))
        .route("/settings", get(page))
        .route("/settings/starter-templates", get(page))
        .route("/user", get(page))
        .route("/user/settings", get(page))
        .route("/user/password", get(page))
        .nest(
            "/internal",
            Router::new()
                .route("/preferences", get(preferences).put(save_preferences))
                .route("/password", post(password))
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    internal_session,
                )),
        )
        .route("/users", get(page))
        .route("/users/new", get(page))
        .route("/users/{id}/edit", get(page))
        .route("/campaigns", get(page))
        .route("/analysis", get(page))
        .route("/webhooks", get(page))
        .route("/webhooks/{id}", get(page))
        .route("/webhooks/{id}/edit", get(page))
        .route("/reporting-mailboxes", get(page))
        .route("/reporting-mailboxes/{id}", get(page))
        .route("/reporting-mailboxes/{id}/edit", get(page))
        .route("/analysis/{id}", get(page))
        .route("/analysis/{id}/recipients/{recipient}", get(page))
        .route("/domains", get(page))
        .route("/domains/{id}", get(page))
        .route("/domains/{id}/edit", get(page))
        .route("/campaigns/{id}", get(page))
        .route("/campaigns/drafts/{id}", get(page))
        .route("/campaigns/drafts/{id}/edit", get(page))
        .route("/groups", get(page))
        .route("/groups/{id}", get(page))
        .route("/groups/{id}/edit", get(page))
        .route("/templates", get(page))
        .route("/templates/{id}", get(page))
        .route("/templates/{id}/edit", get(page))
        .route("/learning-pages", get(page))
        .route("/learning-pages/{id}", get(page))
        .route("/learning-pages/{id}/edit", get(page))
        .route("/pages", get(page))
        .route("/pages/{id}", get(page))
        .route("/pages/{id}/edit", get(page))
        .route("/smtp", get(page))
        .route("/smtp/{id}", get(page))
        .route("/smtp/{id}/edit", get(page))
        .route("/auth/session", get(bootstrap))
        .route("/impersonate", post(impersonate))
        .route("/impersonate/exit", post(exit_impersonation))
        .route("/api/users", get(users_slash).post(users_slash))
        .route("/api/users/", get(users).post(create))
        .route("/api/users/{id}", get(user).put(update).delete(delete))
        .route("/api/reset", post(reset_key))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .merge(super::resources::routes())
        .merge(super::campaigns::routes())
        .merge(super::domains::routes())
        .merge(super::reporting::routes())
        .merge(super::overview::routes())
        .merge(super::integrations::routes())
        .merge(super::starter_templates::routes())
        .merge(super::settings::routes())
        .layer(middleware::from_fn(security_headers))
        .fallback(missing_page)
        .with_state(state)
        .layer(session);
    let admission = std::sync::Arc::new(tokio::sync::Semaphore::new(64));
    super::server::finish(
        assets
            .router
            .merge(routes)
            .layer(middleware::from_fn_with_state(
                admission,
                super::server::admin_admission,
            )),
    )
}

// Only these small identity values are stored in the server-side session.
#[derive(Clone, Serialize, Deserialize)]
struct Principal {
    id: i64,
    epoch: i64,
}
impl From<Identity> for Principal {
    fn from(value: Identity) -> Self {
        Self {
            id: value.id,
            epoch: value.epoch,
        }
    }
}
impl From<Principal> for Identity {
    fn from(value: Principal) -> Self {
        Self {
            id: value.id,
            epoch: value.epoch,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Login {
    user: Principal,
    actor: Option<Principal>,
    expires: i64,
}

pub(crate) struct Context {
    state: AuthState,
    session: Session,
    csrf: CsrfToken,
    pub(crate) headers: HeaderMap,
    pub(crate) uri: Uri,
    method: Method,
    peer: IpAddr,
}
impl FromRequestParts<AuthState> for Context {
    type Rejection = Rejection;
    async fn from_request_parts(parts: &mut Parts, state: &AuthState) -> Result<Self, Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| auth_error(Error::Internal))?;
        let csrf = CsrfToken::from_request_parts(parts, state)
            .await
            .map_err(|_| auth_error(Error::Internal))?;
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .ok_or_else(|| {
                failure(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorCode::InternalError,
                    "Connection identity unavailable",
                )
            })?
            .0
            .ip();
        Ok(Self {
            state: state.clone(),
            session,
            csrf,
            headers: parts.headers.clone(),
            uri: parts.uri.clone(),
            method: parts.method.clone(),
            peer,
        })
    }
}
impl Context {
    pub(crate) fn campaign_list_permit(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, Rejection> {
        self.state
            .campaign_list_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                failure(
                    StatusCode::SERVICE_UNAVAILABLE,
                    ErrorCode::ServiceBusy,
                    "Campaign listing is busy; retry shortly",
                )
            })
    }
    pub(crate) fn integrations(&self) -> &dyn ruphish_application::integrations::Repository {
        self.state.integrations.as_ref()
    }
    pub(crate) fn reporting(&self) -> &dyn ruphish_application::reporting::Repository {
        self.state.reporting.as_ref()
    }
    pub(crate) fn overview(&self) -> &dyn ruphish_application::overview::Repository {
        self.state.overview.as_ref()
    }
    pub(crate) fn settings(&self) -> &dyn ruphish_application::settings::Repository {
        self.state.settings.as_ref()
    }
    pub(crate) fn domains(&self) -> &dyn ruphish_application::domains::Repository {
        self.state.domains.as_ref()
    }
    pub(crate) fn campaigns(&self) -> &dyn ruphish_application::campaign::Repository {
        self.state.campaigns.as_ref()
    }
    pub(crate) fn mailer_enabled(&self) -> bool {
        self.state.mailer_enabled
    }
    pub(crate) fn explicit_api_key(&self) -> bool {
        self.headers.contains_key(header::AUTHORIZATION)
            || self.uri.query().is_some_and(|q| {
                serde_urlencoded::from_str::<HashMap<String, String>>(q)
                    .is_ok_and(|q| q.contains_key("api_key"))
            })
    }
    pub(crate) fn delivery(&self) -> &ruphish_application::delivery::Service {
        &self.state.delivery
    }
    pub(crate) fn resources(&self) -> &ruphish_application::resources::Service {
        &self.state.resources
    }
    async fn identity(&self) -> Result<(Account, Option<Account>), Rejection> {
        let login: Login = self
            .session
            .get("login")
            .await
            .map_err(session_error)?
            .ok_or_else(unauthorized)?;
        if login.expires <= ruphish_application::datetime::now().as_second() {
            self.session.flush().await.map_err(session_error)?;
            return Err(unauthorized());
        }
        let user = self
            .state
            .service
            .session(login.user.into())
            .await
            .map_err(auth_error)?;
        let actor = if let Some(actor) = login.actor {
            let actor = self
                .state
                .service
                .session(actor.into())
                .await
                .map_err(auth_error)?;
            actor
                .require("modify_system")
                .map_err(|e| auth_error(e.into()))?;
            Some(actor)
        } else {
            None
        };
        Ok((user, actor))
    }
    fn csrf(&self, form: Option<&str>) -> Result<(), Rejection> {
        let token = self
            .headers
            .get("x-csrf-token")
            .and_then(|h| h.to_str().ok())
            .or(form)
            .unwrap_or("");
        self.csrf.verify(token).map_err(|_| {
            failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthCsrfInvalid,
                "Invalid CSRF token",
            )
        })?;
        if let Some(origin) = self.headers.get(header::ORIGIN) {
            self.origin(origin.to_str().unwrap_or(""))?;
        } else if let Some(referer) = self.headers.get(header::REFERER) {
            let origin = url::Url::parse(referer.to_str().unwrap_or(""))
                .map_err(|_| {
                    failure(
                        StatusCode::FORBIDDEN,
                        ErrorCode::AuthOriginInvalid,
                        "Invalid request origin",
                    )
                })?
                .origin()
                .ascii_serialization();
            self.origin(&origin)?;
        }
        if self
            .headers
            .get("sec-fetch-site")
            .is_some_and(|v| v == "cross-site")
        {
            return Err(failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthOriginInvalid,
                "Cross-site request denied",
            ));
        }
        Ok(())
    }
    fn origin(&self, origin: &str) -> Result<(), Rejection> {
        let header_host = self.headers.get(header::HOST).and_then(|v| v.to_str().ok());
        let scheme = if self.state.secure { "https" } else { "http" };
        // Hyper represents HTTP/2 :authority in the URI, not the Host header.
        // Use the actual request authority; never infer it from the bind address
        // or client-supplied forwarding headers.
        let authority = self.uri.authority().map(|value| value.as_str());
        let request_origin = url::Url::parse(&format!(
            "{scheme}://{}",
            authority.or(header_host).unwrap_or("")
        ))
        .map_err(|_| {
            failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthOriginInvalid,
                "Invalid request origin",
            )
        })?
        .origin();
        if let (Some(_), Some(host)) = (authority, header_host)
            && !url::Url::parse(&format!("{scheme}://{host}"))
                .is_ok_and(|host| host.origin() == request_origin)
        {
            return Err(failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthOriginInvalid,
                "Conflicting request authority",
            ));
        }
        let parsed = url::Url::parse(origin).map_err(|_| {
            failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthOriginInvalid,
                "Invalid request origin",
            )
        })?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthOriginInvalid,
                "Invalid request origin",
            ));
        }
        let matches = |allowed: &str| {
            url::Url::parse(allowed).is_ok_and(|allowed| allowed.origin() == parsed.origin())
        };
        // Legacy configuration lists host[:port]. Bind these to the listener's
        // scheme; explicit origins can also name a different configured scheme.
        if request_origin != parsed.origin()
            && !self.state.trusted_origins.iter().any(|allowed| {
                if allowed.contains("://") {
                    matches(allowed)
                } else {
                    matches(&format!("{scheme}://{allowed}"))
                }
            })
        {
            return Err(failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthOriginInvalid,
                "Invalid request origin",
            ));
        }
        Ok(())
    }
    pub(crate) async fn api(&self, form_key: Option<&str>) -> Result<Account, Rejection> {
        let query: HashMap<String, String> =
            serde_urlencoded::from_str(self.uri.query().unwrap_or("")).unwrap_or_default();
        let key = form_key.or_else(|| query.get("api_key").map(String::as_str));
        // Any explicitly supplied credential takes precedence, including empty/invalid values.
        let authorization = self.headers.get(header::AUTHORIZATION);
        if key.is_some() || authorization.is_some() {
            let key = key
                .or_else(|| {
                    authorization
                        .and_then(|h| h.to_str().ok())
                        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v))
                })
                .unwrap_or("");
            return self
                .state
                .service
                .api_key(key)
                .await
                .map_err(|error| match error {
                    Error::Internal | Error::Storage(_) | Error::Busy => auth_error(error),
                    _ => failure(
                        StatusCode::UNAUTHORIZED,
                        ErrorCode::AuthApiKeyInvalid,
                        if key.is_empty() {
                            "API Key not set"
                        } else {
                            "Invalid API Key"
                        },
                    ),
                });
        }
        let (user, actor) = self.identity().await.map_err(|error| {
            if matches!(
                error.status(),
                StatusCode::UNAUTHORIZED | StatusCode::NOT_FOUND
            ) {
                failure(
                    StatusCode::UNAUTHORIZED,
                    ErrorCode::AuthRequired,
                    if self
                        .headers
                        .get("x-ruphish-client")
                        .is_some_and(|v| v == "browser")
                    {
                        "Your session ended. Sign in again to continue."
                    } else {
                        "API Key not set"
                    },
                )
            } else {
                error
            }
        })?;
        if actor.is_some()
            && (self.uri.path().starts_with("/api/users")
                || self.uri.path().starts_with("/api/starter-templates")
                || self.uri.path().starts_with("/api/settings")
                || self.uri.path().starts_with("/api/campaigns/complete-all")
                || self.uri.path() == "/api/reset")
        {
            return Err(failure(
                StatusCode::FORBIDDEN,
                ErrorCode::AuthImpersonationActive,
                "Exit impersonation before managing accounts",
            ));
        }
        if !matches!(self.method, Method::GET | Method::HEAD | Method::OPTIONS) {
            self.csrf(None)?;
        }
        if user.password_change_required {
            return Err(auth_error(AccountError::ResetRequired.into()));
        }
        Ok(user)
    }
    async fn establish(
        &self,
        user: Identity,
        actor: Option<Identity>,
        expires: i64,
    ) -> Result<(), Rejection> {
        self.session.cycle_id().await.map_err(session_error)?;
        self.session
            .insert(
                "login",
                Login {
                    user: user.into(),
                    actor: actor.map(Into::into),
                    expires,
                },
            )
            .await
            .map_err(session_error)?;
        self.session.set_expiry(Some(Expiry::AtDateTime(
            OffsetDateTime::from_unix_timestamp(expires)
                .map_err(|_| auth_error(Error::Internal))?,
        )));
        self.session.save().await.map_err(session_error)
    }
    fn json(&self) -> bool {
        self.headers
            .get(header::ACCEPT)
            .is_some_and(|v| v.to_str().unwrap_or("").contains("application/json"))
    }
    fn completed(&self, next: &str) -> Response {
        if self.json() {
            Json(Envelope {
                success: true,
                message: String::new(),
                data: Some(next),
            })
            .into_response()
        } else {
            redirect(StatusCode::FOUND, next)
        }
    }
}

/// Application services composed by the executable, sharing the same identity boundary.
pub struct Services {
    /// Installation policy and bulk closure.
    pub settings: Arc<dyn ruphish_application::settings::Repository>,
    /// Webhooks and owner-scoped reporting mailboxes.
    pub integrations: Arc<dyn ruphish_application::integrations::Repository>,
    /// Consistent campaign analysis and exports.
    pub reporting: Arc<dyn ruphish_application::reporting::Repository>,
    /// Owner-scoped dashboard aggregation.
    pub overview: Arc<dyn ruphish_application::overview::Repository>,
    /// Domain configuration and verification queue.
    pub domains: Arc<dyn ruphish_application::domains::Repository>,
    /// Account and session authorization.
    pub accounts: Service,
    /// Resource persistence and format operations.
    pub resources: ruphish_application::resources::Service,
    /// Synchronous test-email and preview use cases.
    pub delivery: ruphish_application::delivery::Service,
    /// Owner-scoped campaign operations and durable delivery metadata.
    pub campaigns: Arc<dyn ruphish_application::campaign::Repository>,
    /// Whether this process runs campaign delivery workers.
    pub mailer_enabled: bool,
}

/// Direct document navigation reaches the same UI fallback as client navigation.
/// Protocol namespaces and file requests must never be replaced with the shell.
async fn missing_page(context: Context) -> Response {
    let path = context.uri.path();
    let namespace = path.split('/').nth(1).unwrap_or_default();
    let document = context.headers.get_all(header::ACCEPT).iter().any(|value| {
        value.to_str().ok().is_some_and(|value| {
            value.split(',').any(|range| {
                let mut parts = range.split(';');
                let html = parts
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/html"));
                let quality = parts
                    .filter_map(|parameter| parameter.trim().split_once('='))
                    .find(|(name, _)| name.trim().eq_ignore_ascii_case("q"))
                    .map_or(Some(1.0), |(_, value)| value.trim().parse::<f32>().ok());
                html && quality.is_some_and(|quality| quality > 0.0 && quality <= 1.0)
            })
        })
    });
    let head = context.method == Method::HEAD;
    let mut response = if (context.method == Method::GET || head)
        && document
        && !matches!(namespace, "api" | "auth" | "internal")
        && !path.split('/').any(|segment| segment.contains('.'))
    {
        // Reuse page authentication, password-change and permission rules. Only
        // a successful shell changes to 404; redirects and rejections keep theirs.
        let mut response = page(context).await;
        if response.status() == StatusCode::OK {
            *response.status_mut() = StatusCode::NOT_FOUND;
        }
        response
    } else {
        super::response::not_found()
    };
    response
        .headers_mut()
        .insert(header::VARY, header::HeaderValue::from_static("Accept"));
    if head {
        *response.body_mut() = Body::empty();
    }
    page_security_headers(response)
}

async fn page(context: Context) -> Response {
    let path = context.uri.path();
    if path != "/login" {
        match context.identity().await {
            Ok((user, _)) => {
                if user.password_change_required && !matches!(path, "/reset_password" | "/logout") {
                    let query = serde_urlencoded::to_string([("next", path)]).unwrap_or_default();
                    return redirect(
                        StatusCode::TEMPORARY_REDIRECT,
                        &format!("/reset_password?{query}"),
                    );
                }
                if path == "/reset_password" && !user.password_change_required {
                    return redirect(StatusCode::TEMPORARY_REDIRECT, "/user/password");
                }
                if (path == "/settings"
                    || path.starts_with("/settings/")
                    || path == "/users"
                    || path.starts_with("/users/"))
                    && !user.permits("modify_system")
                {
                    // The shell contains no protected data. Keep the HTTP rejection,
                    // but let the authenticated UI explain it; APIs still authorize
                    // each read and mutation independently.
                    return (
                        StatusCode::FORBIDDEN,
                        context.csrf,
                        Html((*context.state.index).clone()),
                    )
                        .into_response();
                }
            }
            Err(error)
                if error.status() == StatusCode::UNAUTHORIZED
                    || error.status() == StatusCode::NOT_FOUND =>
            {
                return redirect(
                    StatusCode::TEMPORARY_REDIRECT,
                    &format!("/login?next={path}"),
                );
            }
            Err(error) => return error.into_response(),
        }
    }
    (context.csrf, Html((*context.state.index).clone())).into_response()
}

async fn bootstrap(context: Context) -> Result<Response, Rejection> {
    let csrf_token = context
        .csrf
        .authenticity_token()
        .map_err(|_| auth_error(Error::Internal))?;
    let mut features = None;
    let mut preferences = ruphish_contracts::auth::Preferences::default();
    let (user, actor) = match context.identity().await {
        Ok((user, actor)) => {
            features = Some(
                context
                    .settings()
                    .features(user.identity)
                    .await
                    .map_err(crate::resources::error)?,
            );
            preferences = preference_wire(user.preferences);
            (
                Some(BrowserUser {
                    id: user.identity.id,
                    username: user.username,
                    role: role(&user.role),
                    password_change_required: user.password_change_required,
                    permissions: user.role.permissions,
                }),
                actor.map(|a| a.username),
            )
        }
        Err(error)
            if matches!(
                error.status(),
                StatusCode::UNAUTHORIZED | StatusCode::NOT_FOUND
            ) =>
        {
            (None, None)
        }
        Err(error) => return Err(error),
    };
    Ok((
        context.csrf,
        Json(BrowserSession {
            features,
            preferences,
            csrf_token,
            user,
            actor,
        }),
    )
        .into_response())
}

fn preference_wire(
    value: ruphish_application::auth::Preferences,
) -> ruphish_contracts::auth::Preferences {
    ruphish_contracts::auth::Preferences {
        language: value.language,
        format_locale: value.format_locale,
        time_zone: value.time_zone,
        show_utc_offset: value.show_utc_offset,
    }
}

// Every browser-only endpoint passes this boundary, including future additions.
// Explicit API credentials must never fall back to an accompanying session.
async fn internal_session(
    context: Context,
    mut request: Request,
    next: Next,
) -> Result<Response, Rejection> {
    reject_internal_key(context.explicit_api_key())?;
    let (user, actor) = context.identity().await?;
    if actor.is_some() {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::AuthImpersonationActive,
            "Exit impersonation before managing accounts",
        ));
    }
    if user.password_change_required {
        return Err(auth_error(AccountError::ResetRequired.into()));
    }
    if !matches!(context.method, Method::GET | Method::HEAD | Method::OPTIONS) {
        context.csrf(None)?;
    }
    request.extensions_mut().insert(user);
    Ok(next.run(request).await)
}

fn reject_internal_key(supplied: bool) -> Result<(), Rejection> {
    if supplied {
        return Err(failure(
            StatusCode::UNAUTHORIZED,
            ErrorCode::AuthApiKeyInvalid,
            "This endpoint requires a browser session without API credentials",
        ));
    }
    Ok(())
}

async fn preferences(
    Extension(user): Extension<Account>,
) -> Result<Json<ruphish_contracts::auth::Preferences>, Rejection> {
    Ok(Json(preference_wire(user.preferences)))
}

async fn save_preferences(
    context: Context,
    Extension(actor): Extension<Account>,
    request: Request,
) -> Result<Json<ruphish_contracts::auth::Preferences>, Rejection> {
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        axum::body::to_bytes(request.into_body(), 4096),
    )
    .await
    .map_err(|_| {
        failure(
            StatusCode::REQUEST_TIMEOUT,
            ErrorCode::RequestTimeout,
            "Request body timed out",
        )
    })?
    .map_err(|_| {
        failure(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::RequestTooLarge,
            "Request body too large",
        )
    })?;
    let input: ruphish_contracts::auth::Preferences =
        serde_json::from_slice(&bytes).map_err(|_| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::PreferencesInvalid,
                "Invalid preferences",
            )
        })?;
    let result = context
        .state
        .service
        .save_preferences(
            actor.identity,
            ruphish_application::auth::Preferences {
                language: input.language,
                format_locale: input.format_locale,
                time_zone: input.time_zone,
                show_utc_offset: input.show_utc_offset,
            },
        )
        .await
        .map_err(auth_error)?;
    Ok(Json(preference_wire(result)))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Form {
    username: String,
    password: String,
    next: String,
    csrf_token: String,
    current_password: Option<String>,
    new_password: String,
    confirm_new_password: String,
    confirm_password: String,
    api_key: Option<String>,
}
async fn form(request: Request) -> Result<Form, Rejection> {
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        axum::body::to_bytes(request.into_body(), 16 * 1024),
    )
    .await
    .map_err(|_| {
        failure(
            StatusCode::REQUEST_TIMEOUT,
            ErrorCode::RequestTimeout,
            "Request body timed out",
        )
    })?
    .map_err(|_| {
        failure(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::RequestTooLarge,
            "Request body too large",
        )
    })?;
    serde_urlencoded::from_bytes(&bytes).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid form",
        )
    })
}
async fn login(context: Context, request: Request) -> Result<Response, Rejection> {
    let input = form(request).await?;
    context.csrf(Some(&input.csrf_token))?;
    if !context.state.limiter.admit(context.peer) {
        return Err(failure(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            "Too many login attempts",
        ));
    }
    let user = match context
        .state
        .service
        .login(&input.username, input.password)
        .await
    {
        Ok(user) => user,
        Err(error) => {
            let response = auth_error(error);
            if !context.json() && response.status() == StatusCode::UNAUTHORIZED {
                return Ok((
                    StatusCode::UNAUTHORIZED,
                    context.csrf,
                    Html((*context.state.index).clone()),
                )
                    .into_response());
            }
            return Err(response);
        }
    };
    context
        .establish(
            user.identity,
            None,
            datetime::now().as_second() + 12 * 60 * 60,
        )
        .await?;
    let query: HashMap<String, String> =
        serde_urlencoded::from_str(context.uri.query().unwrap_or("")).unwrap_or_default();
    let requested = if input.next.is_empty() {
        query.get("next").map(String::as_str).unwrap_or("")
    } else {
        &input.next
    };
    let next = if user.password_change_required && context.json() {
        "/reset_password".to_owned()
    } else {
        local_next(requested)
    };
    Ok(context.completed(&next))
}
async fn logout(context: Context, request: Request) -> Result<Response, Rejection> {
    let input = form(request).await?;
    context.csrf(Some(&input.csrf_token))?;
    context.session.flush().await.map_err(session_error)?;
    Ok(context.completed("/login"))
}
async fn password(context: Context, request: Request) -> Result<Response, Rejection> {
    let input = form(request).await?;
    let reset = context.uri.path() == "/reset_password";
    if !reset {
        reject_internal_key(input.api_key.is_some())?;
    }
    context.csrf(Some(&input.csrf_token))?;
    let (user, actor) = context.identity().await?;
    if actor.is_some() {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::AuthImpersonationActive,
            "Exit impersonation before changing credentials",
        ));
    }
    if reset && !user.password_change_required {
        return Ok(redirect(StatusCode::TEMPORARY_REDIRECT, "/user/password"));
    }
    let changed = context
        .state
        .service
        .change_password(
            user.identity,
            if reset { None } else { input.current_password },
            if reset {
                input.password
            } else {
                input.new_password
            },
            if reset {
                &input.confirm_password
            } else {
                &input.confirm_new_password
            },
        )
        .await
        .map_err(|error| {
            if matches!(error, Error::InvalidCredentials) {
                failure(
                    StatusCode::BAD_REQUEST,
                    ErrorCode::PasswordInvalid,
                    "Invalid password",
                )
            } else {
                auth_error(error)
            }
        })?;
    context
        .establish(
            changed.identity,
            None,
            datetime::now().as_second() + 12 * 60 * 60,
        )
        .await?;
    if reset {
        Ok(context.completed(&local_next(&input.next)))
    } else {
        Ok(Json(Envelope::<()> {
            success: true,
            message: "Settings Updated Successfully".into(),
            data: None,
        })
        .into_response())
    }
}
async fn impersonate(context: Context, request: Request) -> Result<Response, Rejection> {
    let input = form(request).await?;
    context.csrf(Some(&input.csrf_token))?;
    let (actor, original) = context.identity().await?;
    if original.is_some() {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::AuthImpersonationActive,
            "Exit the current impersonation first",
        ));
    }
    let target = context
        .state
        .service
        .impersonate(actor.identity, &input.username)
        .await
        .map_err(auth_error)?;
    let prior: Login = context
        .session
        .get("login")
        .await
        .map_err(session_error)?
        .ok_or_else(unauthorized)?;
    context
        .establish(target.identity, Some(actor.identity), prior.expires)
        .await?;
    tracing::info!(
        actor_id = actor.identity.id,
        user_id = target.identity.id,
        "impersonation_started"
    );
    Ok(context.completed("/"))
}
async fn exit_impersonation(context: Context, request: Request) -> Result<Response, Rejection> {
    let input = form(request).await?;
    context.csrf(Some(&input.csrf_token))?;
    let (_, actor) = context.identity().await?;
    let actor = actor.ok_or_else(|| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::AuthImpersonationInactive,
            "No impersonation is active",
        )
    })?;
    let prior: Login = context
        .session
        .get("login")
        .await
        .map_err(session_error)?
        .ok_or_else(unauthorized)?;
    context
        .establish(actor.identity, None, prior.expires)
        .await?;
    tracing::info!(actor_id = actor.identity.id, "impersonation_finished");
    Ok(context.completed("/users"))
}
async fn users_slash(context: Context) -> Response {
    let suffix = context
        .uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    redirect(
        StatusCode::MOVED_PERMANENTLY,
        &format!("/api/users/{suffix}"),
    )
}
async fn users(context: Context) -> Result<Json<Vec<User>>, Rejection> {
    let actor = context.api(None).await?;
    let users = context
        .state
        .service
        .list(actor.identity)
        .await
        .map_err(auth_error)?;
    Ok(Json(users.into_iter().map(wire).collect::<Result<_, _>>()?))
}
async fn user(context: Context, Path(id): Path<i64>) -> Result<Json<User>, Rejection> {
    let actor = context.api(None).await?;
    Ok(Json(wire(
        context
            .state
            .service
            .account(actor.identity, id)
            .await
            .map_err(auth_error)?,
    )?))
}
async fn edit(request: Request) -> Result<Edit, Rejection> {
    let body = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        axum::body::to_bytes(request.into_body(), 16 * 1024),
    )
    .await
    .map_err(|_| {
        failure(
            StatusCode::REQUEST_TIMEOUT,
            ErrorCode::RequestTimeout,
            "Request body timed out",
        )
    })?
    .map_err(|_| {
        failure(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::RequestTooLarge,
            "Request body too large",
        )
    })?;
    let value: UserEdit = serde_json::from_slice(&body).map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::RequestInvalid,
            "Invalid JSON",
        )
    })?;
    Ok(Edit {
        username: value.username,
        password: value.password,
        role: value.role,
        password_change_required: value.password_change_required,
        account_locked: value.account_locked,
    })
}
async fn create(context: Context, request: Request) -> Result<Json<User>, Rejection> {
    let actor = context.api(None).await?;
    Ok(Json(wire(
        context
            .state
            .service
            .save(actor.identity, None, edit(request).await?)
            .await
            .map_err(auth_error)?,
    )?))
}
async fn update(
    context: Context,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Json<User>, Rejection> {
    let actor = context.api(None).await?;
    let query: HashMap<String, String> =
        serde_urlencoded::from_str(context.uri.query().unwrap_or("")).unwrap_or_default();
    let browser =
        !context.headers.contains_key(header::AUTHORIZATION) && !query.contains_key("api_key");
    let login: Option<Login> = if browser && actor.identity.id == id {
        context.session.get("login").await.map_err(session_error)?
    } else {
        None
    };
    let updated = context
        .state
        .service
        .save(actor.identity, Some(id), edit(request).await?)
        .await
        .map_err(auth_error)?;
    if let Some(login) = login {
        if updated.account_locked {
            context.session.flush().await.map_err(session_error)?;
        } else {
            context
                .establish(updated.identity, None, login.expires)
                .await?;
        }
    }
    tracing::info!(
        actor_id = actor.identity.id,
        user_id = id,
        "account_updated"
    );
    Ok(Json(wire(updated)?))
}
async fn delete(context: Context, Path(id): Path<i64>) -> Result<Json<Envelope<()>>, Rejection> {
    let actor = context.api(None).await?;
    context
        .state
        .service
        .delete(actor.identity, id)
        .await
        .map_err(auth_error)?;
    tracing::info!(
        actor_id = actor.identity.id,
        user_id = id,
        "account_deleted"
    );
    Ok(Json(Envelope {
        success: true,
        message: "User deleted Successfully!".into(),
        data: None,
    }))
}
async fn reset_key(
    context: Context,
    request: Request,
) -> Result<Json<Envelope<String>>, Rejection> {
    let input = form(request).await?;
    let actor = context.api(input.api_key.as_deref()).await?;
    let user = context
        .state
        .service
        .rotate_key(actor.identity)
        .await
        .map_err(auth_error)?;
    Ok(Json(Envelope {
        success: true,
        message: "API Key successfully reset".into(),
        data: Some(user.api_key),
    }))
}
fn role(role: &ruphish_application::auth::Role) -> Role {
    Role {
        slug: role.slug.clone(),
        name: role.name.clone(),
        description: role.description.clone(),
    }
}
fn wire(user: Account) -> Result<User, Rejection> {
    let timestamp = if user.last_login.starts_with("0001-01-01") || user.last_login.is_empty() {
        Timestamp::default()
    } else {
        Timestamp(
            datetime::parse_stored(&user.last_login).map_err(|_| auth_error(Error::Internal))?,
        )
    };
    Ok(User {
        id: user.identity.id,
        username: user.username,
        api_key: user.api_key,
        role: role(&user.role),
        password_change_required: user.password_change_required,
        account_locked: user.account_locked,
        last_login: timestamp,
    })
}
fn local_next(value: &str) -> String {
    let path = url::Url::parse(value)
        .ok()
        .map(|u| u.path().to_owned())
        .unwrap_or_else(|| value.split(['?', '#']).next().unwrap_or("").to_owned());
    // Backslashes and encoded separators must never become a browser authority redirect.
    if path.contains('\\')
        || path.to_ascii_lowercase().contains("%5c")
        || path.to_ascii_lowercase().starts_with("%2f")
    {
        return "/".into();
    }
    format!("/{}", path.trim_start_matches('/'))
}
fn redirect(status: StatusCode, target: &str) -> Response {
    (status, [(header::LOCATION, target)]).into_response()
}
fn unauthorized() -> Rejection {
    failure(
        StatusCode::UNAUTHORIZED,
        ErrorCode::AuthRequired,
        "Authentication required",
    )
}
pub(crate) struct Rejection {
    status: StatusCode,
    message: String,
    code: ErrorCode,
    validation: Option<ruphish_contracts::validation::ValidationReport>,
}
impl Rejection {
    pub(crate) fn with_validation(
        mut self,
        report: &ruphish_application::validation::Report,
    ) -> Self {
        if !report.is_empty() {
            self.validation = Some(report.into());
        }
        self
    }
    fn status(&self) -> StatusCode {
        self.status
    }
}
impl IntoResponse for Rejection {
    fn into_response(self) -> Response {
        tracing::debug!(
            error_code = self.code.as_str(),
            status = self.status.as_u16(),
            "Request rejected"
        );
        let mut body = serde_json::json!({"success": false, "message": self.message, "data": null, "code": self.code.as_str()});
        if let Some(validation) = self.validation {
            body["validation"] = serde_json::json!(validation);
        }
        let mut response = (self.status, Json(body)).into_response();
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, header::HeaderValue::from_static("12"));
        }
        response
    }
}
pub(crate) fn failure(status: StatusCode, code: ErrorCode, message: &str) -> Rejection {
    Rejection {
        status,
        message: message.into(),
        code,
        validation: None,
    }
}
fn session_error(_: tower_sessions::session::Error) -> Rejection {
    auth_error(Error::Internal)
}
pub(crate) fn policy_code(error: &AccountError) -> ErrorCode {
    match error {
        AccountError::Unauthorized => ErrorCode::AuthRequired,
        AccountError::Forbidden | AccountError::Escalation => ErrorCode::AuthForbidden,
        AccountError::ResetRequired => ErrorCode::PasswordChangeRequired,
        AccountError::EmptyUsername => ErrorCode::UsernameRequired,
        AccountError::EmptyRole => ErrorCode::RoleRequired,
        AccountError::LastAdmin => ErrorCode::LastAdmin,
        AccountError::SelfDeletion => ErrorCode::SelfDeletion,
    }
}
fn auth_code(error: &Error) -> ErrorCode {
    use ruphish_application::auth::PasswordError;
    match error {
        Error::Validation(report) => match report.issues().first() {
            Some(issue) => match (issue.field.as_str(), issue.code) {
                ("username", _) => ErrorCode::UsernameRequired,
                ("role", _) => ErrorCode::RoleRequired,
                ("language" | "format_locale" | "time_zone", _) => ErrorCode::PreferencesInvalid,
                (_, ruphish_application::validation::Code::PasswordShort) => {
                    ErrorCode::PasswordTooShort
                }
                (_, ruphish_application::validation::Code::PasswordLong) => {
                    ErrorCode::PasswordTooLong
                }
                (_, ruphish_application::validation::Code::PasswordMismatch) => {
                    ErrorCode::PasswordMismatch
                }
                _ => ErrorCode::PasswordRequired,
            },
            None => ErrorCode::RequestInvalid,
        },
        Error::Policy(error) => policy_code(error),
        Error::Preferences(_) => ErrorCode::PreferencesInvalid,
        Error::Password(error) => match error {
            PasswordError::Empty => ErrorCode::PasswordRequired,
            PasswordError::TooShort => ErrorCode::PasswordTooShort,
            PasswordError::Mismatch => ErrorCode::PasswordMismatch,
            PasswordError::Reused => ErrorCode::PasswordReused,
        },
        Error::InvalidCredentials => ErrorCode::AuthInvalidCredentials,
        Error::NotFound => ErrorCode::AccountNotFound,
        Error::Conflict => ErrorCode::UsernameTaken,
        Error::RoleNotFound => ErrorCode::RoleNotFound,
        Error::Changed => ErrorCode::AuthChanged,
        Error::Busy => ErrorCode::ServiceBusy,
        Error::TooLong => ErrorCode::PasswordTooLong,
        Error::Storage(_) | Error::Internal => ErrorCode::InternalError,
    }
}
fn auth_error(error: Error) -> Rejection {
    let status = match &error {
        Error::InvalidCredentials | Error::Changed | Error::Policy(AccountError::Unauthorized) => {
            StatusCode::UNAUTHORIZED
        }
        Error::NotFound => StatusCode::NOT_FOUND,
        Error::Policy(
            AccountError::Forbidden | AccountError::ResetRequired | AccountError::SelfDeletion,
        ) => StatusCode::FORBIDDEN,
        Error::Policy(AccountError::LastAdmin)
        | Error::Internal
        | Error::Storage(_)
        | Error::RoleNotFound => StatusCode::INTERNAL_SERVER_ERROR,
        Error::Busy => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_REQUEST,
    };
    let message = if matches!(error, Error::RoleNotFound) {
        "record not found".to_owned()
    } else {
        error.to_string()
    };
    let rejection = failure(status, auth_code(&error), &message);
    match &error {
        Error::Validation(report) => rejection.with_validation(report),
        Error::Conflict
        | Error::RoleNotFound
        | Error::Password(ruphish_application::auth::PasswordError::Reused) => {
            let field = match error {
                Error::Conflict => "username",
                Error::RoleNotFound => "role",
                _ => "password",
            };
            let mut report = ruphish_application::validation::Report::default();
            report.reject(
                field,
                ruphish_application::validation::Code::Conflict,
                "Choose a different value",
            );
            rejection.with_validation(&report)
        }
        _ => rejection,
    }
}
async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let api = request.uri().path().starts_with("/api/");
    let mut response = if api && request.method() == Method::OPTIONS {
        StatusCode::OK.into_response()
    } else {
        next.run(request).await
    };
    if api {
        response.headers_mut().insert(
            "access-control-allow-origin",
            header::HeaderValue::from_static("*"),
        );
        response.headers_mut().insert(
            "access-control-allow-methods",
            header::HeaderValue::from_static("POST, GET, OPTIONS, PUT, DELETE"),
        );
        response.headers_mut().insert(
            "access-control-allow-headers",
            header::HeaderValue::from_static("Content-Type, Authorization"),
        );
        // No Allow-Credentials: external browser integrations must supply their API key.
    }
    page_security_headers(response)
}

fn page_security_headers(mut response: Response) -> Response {
    response.headers_mut().insert(
        "content-security-policy",
        header::HeaderValue::from_static("frame-ancestors 'none';"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        header::HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        "referrer-policy",
        header::HeaderValue::from_static("no-referrer"),
    );
    response
        .headers_mut()
        .insert("x-frame-options", header::HeaderValue::from_static("DENY"));
    response
}

#[cfg(test)]
mod error_contract_tests {
    use super::*;

    #[tokio::test]
    async fn error_codes_extend_legacy_errors_without_changing_status_or_message() {
        let response = auth_error(Error::InvalidCredentials).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["code"], "AUTH_INVALID_CREDENTIALS");
        assert_eq!(value["message"], "Invalid Username/Password");
        assert_eq!(value["success"], false);
        assert!(value["data"].is_null());
        let legacy: Envelope<()> = serde_json::from_slice(&body).unwrap();
        assert!(!legacy.success);
        let response = failure(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            "Too many login attempts",
        )
        .into_response();
        assert_eq!(response.headers()[header::RETRY_AFTER], "12");
    }
}

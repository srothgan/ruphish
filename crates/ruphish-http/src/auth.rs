//! Browser sessions, CSRF and legacy account API boundaries.
use super::rate::LoginLimiter;
use axum::{
    Json, Router,
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
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tower_sessions::{
    Session, SessionManagerLayer, SessionStore,
    cookie::{Key, SameSite},
    session::Expiry,
};

#[derive(Clone)]
struct AuthState {
    service: Service,
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
    service: Service,
    store: S,
    key: &[u8; 64],
    secure: bool,
    trusted_origins: Vec<String>,
    csrf_key: String,
) -> Router {
    let state = AuthState {
        service,
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
        .route("/settings", get(page).post(password))
        .route("/users", get(page))
        .route("/auth/session", get(bootstrap))
        .route("/impersonate", post(impersonate))
        .route("/impersonate/exit", post(exit_impersonation))
        .route("/api/users", get(users_slash).post(users_slash))
        .route("/api/users/", get(users).post(create))
        .route("/api/users/{id}", get(user).put(update).delete(delete))
        .route("/api/reset", post(reset_key))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
        .layer(session);
    super::server::finish(assets.router.merge(routes))
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

struct Context {
    state: AuthState,
    session: Session,
    csrf: CsrfToken,
    headers: HeaderMap,
    uri: Uri,
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
    async fn identity(&self) -> Result<(Account, Option<Account>), Rejection> {
        let login: Login = self
            .session
            .get("login")
            .await
            .map_err(session_error)?
            .ok_or_else(unauthorized)?;
        if login.expires <= OffsetDateTime::now_utc().unix_timestamp() {
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
        self.csrf
            .verify(token)
            .map_err(|_| failure(StatusCode::FORBIDDEN, "Invalid CSRF token"))?;
        if let Some(origin) = self.headers.get(header::ORIGIN) {
            self.origin(origin.to_str().unwrap_or(""))?;
        } else if let Some(referer) = self.headers.get(header::REFERER) {
            let origin = url::Url::parse(referer.to_str().unwrap_or(""))
                .map_err(|_| failure(StatusCode::FORBIDDEN, "Invalid request origin"))?
                .origin()
                .ascii_serialization();
            self.origin(&origin)?;
        }
        if self
            .headers
            .get("sec-fetch-site")
            .is_some_and(|v| v == "cross-site")
        {
            return Err(failure(StatusCode::FORBIDDEN, "Cross-site request denied"));
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
        .map_err(|_| failure(StatusCode::FORBIDDEN, "Invalid request origin"))?
        .origin();
        if let (Some(_), Some(host)) = (authority, header_host)
            && !url::Url::parse(&format!("{scheme}://{host}"))
                .is_ok_and(|host| host.origin() == request_origin)
        {
            return Err(failure(
                StatusCode::FORBIDDEN,
                "Conflicting request authority",
            ));
        }
        let parsed = url::Url::parse(origin)
            .map_err(|_| failure(StatusCode::FORBIDDEN, "Invalid request origin"))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(failure(StatusCode::FORBIDDEN, "Invalid request origin"));
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
            return Err(failure(StatusCode::FORBIDDEN, "Invalid request origin"));
        }
        Ok(())
    }
    async fn api(&self, form_key: Option<&str>) -> Result<Account, Rejection> {
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
                    Error::Internal | Error::Busy => auth_error(error),
                    _ => failure(
                        StatusCode::UNAUTHORIZED,
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
        if actor.is_some() {
            return Err(failure(
                StatusCode::FORBIDDEN,
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
                    return redirect(StatusCode::TEMPORARY_REDIRECT, "/settings");
                }
                if path == "/users" && !user.permits("modify_system") {
                    return failure(StatusCode::FORBIDDEN, "Forbidden").into_response();
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
    let (user, actor) = match context.identity().await {
        Ok((user, actor)) => (
            Some(BrowserUser {
                id: user.identity.id,
                username: user.username,
                role: role(&user.role),
                password_change_required: user.password_change_required,
                permissions: user.role.permissions,
            }),
            actor.map(|a| a.username),
        ),
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
            csrf_token,
            user,
            actor,
        }),
    )
        .into_response())
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
    .map_err(|_| failure(StatusCode::REQUEST_TIMEOUT, "Request body timed out"))?
    .map_err(|_| failure(StatusCode::PAYLOAD_TOO_LARGE, "Request body too large"))?;
    serde_urlencoded::from_bytes(&bytes)
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid form"))
}
async fn login(context: Context, request: Request) -> Result<Response, Rejection> {
    let input = form(request).await?;
    context.csrf(Some(&input.csrf_token))?;
    if !context.state.limiter.admit(context.peer) {
        return Err(failure(
            StatusCode::TOO_MANY_REQUESTS,
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
            (OffsetDateTime::now_utc() + Duration::hours(12)).unix_timestamp(),
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
    context.csrf(Some(&input.csrf_token))?;
    let (user, actor) = context.identity().await?;
    if actor.is_some() {
        return Err(failure(
            StatusCode::FORBIDDEN,
            "Exit impersonation before changing credentials",
        ));
    }
    let reset = context.uri.path() == "/reset_password";
    if reset && !user.password_change_required {
        return Ok(redirect(StatusCode::TEMPORARY_REDIRECT, "/settings"));
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
                failure(StatusCode::BAD_REQUEST, "Invalid password")
            } else {
                auth_error(error)
            }
        })?;
    context
        .establish(
            changed.identity,
            None,
            (OffsetDateTime::now_utc() + Duration::hours(12)).unix_timestamp(),
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
    let actor =
        actor.ok_or_else(|| failure(StatusCode::BAD_REQUEST, "No impersonation is active"))?;
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
    .map_err(|_| failure(StatusCode::REQUEST_TIMEOUT, "Request body timed out"))?
    .map_err(|_| failure(StatusCode::PAYLOAD_TOO_LARGE, "Request body too large"))?;
    let value: UserEdit = serde_json::from_slice(&body)
        .map_err(|_| failure(StatusCode::BAD_REQUEST, "Invalid JSON"))?;
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
            OffsetDateTime::parse(&user.last_login, &Rfc3339)
                .or_else(|_| {
                    OffsetDateTime::parse(
                        &user.last_login.replace(' ', "T").replace("+00:00", "Z"),
                        &Rfc3339,
                    )
                })
                .map_err(|_| auth_error(Error::Internal))?,
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
    failure(StatusCode::UNAUTHORIZED, "Authentication required")
}
struct Rejection {
    status: StatusCode,
    message: String,
}
impl Rejection {
    fn status(&self) -> StatusCode {
        self.status
    }
}
impl IntoResponse for Rejection {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(Envelope::<()> {
                success: false,
                message: self.message,
                data: None,
            }),
        )
            .into_response();
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, header::HeaderValue::from_static("12"));
        }
        response
    }
}
fn failure(status: StatusCode, message: &str) -> Rejection {
    Rejection {
        status,
        message: message.into(),
    }
}
fn session_error(_: tower_sessions::session::Error) -> Rejection {
    auth_error(Error::Internal)
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
        Error::Policy(AccountError::LastAdmin) | Error::Internal | Error::RoleNotFound => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        Error::Busy => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_REQUEST,
    };
    let message = if matches!(error, Error::RoleNotFound) {
        "record not found".to_owned()
    } else {
        error.to_string()
    };
    failure(status, &message)
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

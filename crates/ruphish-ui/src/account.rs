//! Account pages using the public API and session-only browser actions.
use crate::feedback::Message;
use crate::shared::components::*;
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local_scoped_with_cancellation as spawn_local;
use ruphish_contracts::auth::{BrowserSession, UserEdit};
use ruphish_ui::session::{SessionAction, SessionMonitor};
use ruphish_ui::{tr, trs};
use serde_json::Value;

#[path = "account_menu.rs"]
mod account_menu;
pub(super) use account_menu::copy_text;
#[path = "users.rs"]
mod users;

#[derive(Clone, Copy)]
struct PasswordNotice(RwSignal<bool>);

/// localStorage key for the persisted desktop sidebar collapse state (per device).
const NAV_COLLAPSED_KEY: &str = "nav-collapsed";

/// Whether the viewport is in the mobile range where the sidebar is an off-canvas drawer.
/// Checked at click time so no resize listener is needed; the breakpoint matches the stylesheet.
fn viewport_is_narrow() -> bool {
    web_sys::window()
        .and_then(|window| window.match_media("(max-width: 700px)").ok().flatten())
        .is_some_and(|query| query.matches())
}

/// Sidebar-panel glyph for the collapse/expand control; the left column fills when the
/// sidebar is expanded and is hollow when collapsed, as a quiet state cue.
fn nav_toggle_icon(expanded: bool) -> impl IntoView {
    view! {
        <svg
            class="nav-toggle-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.7"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
        >
            <rect x="3" y="4" width="18" height="16" rx="2" />
            <line x1="9" y1="4" x2="9" y2="20" />
            {expanded
                .then(|| {
                    view! {
                        <rect
                            x="3.9"
                            y="4.9"
                            width="4.2"
                            height="14.2"
                            rx="1"
                            fill="currentColor"
                            stroke="none"
                        />
                    }
                })}
        </svg>
    }
}

#[derive(Clone, Copy)]
pub(super) struct Api {
    pub(super) features: RwSignal<ruphish_contracts::settings::Features>,
    feature_generation: RwSignal<u64>,
    monitor: RwSignal<SessionMonitor>,
    i18n: leptos_i18n::I18nContext<ruphish_ui::localization::Locale>,
}
impl Api {
    pub(super) fn set_features(self, value: ruphish_contracts::settings::Features) {
        self.feature_generation.update(|n| *n = n.wrapping_add(1));
        self.features.set(value);
    }
    pub(super) fn invalid_read(self) -> String {
        leptos_i18n::t_string!(self.i18n, ui.invalid_read_response).to_owned()
    }
    /// Decode required wire fields before a read can enter a query cache.
    pub(super) async fn read<T: serde::de::DeserializeOwned>(
        self,
        path: &str,
    ) -> Result<T, String> {
        let value = self.get(path).await?;
        serde_json::from_value(value)
            .map_err(|_| leptos_i18n::t_string!(self.i18n, ui.invalid_read_response).to_owned())
    }

    pub(super) async fn launch(
        self,
        path: &str,
        token: &str,
        key: &str,
        value: &Value,
    ) -> Result<Value, (u16, String)> {
        let i18n = self.i18n;

        if !self.active() {
            return Err((
                401,
                leptos_i18n::t_string!(i18n, ui.your_session_ended).into(),
            ));
        }
        let request = ruphish_ui::browser_http::launch_request(path, token, key, value)
            .map_err(|e| (0, e))?;
        let response=self.send_request(path, request).await.map_err(|_|(0,leptos_i18n::t_string!(i18n, ui.launch_outcome_is_not_known_retry_uses_the_same_launch_key_do_not_create_another).into()))?;
        let status = response.status;
        let value = self
            .response(path, response)
            .await
            .map_err(|e| (status, e))?;
        if value["id"].as_i64().is_none_or(|id| id <= 0) {
            return Err((0, leptos_i18n::t_string!(i18n, ui.launch_outcome_is_not_known_retry_uses_the_same_launch_key_do_not_create_another).into()));
        }
        Ok(value)
    }
    pub(super) async fn resource(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&Value>,
        revision: Option<&str>,
    ) -> Result<(Value, Option<String>), String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(
                i18n,
                ui.your_session_ended_sign_in_again_to_continue
            )
            .into());
        }
        let request =
            ruphish_ui::browser_http::resource_request(method, path, token, value, revision)?;
        let response = self.send_request(path, request).await?;
        let etag = response.etag.clone();
        self.response(path, response)
            .await
            .map(|value| (value, etag))
    }
    /// Only explicit acknowledgements confirm actions without a resource response.
    pub(super) async fn command(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&Value>,
        revision: Option<&str>,
    ) -> Result<(), String> {
        let (value, _) = self.resource(method, path, token, value, revision).await?;
        if value["success"] != true {
            return Err(self.invalid_read());
        }
        Ok(())
    }

    pub(super) async fn download(self, path: &str, filename: &str) -> Result<(), String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(i18n, ui.your_session_ended).into());
        }
        let request = ruphish_ui::browser_http::resource_request("GET", path, "", None, None)?;
        let observed_path = path.to_owned();
        let (response, file) = ruphish_ui::browser_http::download(request, move |status| {
            self.monitor
                .update(|monitor| monitor.response(&observed_path, status));
        })
        .await?;
        if !self.active() {
            return Err(leptos_i18n::t_string!(i18n, ui.your_session_ended).into());
        }
        if !(200..300).contains(&response.status) {
            self.response(path, response).await?;
            return Err(leptos_i18n::t_string!(i18n, ui.the_export_could_not_be_generated).into());
        }
        ruphish_ui::browser_http::save_file(&file, filename)
    }
    fn active(self) -> bool {
        self.monitor
            .with_untracked(|m| m.action() == SessionAction::Continue)
    }
}

fn location() -> String {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".into())
}
fn navigate(path: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href(path);
    }
}
impl Api {
    async fn send_request(
        self,
        path: &str,
        request: Request,
    ) -> Result<ruphish_ui::browser_http::Received, String> {
        let path = path.to_owned();
        ruphish_ui::browser_http::send(request, move |status| {
            self.monitor.update(|m| m.response(&path, status))
        })
        .await
    }
    pub(super) async fn response(
        self,
        _path: &str,
        response: ruphish_ui::browser_http::Received,
    ) -> Result<Value, String> {
        let i18n = self.i18n;

        let status = response.status;
        if !self.active() {
            return Err(leptos_i18n::t_string!(
                i18n,
                ui.your_session_ended_sign_in_again_to_continue
            )
            .into());
        }
        let text = response.body;
        let decoded = ruphish_ui::browser_http::decode(status, &text);
        let value = if (200..300).contains(&status) {
            decoded?
        } else {
            decoded.unwrap_or(Value::Null)
        };
        if !(200..300).contains(&status) || value["success"] == false {
            let code = value["code"].as_str().unwrap_or(match status {
                401 => "AUTH_REQUIRED",
                403 => "AUTH_FORBIDDEN",
                429 => "RATE_LIMITED",
                503 => "SERVICE_BUSY",
                _ => "REQUEST_FAILED",
            });
            return Err(ruphish_ui::localization::failure_label(i18n, code));
        }
        Ok(value)
    }
    pub(super) async fn get(self, path: &str) -> Result<Value, String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(
                i18n,
                ui.your_session_ended_sign_in_again_to_continue
            )
            .into());
        }
        let request = Request::get(path)
            .header("x-ruphish-client", "browser")
            .header("accept", "application/json")
            .build()
            .map_err(|_| leptos_i18n::t_string!(i18n, ui.cannot_prepare_request).to_owned())?;
        self.response(path, self.send_request(path, request).await?)
            .await
    }

    async fn form(
        self,
        path: &str,
        token: &str,
        fields: &[(&str, String)],
    ) -> Result<Value, String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(
                i18n,
                ui.your_session_ended_sign_in_again_to_continue
            )
            .into());
        }
        let body = serde_urlencoded::to_string(fields)
            .map_err(|_| leptos_i18n::t_string!(i18n, ui.cannot_prepare_the_request).to_owned())?;
        let request = Request::post(path)
            .header("x-ruphish-client", "browser")
            .header("accept", "application/json")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("x-csrf-token", token)
            .body(body)
            .map_err(|_| leptos_i18n::t_string!(i18n, ui.cannot_prepare_the_request).to_owned())?;
        self.response(path, self.send_request(path, request).await?)
            .await
    }
    async fn json(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&UserEdit>,
    ) -> Result<Value, String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(
                i18n,
                ui.your_session_ended_sign_in_again_to_continue
            )
            .into());
        }
        let builder = match method {
            "PUT" => Request::put(path),
            "DELETE" => Request::delete(path),
            _ => Request::post(path),
        }
        .header("x-ruphish-client", "browser")
        .header("accept", "application/json")
        .header("x-csrf-token", token);
        let request = if let Some(value) = value {
            builder.json(value)
        } else {
            builder.build()
        }
        .map_err(|_| leptos_i18n::t_string!(i18n, ui.cannot_prepare_the_request).to_owned())?;
        self.response(path, self.send_request(path, request).await?)
            .await
    }
}

fn decode_session(value: serde_json::Value) -> Result<BrowserSession, ()> {
    // Missing fields must not silently reset an account to browser defaults.
    ruphish_ui::preference_state::decode(value.get("preferences").cloned().ok_or(())?).ok_or(())?;
    let session: BrowserSession = serde_json::from_value(value).map_err(|_| ())?;
    if session.user.is_some() && session.features.is_none() {
        return Err(());
    }
    Ok(session)
}

pub(super) fn app() -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    ruphish_ui::query::provide_client();

    let preferences = ruphish_ui::display_preferences::DisplayPreferences::provide();
    let session = RwSignal::new(None::<Result<BrowserSession, String>>);
    let api = Api {
        features: RwSignal::new(Default::default()),
        feature_generation: RwSignal::new(0),
        monitor: RwSignal::new(SessionMonitor::default()),
        i18n,
    };
    spawn_local(async move {
        let loaded = api.get("/auth/session").await.and_then(|v| {
            decode_session(v)
                .map_err(|_| leptos_i18n::t_string!(i18n, ui.invalid_session_response).into())
        });
        if let Ok(value) = &loaded {
            if let Some(features) = value.features {
                api.set_features(features);
            }
            preferences.apply(value.preferences.clone(), i18n);
            api.monitor.update(|m| m.observe(value));
        }
        session.set(Some(loaded));
    });
    let navigation_guard = crate::navigation::install_guard(move || api.active());
    let checking = RwSignal::new(false);
    let owner = StoredValue::new(Owner::current());
    let revalidate = move || {
        if !api.active() || session.get_untracked().is_none() || checking.get_untracked() {
            return;
        }
        checking.set(true);
        let preference_ticket = preferences.read_ticket();
        let feature_ticket = api.feature_generation.get_untracked();
        // Global browser callbacks do not establish a Leptos owner themselves.
        owner.with_value(|owner| {
            if let Some(owner) = owner {
                owner.with(|| {
                    spawn_local(async move {
                        match api.get("/auth/session").await.and_then(|v| {
                            decode_session(v).map_err(|_| {
                                leptos_i18n::t_string!(i18n, ui.invalid_session_response).into()
                            })
                        }) {
                            Ok(value) => {
                                api.monitor.update(|m| m.observe(&value));
                                if api.active() {
                                    if feature_ticket == api.feature_generation.get_untracked()
                                        && let Some(features) = value.features
                                    {
                                        api.set_features(features);
                                    }
                                    preferences.apply_read(
                                        preference_ticket,
                                        value.preferences,
                                        i18n,
                                    );
                                }
                            }
                            Err(error) => preferences.read_failed(preference_ticket, error),
                        }
                        checking.set(false);
                    })
                });
            }
        });
    };
    provide_context(ruphish_ui::display_preferences::PreferenceRefresh {
        fetching: checking,
        refresh: Callback::new(move |()| revalidate()),
    });
    let route = leptos_router::hooks::use_location();
    Effect::new(move |_| {
        route.pathname.track();
        route.search.track();
        revalidate();
    });
    let focus = window_event_listener(leptos::ev::focus, move |_| revalidate());
    let visible = window_event_listener(leptos::ev::visibilitychange, move |_| {
        if web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|d| !d.hidden())
        {
            revalidate();
        }
    });
    let pageshow = window_event_listener(leptos::ev::pageshow, move |_| revalidate());
    on_cleanup(move || {
        focus.remove();
        visible.remove();
        pageshow.remove();
    });
    // The authenticated shell survives routing and routine session checks.
    // Only an identity/permission change invalidates it.
    let action = Memo::new(move |_| api.monitor.with(|m| m.action()));
    Effect::new(move |_| {
        let action = action.get();
        if let Some(window) = web_sys::window() {
            match action {
                SessionAction::Continue => {}
                SessionAction::SignedOut => {
                    let _ = window.location().replace("/login");
                }
                SessionAction::SignIn => {
                    let next = location();
                    let query = serde_urlencoded::to_string([
                        ("next", next.as_str()),
                        ("reason", "session-ended"),
                    ])
                    .unwrap_or_default();
                    let _ = window.location().replace(&format!("/login?{query}"));
                }
                SessionAction::Reload => {
                    let _ = window.location().reload();
                }
            }
        }
    });
    view! {
        <div class="account-app">
            {navigation_guard}
            {move || {
                if action.get() != SessionAction::Continue {
                    view! {
                        <main class="login-screen">
                            <p role="status">{tr!(ui.refreshing_your_session)}</p>
                            <a rel="external" href="/login">
                                {tr!(ui.sign_in_again)}
                            </a>
                        </main>
                    }
                        .into_any()
                } else {
                    match session.get() {
                        None => {
                            view! {
                                <main class="login-screen">
                                    <p role="status">{tr!(ui.opening_ruphish)}</p>
                                </main>
                            }
                                .into_any()
                        }
                        Some(Err(error)) => {
                            view! {
                                <main class="login-screen">
                                    <h1>{tr!(ui.ruphish)}</h1>
                                    <p class="form-message error" role="alert">
                                        {error}
                                    </p>
                                    <a rel="external" href="/login">
                                        {tr!(ui.try_signing_in_again)}
                                    </a>
                                </main>
                            }
                                .into_any()
                        }
                        Some(Ok(session)) if session.user.is_none() => {
                            login(api, session.csrf_token).into_any()
                        }
                        Some(Ok(session)) => shell(api, session).into_any(),
                    }
                }
            }}
        </div>
    }
}
fn login(api: Api, token: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(token);
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let expired = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .is_some_and(|q| {
            q.trim_start_matches('?')
                .split('&')
                .any(|p| p == "reason=session-ended")
        });
    let message = Message::new(if expired {
        leptos_i18n::t_string!(i18n, ui.your_session_ended_sign_in_again_to_continue).to_owned()
    } else {
        String::new()
    });
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        spawn_local(async move {
            let next = web_sys::window()
                .and_then(|w| w.location().search().ok())
                .and_then(|s| {
                    serde_urlencoded::from_str::<std::collections::HashMap<String, String>>(
                        s.trim_start_matches('?'),
                    )
                    .ok()
                })
                .and_then(|mut q| q.remove("next"))
                .unwrap_or_else(|| "/".into());
            let result = api
                .form(
                    "/login",
                    &token.get_value(),
                    &[
                        ("username", username.get_untracked()),
                        ("password", password.get_untracked()),
                        ("next", next),
                    ],
                )
                .await;
            password.set(String::new());
            busy.set(false);
            match result {
                Ok(value) => navigate(value["data"].as_str().unwrap_or("/")),
                Err(error) => message.error(error),
            }
        });
    };
    view! {
        <main class="login-screen">
            <section class="login-form">
                <div class="login-brand">{tr!(ui.ruphish)}</div>
                <h1>{tr!(ui.sign_in)}</h1>
                <p>{tr!(ui.manage_your_security_awareness_workspace)}</p>
                <form on:submit=submit>
                    <label>
                        {tr!(ui.username)}
                        <TextInput
                            name="username"
                            autocomplete="username"
                            required=true
                            value=Signal::derive(move || username.get())
                            on_input=move |e| username.set(event_target_value(&e))
                        />
                    </label>
                    <label>
                        {tr!(ui.password)}
                        <TextInput
                            name="password"
                            input_type="password"
                            autocomplete="current-password"
                            required=true
                            value=Signal::derive(move || password.get())
                            on_input=move |e| password.set(event_target_value(&e))
                        />
                    </label>
                    {message.view()}
                    <ActionButton
                        style=ButtonStyle::Primary
                        kind="submit"
                        disabled=Signal::derive(move || busy.get())
                    >
                        {move || {
                            if busy.get() {
                                leptos_i18n::t_string!(i18n, ui.signing_in)
                            } else {
                                leptos_i18n::t_string!(i18n, ui.sign_in)
                            }
                        }}
                    </ActionButton>
                </form>
            </section>
        </main>
    }
}
fn shell(api: Api, session: BrowserSession) -> impl IntoView {
    provide_context(api);
    let queries = expect_context::<leptos_fetch::QueryClient>();
    crate::overview_queries::OverviewQueries::provide(api, queries);
    crate::campaign_queries::CampaignQueries::provide(api, queries);
    crate::resource_queries::ResourceQueries::provide(api, queries);
    crate::management_queries::ManagementQueries::provide(api, queries);
    provide_context(ruphish_ui::import_drafts::ImportDrafts::default());
    provide_context(PasswordNotice(RwSignal::new(false)));
    provide_context(users::Notice(RwSignal::new(None)));
    let user = session.user.clone();
    provide_context(crate::resources::ResourceAccess(user.as_ref().is_some_and(
        |user| {
            user.permissions
                .iter()
                .any(|permission| permission == "modify_objects")
        },
    )));
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_default();
    let role_name = StoredValue::new(user.as_ref().map(|u| u.role.clone()));
    let role_name = Signal::derive(move || {
        role_name.with_value(|role| {
            role.as_ref()
                .map(|role| ruphish_ui::localization::role_label(api.i18n, role))
                .unwrap_or_default()
        })
    });
    let admin = user
        .as_ref()
        .is_some_and(|u| u.permissions.iter().any(|p| p == "modify_system"));
    let required = user.as_ref().is_some_and(|u| u.password_change_required);
    let token = StoredValue::new(session.csrf_token.clone());
    let message = Message::new(String::new());
    let actor = session.actor.clone();
    let active = actor.is_some();
    let username_banner = username.clone();
    let can_leave = crate::navigation::confirm_leave_action();
    let action_busy = RwSignal::new(false);
    let action_tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let action = move |path: &'static str| {
        if action_busy.get_untracked() {
            return;
        }
        action_busy.set(true);
        action_tasks.spawn(async move {
            if !can_leave().await || !api.active() {
                action_busy.set(false);
                return;
            }
            match api.form(path, &token.get_value(), &[]).await {
                Ok(_) if path == "/logout" => api.monitor.update(|m| m.signed_out()),
                Ok(value) => navigate(value["data"].as_str().unwrap_or("/login")),
                Err(error) => message.error(error),
            }
            action_busy.set(false);
        });
    };
    use crate::resources::{self, Family, Screen};
    use leptos_router::{
        components::{Route, Routes},
        hooks::use_location,
        path,
    };
    let path = use_location().pathname;
    // Desktop icon-rail state persists per device; the mobile drawer is always ephemeral.
    let nav_collapsed = ruphish_ui::local_storage::persistent(NAV_COLLAPSED_KEY, false);
    let drawer_open = RwSignal::new(false);
    // Close the mobile drawer on any navigation so it never covers the page just opened.
    Effect::new(move |_| {
        path.track();
        drawer_open.set(false);
    });
    let expand_label = trs!(ui.expand_navigation);
    let collapse_label = trs!(ui.collapse_navigation);
    let session = StoredValue::new(session);
    enum AccountPage {
        Overview,
        Preferences,
        Password,
        Users(users::Screen),
        CompanySettings,
        StarterTemplates,
    }
    let account_page = move |page: AccountPage| {
        if required {
            return password_form(api, token.get_value(), true).into_any();
        }
        if active {
            return view! {
                <Card class="account-panel">
                    <h1>{tr!(ui.account_management)}</h1>
                    <p>{tr!(ui.exit_impersonation_to_manage_accounts_and_credentials)}</p>
                    <ActionButton
                        disabled=Signal::derive(move || action_busy.get())
                        on_click=move |_| action("/impersonate/exit")
                    >
                        {tr!(ui.exit_impersonation)}
                    </ActionButton>
                </Card>
            }
            .into_any();
        }
        if matches!(
            page,
            AccountPage::Users(_) | AccountPage::CompanySettings | AccountPage::StarterTemplates
        ) && !admin
        {
            return view! {
                <Card class="account-panel">
                    <h1>{tr!(ui.access_denied)}</h1>
                    <p>{tr!(ui.administration_requires_permissions)}</p>
                </Card>
            }
            .into_any();
        }
        match page {
            AccountPage::Users(screen) => users::page(
                api,
                session.with_value(|s| s.user.as_ref().map(|u| u.id).unwrap_or_default()),
                token.get_value(),
                screen,
            )
            .into_any(),
            AccountPage::Password => password_form(api, token.get_value(), false).into_any(),
            AccountPage::Preferences => preferences_page(api, token.get_value()).into_any(),
            AccountPage::CompanySettings => {
                crate::company_settings::page(api, token.get_value()).into_any()
            }
            AccountPage::StarterTemplates => {
                crate::starter_templates::page(api, token.get_value()).into_any()
            }
            AccountPage::Overview => account_overview(api, session.get_value()).into_any(),
        }
    };
    let resource_page = move |family: Family, screen: Screen| {
        if required {
            password_form(api, token.get_value(), true).into_any()
        } else {
            match screen {
                Screen::List => resources::list(api, token.get_value(), family).into_any(),
                Screen::View => resources::detail(api, token.get_value(), family, false).into_any(),
                Screen::Edit => resources::detail(api, token.get_value(), family, true).into_any(),
            }
        }
    };
    let campaign_page = move |list: bool| {
        if required {
            password_form(api, token.get_value(), true).into_any()
        } else if list {
            crate::campaigns::list(api, token.get_value()).into_any()
        } else {
            crate::campaigns::detail(api, token.get_value()).into_any()
        }
    };
    let draft_page = move |editing: bool| {
        if required {
            password_form(api, token.get_value(), true).into_any()
        } else {
            crate::campaign_drafts::page(api, token.get_value(), editing).into_any()
        }
    };
    let integration_page = move |kind: crate::integrations::Kind, list: bool, editing: bool| {
        if required {
            password_form(api, token.get_value(), true).into_any()
        } else if matches!(kind, crate::integrations::Kind::Webhook) && (!admin || active) {
            view! { <p role="alert">{tr!(ui.webhooks_require_administrator_access_outside_impersonation)}</p> }.into_any()
        } else if !(if matches!(kind, crate::integrations::Kind::Webhook) {
            api.features.get().webhooks_enabled
        } else {
            api.features.get().reporting_mailboxes_enabled
        }) {
            crate::company_settings::disabled(admin && !active).into_any()
        } else if list {
            crate::integrations::list(api, token.get_value(), kind).into_any()
        } else {
            crate::integrations::page(api, token.get_value(), kind, editing).into_any()
        }
    };
    let analysis_page = move |directory: bool| {
        if required {
            password_form(api, token.get_value(), true).into_any()
        } else if directory {
            crate::campaigns::analysis_directory(api).into_any()
        } else {
            crate::analysis::page(api).into_any()
        }
    };
    let content = view! {
        <Routes fallback=|| view! { <p>{tr!(ui.this_page_is_not_available)}</p> }>
            <Route
                path=path!("/logout")
                view=move || {
                    view! {
                        <Card class="account-panel">
                            <h1>{tr!(ui.sign_out)}</h1>
                            <p>
                                {tr!(
                                    ui.your_saved_work_will_remain_available_the_next_time_you_sign_in
                                )}
                            </p>
                            <ActionButton
                                style=ButtonStyle::Primary
                                disabled=Signal::derive(move || action_busy.get())
                                on_click=move |_| action("/logout")
                            >
                                {tr!(ui.sign_out_2)}
                            </ActionButton>
                            <a class="cancel-link ghost" href="/">
                                {tr!(ui.stay_signed_in)}
                            </a>
                        </Card>
                    }
                }
            />
            <Route path=path!("/user") view=move || account_page(AccountPage::Overview) />
            <Route
                path=path!("/user/settings")
                view=move || account_page(AccountPage::Preferences)
            />
            <Route
                path=path!("/settings")
                view=move || account_page(AccountPage::CompanySettings)
            />
            <Route
                path=path!("/settings/starter-templates")
                view=move || account_page(AccountPage::StarterTemplates)
            />
            <Route path=path!("/user/password") view=move || account_page(AccountPage::Password) />
            <Route
                path=path!("/users")
                view=move || account_page(AccountPage::Users(users::Screen::List))
            />
            <Route
                path=path!("/users/new")
                view=move || account_page(AccountPage::Users(users::Screen::New))
            />
            <Route
                path=path!("/users/:id/edit")
                view=move || account_page(AccountPage::Users(users::Screen::Edit))
            />
            <Route
                path=path!("/")
                view=move || {
                    if required {
                        password_form(api, token.get_value(), true).into_any()
                    } else {
                        crate::overview::page().into_any()
                    }
                }
            />
            <Route path=path!("/analysis") view=move || analysis_page(true) />
            <Route path=path!("/analysis/:id") view=move || analysis_page(false) />
            <Route
                path=path!("/analysis/:id/recipients/:recipient")
                view=move || analysis_page(false)
            />
            <Route
                path=path!("/webhooks")
                view=move || move || integration_page(
                    crate::integrations::Kind::Webhook,
                    true,
                    false,
                )
            />
            <Route
                path=path!("/webhooks/:id")
                view=move || move || integration_page(
                    crate::integrations::Kind::Webhook,
                    false,
                    false,
                )
            />
            <Route
                path=path!("/webhooks/:id/edit")
                view=move || move || integration_page(
                    crate::integrations::Kind::Webhook,
                    false,
                    true,
                )
            />
            <Route
                path=path!("/reporting-mailboxes")
                view=move || move || integration_page(
                    crate::integrations::Kind::Mailbox,
                    true,
                    false,
                )
            />
            <Route
                path=path!("/reporting-mailboxes/:id")
                view=move || move || integration_page(
                    crate::integrations::Kind::Mailbox,
                    false,
                    false,
                )
            />
            <Route
                path=path!("/reporting-mailboxes/:id/edit")
                view=move || move || integration_page(
                    crate::integrations::Kind::Mailbox,
                    false,
                    true,
                )
            />
            <Route path=path!("/campaigns/drafts/:id") view=move || draft_page(false) />
            <Route path=path!("/campaigns/drafts/:id/edit") view=move || draft_page(true) />
            <Route path=path!("/campaigns") view=move || campaign_page(true) />
            <Route path=path!("/campaigns/:id") view=move || campaign_page(false) />
            <Route
                path=path!("/domains")
                view=move || {
                    if required {
                        password_form(api, token.get_value(), true).into_any()
                    } else {
                        crate::domains::list(api, token.get_value()).into_any()
                    }
                }
            />
            <Route
                path=path!("/domains/:id")
                view=move || {
                    if required {
                        password_form(api, token.get_value(), true).into_any()
                    } else {
                        crate::domains::detail(api, token.get_value(), false).into_any()
                    }
                }
            />
            <Route
                path=path!("/domains/:id/edit")
                view=move || {
                    if required {
                        password_form(api, token.get_value(), true).into_any()
                    } else {
                        crate::domains::detail(api, token.get_value(), true).into_any()
                    }
                }
            />
            <Route path=path!("/groups") view=move || resource_page(Family::Groups, Screen::List) />
            <Route
                path=path!("/groups/:id")
                view=move || resource_page(Family::Groups, Screen::View)
            />
            <Route
                path=path!("/groups/:id/edit")
                view=move || resource_page(Family::Groups, Screen::Edit)
            />
            <Route
                path=path!("/templates")
                view=move || resource_page(Family::Templates, Screen::List)
            />
            <Route
                path=path!("/templates/:id")
                view=move || resource_page(Family::Templates, Screen::View)
            />
            <Route
                path=path!("/templates/:id/edit")
                view=move || resource_page(Family::Templates, Screen::Edit)
            />
            <Route
                path=path!("/learning-pages")
                view=move || resource_page(Family::LearningPages, Screen::List)
            />
            <Route
                path=path!("/learning-pages/:id")
                view=move || resource_page(Family::LearningPages, Screen::View)
            />
            <Route
                path=path!("/learning-pages/:id/edit")
                view=move || resource_page(Family::LearningPages, Screen::Edit)
            />
            <Route path=path!("/pages") view=move || resource_page(Family::Pages, Screen::List) />
            <Route
                path=path!("/pages/:id")
                view=move || resource_page(Family::Pages, Screen::View)
            />
            <Route
                path=path!("/pages/:id/edit")
                view=move || resource_page(Family::Pages, Screen::Edit)
            />
            <Route path=path!("/smtp") view=move || resource_page(Family::Profiles, Screen::List) />
            <Route
                path=path!("/smtp/:id")
                view=move || resource_page(Family::Profiles, Screen::View)
            />
            <Route
                path=path!("/smtp/:id/edit")
                view=move || resource_page(Family::Profiles, Screen::Edit)
            />
            <Route path=path!("/reset_password") view=move || account_page(AccountPage::Overview) />
        </Routes>
    };
    view! {
        <div
            class="admin-shell"
            data-nav-collapsed=move || nav_collapsed.get().then_some("")
            data-nav-open=move || drawer_open.get().then_some("")
        >
            <button
                type="button"
                class="nav-launcher"
                aria-controls="main-navigation"
                aria-expanded=move || if drawer_open.get() { "true" } else { "false" }
                aria-label=trs!(ui.open_navigation)
                on:click=move |_| drawer_open.set(true)
            >
                <svg
                    class="nav-launcher-icon"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                    focusable="false"
                >
                    <path d="M4 7h16 M4 12h16 M4 17h16" />
                </svg>
            </button>
            <aside
                class="admin-nav"
                on:keydown=move |event: leptos::ev::KeyboardEvent| {
                    if event.key() == "Escape" {
                        drawer_open.set(false);
                    }
                }
            >
                <div class="nav-header">
                    <a class="product-name" href="/">
                        <img
                            src="/branding/ruphish-wordmark-white.svg"
                            alt=tr!(ui.ruphish)
                            width="1576"
                            height="512"
                        />
                    </a>
                    <button
                        type="button"
                        class="nav-toggle"
                        aria-controls="main-navigation"
                        aria-expanded=move || if nav_collapsed.get() { "false" } else { "true" }
                        aria-label=move || {
                            if nav_collapsed.get() {
                                expand_label.get()
                            } else {
                                collapse_label.get()
                            }
                        }
                        on:click=move |_| {
                            if viewport_is_narrow() {
                                drawer_open.set(false)
                            } else {
                                nav_collapsed.update(|collapsed| *collapsed = !*collapsed)
                            }
                        }
                    >
                        {move || nav_toggle_icon(!nav_collapsed.get())}
                    </button>
                </div>
                <nav id="main-navigation" aria-label=tr!(ui.main_navigation)>
                    {crate::navigation::sections(path, admin)}
                </nav>
                {account_menu::menu(api, token.get_value(), username, role_name)}
            </aside>
            <div
                class="nav-scrim"
                aria-hidden="true"
                on:click=move |_| drawer_open.set(false)
            ></div>
            <div class="admin-content">
                {active
                    .then(|| {
                        view! {
                            <div class="impersonation" role="status">
                                <span>
                                    {tr!(ui.viewing_as)}<strong>{username_banner}</strong>
                                    {tr!(ui.administrator)}{actor.unwrap_or_default()}
                                </span>
                                <ActionButton
                                    disabled=Signal::derive(move || action_busy.get())
                                    on_click=move |_| action("/impersonate/exit")
                                >
                                    {tr!(ui.exit_impersonation)}
                                </ActionButton>
                            </div>
                        }
                    })} {message.view()}{content}
            </div>
        </div>
    }
}
fn preferences_page(api: Api, token: String) -> impl IntoView {
    view! {
        <section class="account-overview">
            <div class="account-heading">
                <h1>{tr!(ui.settings)}</h1>
            </div>
            <crate::preferences::DisplayPreferencesForm api=api token=token />
        </section>
    }
}

fn password_form(api: Api, token: String, required: bool) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(token);
    let current = RwSignal::new(String::new());
    let candidate = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let notice = use_context::<PasswordNotice>();
    let navigate_account = leptos_router::hooks::use_navigate();
    let cancel_navigation = navigate_account.clone();
    crate::navigation::protect_draft(move || {
        !current.get().is_empty()
            || !candidate.get().is_empty()
            || !confirm.get().is_empty()
            || busy.get()
    });
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        let navigate_account = navigate_account.clone();
        spawn_local(async move {
            let path = if required {
                "/reset_password"
            } else {
                "/internal/password"
            };
            let fields = if required {
                vec![
                    ("password", candidate.get_untracked()),
                    ("confirm_password", confirm.get_untracked()),
                ]
            } else {
                vec![
                    ("current_password", current.get_untracked()),
                    ("new_password", candidate.get_untracked()),
                    ("confirm_new_password", confirm.get_untracked()),
                ]
            };
            let result = api.form(path, &token.get_value(), &fields).await;
            current.set(String::new());
            candidate.set(String::new());
            confirm.set(String::new());
            busy.set(false);
            match result {
                Ok(_) if required => navigate("/"),
                Ok(_) => {
                    if let Some(notice) = notice {
                        notice.0.set(true);
                    }
                    navigate_account(
                        "/user",
                        leptos_router::NavigateOptions {
                            replace: true,
                            ..Default::default()
                        },
                    );
                }
                Err(error) => message.error(error),
            }
        });
    };
    view! {
        <Card class="account-panel password-page">
            {(!required)
                .then(|| {
                    view! { <BackLink href="/user">{tr!(ui.back_to_my_account)}</BackLink> }
                })}
            <h1>
                {move || {
                    if required {
                        leptos_i18n::t_string!(i18n, ui.choose_your_password)
                    } else {
                        leptos_i18n::t_string!(i18n, ui.change_password)
                    }
                }}
            </h1>
            <p>
                {move || {
                    if required {
                        leptos_i18n::t_string!(
                            i18n, ui.replace_your_temporary_password_to_finish_setting_up_your_account
                        )
                    } else {
                        leptos_i18n::t_string!(
                            i18n, ui.use_a_unique_password_changing_it_signs_out_your_other_sessions
                        )
                    }
                }}
            </p> <form class="account-form" on:submit=submit>
                {(!required)
                    .then(|| {
                        view! {
                            <label>
                                {tr!(ui.current_password)}
                                <TextInput
                                    input_type="password"
                                    autocomplete="current-password"
                                    required=true
                                    value=Signal::derive(move || current.get())
                                    on_input=move |e| current.set(event_target_value(&e))
                                />
                            </label>
                        }
                    })}
                <label>
                    {tr!(ui.new_password)}
                    <TextInput
                        input_type="password"
                        autocomplete="new-password"
                        required=true
                        value=Signal::derive(move || candidate.get())
                        on_input=move |e| candidate.set(event_target_value(&e))
                    />
                </label>
                <label>
                    {tr!(ui.confirm_new_password)}
                    <TextInput
                        input_type="password"
                        autocomplete="new-password"
                        required=true
                        value=Signal::derive(move || confirm.get())
                        on_input=move |e| confirm.set(event_target_value(&e))
                    />
                </label>
                {message.view()}
                <ActionButton
                    style=ButtonStyle::Primary
                    kind="submit"
                    disabled=Signal::derive(move || busy.get())
                >
                    {move || {
                        if busy.get() {
                            leptos_i18n::t_string!(i18n, ui.saving)
                        } else {
                            leptos_i18n::t_string!(i18n, ui.save_password)
                        }
                    }}
                </ActionButton>
            </form>
            {(!required)
                .then(|| {
                    view! {
                        <div class="account-actions">
                            <ActionButton
                                style=ButtonStyle::Ghost
                                disabled=Signal::derive(move || busy.get())
                                on_click=move |_| {
                                    current.set(String::new());
                                    candidate.set(String::new());
                                    confirm.set(String::new());
                                    cancel_navigation("/user", Default::default());
                                }
                            >
                                {tr!(ui.cancel)}
                            </ActionButton>
                        </div>
                    }
                })}
        </Card>
    }
}
fn account_overview(api: Api, session: BrowserSession) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(session.csrf_token.clone());
    let id = session.user.as_ref().map(|u| u.id).unwrap_or_default();
    let username = session
        .user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_default();
    let role = StoredValue::new(session.user.as_ref().map(|u| u.role.clone()));
    let role = move || {
        role.with_value(|role| {
            role.as_ref()
                .map(|role| ruphish_ui::localization::role_label(i18n, role))
                .unwrap_or_default()
        })
    };
    let base = StoredValue::new(
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .map(|origin| format!("{origin}/api/"))
            .unwrap_or_default(),
    );
    let key = RwSignal::new(None::<String>);
    let message = Message::new(String::new());
    if let Some(notice) = use_context::<PasswordNotice>() {
        let mut success = false;
        notice.0.update(|value| success = std::mem::take(value));
        if success {
            message.status({
                leptos_i18n::t_string!(
                    i18n,
                    ui.password_changed_your_other_sessions_have_been_signed_out
                )
                .to_owned()
            });
        }
    }
    let confirm = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let reveal = move |_| {
        if busy.get_untracked() {
            return;
        }
        if key.get_untracked().is_some() {
            key.set(None);
            return;
        }
        busy.set(true);
        message.clear();
        spawn_local(async move {
            match load_api_key(api, id).await {
                Ok(value) => key.set(Some(value)),
                Err(error) => message.error(error),
            }
            busy.set(false);
        });
    };
    let copy = move |secret: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        let existing = key.get_untracked();
        spawn_local(async move {
            let value = if secret {
                match existing {
                    Some(value) => Ok(value),
                    None => load_api_key(api, id).await,
                }
            } else {
                Ok(base.get_value())
            };
            match value {
                Ok(value) => match account_menu::copy_text(&value).await {
                    Ok(()) => message.status(if secret { leptos_i18n::t_string!(i18n, ui.api_key_copied) } else { leptos_i18n::t_string!(i18n, ui.api_base_url_copied) }.into()),
                    Err(_) => message.error(if secret {
                        leptos_i18n::t_string!(i18n, ui.clipboard_access_is_unavailable_reveal_the_key_select_it_and_copy_it_manually)
                    } else { leptos_i18n::t_string!(i18n, ui.clipboard_access_is_unavailable_select_the_url_and_copy_it_manually) }.into()),
                },
                Err(error) => message.error(error),
            }
            busy.set(false);
        });
    };
    let rotate = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        spawn_local(async move {
            match api.form("/api/reset", &token.get_value(), &[]).await {
                Ok(value) => {
                    key.set(value["data"].as_str().map(str::to_owned));
                    confirm.set(false);
                    message.status({
                        leptos_i18n::t_string!(
                            i18n,
                            ui.api_key_replaced_update_integrations_that_used_the_previous_key
                        )
                        .to_owned()
                    });
                }
                Err(error) => message.error(error),
            }
            busy.set(false);
        });
    };
    view! {
        <section class="account-overview">
            <div class="account-heading">
                <h1>{tr!(ui.my_account)}</h1>
                <a class="button" href="/user/password">
                    {tr!(ui.change_password)}
                </a>
            </div>
            {message.view()}
            <div class="settings-stack">
                <Card class="account-panel">
                    <h2>{tr!(ui.account)}</h2>
                    <dl class="account-facts">
                        <div>
                            <dt>{tr!(ui.username)}</dt>
                            <dd>{username}</dd>
                        </div>
                        <div>
                            <dt>{tr!(ui.role)}</dt>
                            <dd>{role}</dd>
                        </div>
                    </dl>
                </Card>
                <Card class="account-panel">
                    <h2>{tr!(ui.api_access)}</h2>
                    <p>{tr!(ui.use_your_api_key_for_integrations_treat_it_like_a_password)}</p>
                    <div class="account-api-row">
                        <label>
                            {tr!(ui.api_base_url)}
                            <TextInput
                                readonly=true
                                value=base.get_value()
                                on_focus=Callback::new(move |e: leptos::ev::FocusEvent| {
                                    event_target::<web_sys::HtmlInputElement>(&e).select();
                                })
                            />
                        </label>
                        <ActionButton
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| copy(false)
                        >
                            {tr!(ui.copy_url)}
                        </ActionButton>
                    </div>
                    <div class="account-api-row">
                        <label>
                            {tr!(ui.api_key)}
                            <TextInput
                                id="account-api-key"
                                class="secret-value"
                                readonly=true
                                value=Signal::derive(move || {
                                    key.get().unwrap_or_else(|| "\u{2022}".repeat(16))
                                })
                                on_focus=Callback::new(move |e: leptos::ev::FocusEvent| {
                                    if key.get_untracked().is_some() {
                                        event_target::<web_sys::HtmlInputElement>(&e).select();
                                    }
                                })
                            />
                        </label>
                        <div class="api-key-controls">
                            <button
                                type="button"
                                class="key-visibility"
                                disabled=move || busy.get()
                                on:click=reveal
                                aria-controls="account-api-key"
                                aria-label=tr!(ui.show_api_key)
                                aria-pressed=move || key.get().is_some().to_string()
                                title=move || {
                                    if key.get().is_some() {
                                        leptos_i18n::t_string!(i18n, ui.hide_api_key)
                                    } else {
                                        leptos_i18n::t_string!(i18n, ui.reveal_api_key)
                                    }
                                }
                            >
                                <svg
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="1.8"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    aria-hidden="true"
                                    focusable="false"
                                >
                                    <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" />
                                    <circle cx="12" cy="12" r="3" />
                                    <path
                                        class:hidden-eye=move || key.get().is_some()
                                        d="m3 3 18 18"
                                    />
                                </svg>
                            </button>
                            <ActionButton
                                kind="button"
                                disabled=Signal::derive(move || busy.get())
                                on_click=move |_| copy(true)
                            >
                                {tr!(ui.copy_key)}
                            </ActionButton>
                        </div>
                    </div>
                    <div class="account-actions">
                        <ActionButton
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| confirm.set(true)
                        >
                            {tr!(ui.replace_api_key)}
                        </ActionButton>
                    </div>
                    {move || {
                        confirm
                            .get()
                            .then(|| {
                                view! {
                                    <div class="confirmation">
                                        <p>
                                            {tr!(ui.the_previous_key_will_stop_working_immediately)}
                                        </p>
                                        <ActionButton
                                            style=ButtonStyle::Danger
                                            disabled=Signal::derive(move || busy.get())
                                            on_click=rotate
                                        >
                                            {tr!(ui.replace_key_now)}
                                        </ActionButton>
                                        <ActionButton
                                            style=ButtonStyle::Ghost
                                            disabled=Signal::derive(move || busy.get())
                                            on_click=move |_| confirm.set(false)
                                        >
                                            {tr!(ui.cancel)}
                                        </ActionButton>
                                    </div>
                                }
                            })
                    }}
                </Card>
            </div>
        </section>
    }
}

/// Only retrieve the secret on an explicit reveal/copy action.
async fn load_api_key(api: Api, id: i64) -> Result<String, String> {
    let i18n = api.i18n;

    let value = api.get(&format!("/api/users/{id}")).await?;
    value["api_key"]
        .as_str()
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            leptos_i18n::t_string!(i18n, ui.the_server_did_not_return_an_api_key).to_owned()
        })
}

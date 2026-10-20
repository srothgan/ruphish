//! Account workflows over the same HTTP boundary used by external clients.
use crate::feedback::Message;
use crate::shared::components::*;
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local_scoped_with_cancellation as spawn_local;
use ruphish_contracts::auth::{BrowserSession, UserEdit};
use ruphish_ui::session::{SessionAction, SessionMonitor};
use serde_json::Value;

#[path = "account_menu.rs"]
mod account_menu;
pub(super) use account_menu::copy_text;
#[path = "users.rs"]
mod users;

#[derive(Clone, Copy)]
struct PasswordNotice(RwSignal<Option<String>>);

#[derive(Clone, Copy)]
pub(super) struct Api {
    monitor: RwSignal<SessionMonitor>,
}
impl Api {
    pub(super) async fn launch(
        self,
        token: &str,
        key: &str,
        value: &Value,
    ) -> Result<Value, (u16, String)> {
        if !self.active() {
            return Err((401, "Your session ended.".into()));
        }
        let request =
            ruphish_ui::browser_http::launch_request("/api/campaigns/", token, key, value)
                .map_err(|e| (0, e))?;
        let response=self.send_request("/api/campaigns/", request).await.map_err(|_|(0,"Launch outcome is not known. Retry uses the same launch key; do not create another campaign.".into()))?;
        let status = response.status;
        self.response("/api/campaigns/", response)
            .await
            .map_err(|e| (status, e))
    }
    pub(super) async fn resource(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&Value>,
        revision: Option<&str>,
    ) -> Result<(Value, Option<String>), String> {
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
        }
        let request =
            ruphish_ui::browser_http::resource_request(method, path, token, value, revision)?;
        let response = self.send_request(path, request).await?;
        let etag = response.etag.clone();
        self.response(path, response)
            .await
            .map(|value| (value, etag))
    }
    pub(super) async fn download(self, path: &str, filename: &str) -> Result<(), String> {
        if !self.active() {
            return Err("Your session ended.".into());
        }
        let request = ruphish_ui::browser_http::resource_request("GET", path, "", None, None)?;
        let observed_path = path.to_owned();
        let (response, file) = ruphish_ui::browser_http::download(request, move |status| {
            self.monitor
                .update(|monitor| monitor.response(&observed_path, status));
        })
        .await?;
        if !self.active() {
            return Err("Your session ended.".into());
        }
        if !(200..300).contains(&response.status) {
            self.response(path, response).await?;
            return Err("The export could not be generated.".into());
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
        let status = response.status;
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
        }
        let text = response.body;
        let decoded = ruphish_ui::browser_http::decode(status, &text);
        let value = if (200..300).contains(&status) {
            decoded?
        } else {
            decoded.unwrap_or(Value::Null)
        };
        if !(200..300).contains(&status) {
            return Err(value["message"].as_str().map(str::to_owned).unwrap_or_else(
                || match status {
                    401 => "Your session ended. Sign in again to continue.".into(),
                    429 => "Too many requests. Wait a moment and try again.".into(),
                    503 => "The server is temporarily unavailable. Try again shortly.".into(),
                    _ => format!("The request failed ({status}). Try again."),
                },
            ));
        }
        Ok(value)
    }
    pub(super) async fn get(self, path: &str) -> Result<Value, String> {
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
        }
        let request = Request::get(path)
            .header("x-ruphish-client", "browser")
            .header("accept", "application/json")
            .build()
            .map_err(|_| "Cannot prepare request".to_owned())?;
        self.response(path, self.send_request(path, request).await?)
            .await
    }

    async fn form(
        self,
        path: &str,
        token: &str,
        fields: &[(&str, String)],
    ) -> Result<Value, String> {
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
        }
        let body = serde_urlencoded::to_string(fields)
            .map_err(|_| "Cannot prepare the request".to_owned())?;
        let request = Request::post(path)
            .header("x-ruphish-client", "browser")
            .header("accept", "application/json")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("x-csrf-token", token)
            .body(body)
            .map_err(|_| "Cannot prepare the request".to_owned())?;
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
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
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
        .map_err(|_| "Cannot prepare the request".to_owned())?;
        self.response(path, self.send_request(path, request).await?)
            .await
    }
}

pub(super) fn app() -> impl IntoView {
    let session = RwSignal::new(None::<Result<BrowserSession, String>>);
    let api = Api {
        monitor: RwSignal::new(SessionMonitor::default()),
    };
    spawn_local(async move {
        let loaded = api.get("/auth/session").await.and_then(|v| {
            serde_json::from_value::<BrowserSession>(v)
                .map_err(|_| "Invalid session response".into())
        });
        if let Ok(value) = &loaded {
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
        // Global browser callbacks do not establish a Leptos owner themselves.
        owner.with_value(|owner| {
            if let Some(owner) = owner {
                owner.with(|| {
                    spawn_local(async move {
                        if let Ok(value) = api.get("/auth/session").await.and_then(|v| {
                            serde_json::from_value::<BrowserSession>(v)
                                .map_err(|_| "Invalid session response".into())
                        }) {
                            api.monitor.update(|m| m.observe(&value));
                        }
                        checking.set(false);
                    })
                });
            }
        });
    };
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
    view! { <div class="account-app">{navigation_guard}{move || if action.get() != SessionAction::Continue {
        view! { <main class="login-screen"><p role="status">"Refreshing your session…"</p><a rel="external" href="/login">"Sign in again"</a></main> }.into_any()
    } else { match session.get() {
        None => view! { <main class="login-screen"><p role="status">"Opening Ruphish…"</p></main> }.into_any(),
        Some(Err(error)) => view! { <main class="login-screen"><h1>"Ruphish"</h1><p class="form-message error" role="alert">{error}</p><a rel="external" href="/login">"Try signing in again"</a></main> }.into_any(),
        Some(Ok(session)) if session.user.is_none() => login(api, session.csrf_token).into_any(),
        Some(Ok(session)) => shell(api, session).into_any(),
    } }}</div> }
}
fn login(api: Api, token: String) -> impl IntoView {
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
        "Your session ended. Sign in again to continue.".to_owned()
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
    view! { <main class="login-screen"><div class="login-brand">"Ruphish"</div><section class="login-form"><h1>"Sign in"</h1><p>"Manage your security awareness workspace."</p>
    <form on:submit=submit><label>"Username"<TextInput name="username" autocomplete="username" required=true value=Signal::derive(move || username.get()) on_input=move |e| username.set(event_target_value(&e))/></label>
    <label>"Password"<TextInput name="password" input_type="password" autocomplete="current-password" required=true value=Signal::derive(move || password.get()) on_input=move |e| password.set(event_target_value(&e))/></label>
    {message.view()}<ActionButton style=ButtonStyle::Primary kind="submit" disabled=Signal::derive(move || busy.get())>{move || if busy.get() { "Signing in…" } else { "Sign in" }}</ActionButton></form></section></main> }
}
fn shell(api: Api, session: BrowserSession) -> impl IntoView {
    provide_context(ruphish_ui::import_drafts::ImportDrafts::default());
    provide_context(PasswordNotice(RwSignal::new(None)));
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
    let role_name = user
        .as_ref()
        .map(|u| u.role.name.clone())
        .unwrap_or_default();
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
    let session = StoredValue::new(session);
    let account_page = move |users_page: Option<users::Screen>, password_page: bool| {
        if required {
            password_form(api, token.get_value(), true).into_any()
        } else if active {
            view! { <Card class="account-panel"><h1>"Account management"</h1><p>"Exit impersonation to manage accounts and credentials."</p><ActionButton disabled=Signal::derive(move||action_busy.get()) on_click=move |_| action("/impersonate/exit")>"Exit impersonation"</ActionButton></Card> }.into_any()
        } else if let Some(screen) = users_page {
            if admin {
                users::page(
                    api,
                    session.with_value(|s| s.user.as_ref().map(|u| u.id).unwrap_or_default()),
                    token.get_value(),
                    screen,
                )
                .into_any()
            } else {
                view!{<Card class="account-panel"><h1>"Access denied"</h1><p>"User management requires administrator permissions."</p></Card>}.into_any()
            }
        } else if password_page {
            password_form(api, token.get_value(), false).into_any()
        } else {
            settings(api, session.get_value()).into_any()
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
            crate::campaigns::list(api).into_any()
        } else {
            crate::campaigns::detail(api, token.get_value()).into_any()
        }
    };
    let integration_page = move |kind: crate::integrations::Kind, list: bool, editing: bool| {
        if required {
            password_form(api, token.get_value(), true).into_any()
        } else if matches!(kind, crate::integrations::Kind::Webhook) && (!admin || active) {
            view! { <p role="alert">"Webhooks require administrator access outside impersonation."</p> }.into_any()
        } else if list {
            crate::integrations::list(api, kind).into_any()
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
    let content = view! {<Routes fallback=||view!{<p>"This page is not available."</p>}>
    <Route path=path!("/logout") view=move||view! { <Card class="account-panel"><h1>"Sign out?"</h1><p>"Your saved work will remain available the next time you sign in."</p><ActionButton style=ButtonStyle::Primary disabled=Signal::derive(move||action_busy.get()) on_click=move |_| action("/logout")>"Sign out"</ActionButton><a class="cancel-link ghost" href="/">"Stay signed in"</a></Card> }/>
    <Route path=path!("/settings") view=move||account_page(None,false)/>
    <Route path=path!("/settings/password") view=move||account_page(None,true)/>
    <Route path=path!("/users") view=move||account_page(Some(users::Screen::List),false)/>
    <Route path=path!("/users/new") view=move||account_page(Some(users::Screen::New),false)/>
    <Route path=path!("/users/:id/edit") view=move||account_page(Some(users::Screen::Edit),false)/>
    <Route path=path!("/") view=move||campaign_page(true)/>
    <Route path=path!("/analysis") view=move || analysis_page(true)/>
    <Route path=path!("/analysis/:id") view=move || analysis_page(false)/>
    <Route path=path!("/analysis/:id/recipients/:recipient") view=move || analysis_page(false)/>
    <Route path=path!("/webhooks") view=move || integration_page(crate::integrations::Kind::Webhook, true, false)/>
    <Route path=path!("/webhooks/:id") view=move || integration_page(crate::integrations::Kind::Webhook, false, false)/>
    <Route path=path!("/webhooks/:id/edit") view=move || integration_page(crate::integrations::Kind::Webhook, false, true)/>
    <Route path=path!("/reporting-mailboxes") view=move || integration_page(crate::integrations::Kind::Mailbox, true, false)/>
    <Route path=path!("/reporting-mailboxes/:id") view=move || integration_page(crate::integrations::Kind::Mailbox, false, false)/>
    <Route path=path!("/reporting-mailboxes/:id/edit") view=move || integration_page(crate::integrations::Kind::Mailbox, false, true)/>
    <Route path=path!("/campaigns") view=move||campaign_page(true)/>
    <Route path=path!("/campaigns/:id") view=move||campaign_page(false)/>
    <Route path=path!("/domains") view=move||if required{password_form(api,token.get_value(),true).into_any()}else{crate::domains::list(api).into_any()}/>
    <Route path=path!("/domains/:id") view=move||if required{password_form(api,token.get_value(),true).into_any()}else{crate::domains::detail(api,token.get_value(),false).into_any()}/>
    <Route path=path!("/domains/:id/edit") view=move||if required{password_form(api,token.get_value(),true).into_any()}else{crate::domains::detail(api,token.get_value(),true).into_any()}/>
    <Route path=path!("/groups") view=move||resource_page(Family::Groups,Screen::List)/>
    <Route path=path!("/groups/:id") view=move||resource_page(Family::Groups,Screen::View)/>
    <Route path=path!("/groups/:id/edit") view=move||resource_page(Family::Groups,Screen::Edit)/>
    <Route path=path!("/templates") view=move||resource_page(Family::Templates,Screen::List)/>
    <Route path=path!("/templates/:id") view=move||resource_page(Family::Templates,Screen::View)/>
    <Route path=path!("/templates/:id/edit") view=move||resource_page(Family::Templates,Screen::Edit)/>
    <Route path=path!("/pages") view=move||resource_page(Family::Pages,Screen::List)/>
    <Route path=path!("/pages/:id") view=move||resource_page(Family::Pages,Screen::View)/>
    <Route path=path!("/pages/:id/edit") view=move||resource_page(Family::Pages,Screen::Edit)/>
    <Route path=path!("/smtp") view=move||resource_page(Family::Profiles,Screen::List)/>
    <Route path=path!("/smtp/:id") view=move||resource_page(Family::Profiles,Screen::View)/>
    <Route path=path!("/smtp/:id/edit") view=move||resource_page(Family::Profiles,Screen::Edit)/>
        <Route path=path!("/reset_password") view=move||account_page(None,false)/>
    </Routes>};
    view! { <div class="admin-shell"><aside class="admin-nav"><a class="product-name" href="/">"Ruphish"</a><nav aria-label="Main navigation">
    {crate::navigation::sections(path,admin)}
    </nav>{account_menu::menu(api,token.get_value(),username,role_name)}</aside>
    <div class="admin-content">{active.then(|| view! { <div class="impersonation" role="status"><span>"Viewing as "<strong>{username_banner}</strong>". Administrator: "{actor.unwrap_or_default()}</span><ActionButton disabled=Signal::derive(move||action_busy.get()) on_click=move |_| action("/impersonate/exit")>"Exit impersonation"</ActionButton></div> })}
    {message.view()}{content}</div></div> }
}
fn password_form(api: Api, token: String, required: bool) -> impl IntoView {
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
                "/settings"
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
                        notice.0.set(Some(
                            "Password changed. Your other sessions have been signed out.".into(),
                        ));
                    }
                    navigate_account(
                        "/settings",
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
    view! { <Card class="account-panel password-page">{(!required).then(||view!{<BackLink href="/settings">"Back to my account"</BackLink>})}<h1>{if required { "Choose your password" } else { "Change password" }}</h1>
    <p>{if required { "Replace your temporary password to finish setting up your account." } else { "Use a unique password. Changing it signs out your other sessions." }}</p>
    <form class="account-form" on:submit=submit>
    {(!required).then(|| view! { <label>"Current password"<TextInput input_type="password" autocomplete="current-password" required=true value=Signal::derive(move || current.get()) on_input=move |e| current.set(event_target_value(&e))/></label> })}
    <label>"New password"<TextInput input_type="password" autocomplete="new-password" required=true value=Signal::derive(move || candidate.get()) on_input=move |e| candidate.set(event_target_value(&e))/></label>
    <label>"Confirm new password"<TextInput input_type="password" autocomplete="new-password" required=true value=Signal::derive(move || confirm.get()) on_input=move |e| confirm.set(event_target_value(&e))/></label>
    {message.view()}<ActionButton style=ButtonStyle::Primary kind="submit" disabled=Signal::derive(move || busy.get())>{move || if busy.get() { "Saving…" } else { "Save password" }}</ActionButton></form>{(!required).then(||view!{<div class="account-actions"><ActionButton style=ButtonStyle::Ghost disabled=Signal::derive(move||busy.get()) on_click=move |_|{current.set(String::new());candidate.set(String::new());confirm.set(String::new());cancel_navigation("/settings",Default::default());}>"Cancel"</ActionButton></div>})}</Card> }
}
fn settings(api: Api, session: BrowserSession) -> impl IntoView {
    let token = StoredValue::new(session.csrf_token.clone());
    let id = session.user.as_ref().map(|u| u.id).unwrap_or_default();
    let username = session
        .user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_default();
    let role = session
        .user
        .as_ref()
        .map(|u| u.role.name.clone())
        .unwrap_or_default();
    let base = StoredValue::new(
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .map(|origin| format!("{origin}/api/"))
            .unwrap_or_default(),
    );
    let key = RwSignal::new(None::<String>);
    let message = Message::new(String::new());
    if let Some(notice) = use_context::<PasswordNotice>() {
        let mut success = None;
        notice.0.update(|value| success = value.take());
        if let Some(success) = success {
            message.status(success);
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
                    Ok(()) => message.status(if secret { "API key copied." } else { "API base URL copied." }.into()),
                    Err(_) => message.error(if secret {
                        "Clipboard access is unavailable. Reveal the key, select it and copy it manually."
                    } else { "Clipboard access is unavailable. Select the URL and copy it manually." }.into()),
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
                    message.status(
                        "API key replaced. Update integrations that used the previous key.".into(),
                    );
                }
                Err(error) => message.error(error),
            }
            busy.set(false);
        });
    };
    view! {<section class="account-overview"><div class="account-heading"><h1>"My account"</h1><a class="button" href="/settings/password">"Change password"</a></div>
    {message.view()}<div class="settings-stack"><Card class="account-panel"><h2>"Account"</h2><dl class="account-facts"><div><dt>"Username"</dt><dd>{username}</dd></div><div><dt>"Role"</dt><dd>{role}</dd></div></dl></Card>
    <Card class="account-panel"><h2>"API access"</h2><p>"Use your API key for integrations. Treat it like a password."</p>
    <div class="account-api-row"><label>"API base URL"<TextInput readonly=true value=base.get_value() on_focus=Callback::new(move|e: leptos::ev::FocusEvent|{event_target::<web_sys::HtmlInputElement>(&e).select();})/></label><ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|copy(false)>"Copy URL"</ActionButton></div>
    <div class="account-api-row"><label>"API key"<TextInput id="account-api-key" class="secret-value" readonly=true value=Signal::derive(move||key.get().unwrap_or_else(||"\u{2022}".repeat(16))) on_focus=Callback::new(move|e: leptos::ev::FocusEvent|{if key.get_untracked().is_some(){event_target::<web_sys::HtmlInputElement>(&e).select();}})/></label>
        <div class="api-key-controls">
            <button type="button" class="key-visibility" disabled=move||busy.get() on:click=reveal aria-controls="account-api-key" aria-label="Show API key" aria-pressed=move||key.get().is_some().to_string() title=move||if key.get().is_some(){"Hide API key"}else{"Reveal API key"}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
                    <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"/>
                    <circle cx="12" cy="12" r="3"/>
                    <path class:hidden-eye=move||key.get().is_some() d="m3 3 18 18"/>
                </svg>
            </button>
            <ActionButton kind="button" disabled=Signal::derive(move||busy.get()) on_click=move |_|copy(true)>"Copy key"</ActionButton>
        </div>
    </div>
    <div class="account-actions"><ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|confirm.set(true)>"Replace API key…"</ActionButton></div>
    {move||confirm.get().then(||view!{<div class="confirmation"><p>"The previous key will stop working immediately."</p><ActionButton style=ButtonStyle::Danger disabled=Signal::derive(move||busy.get()) on_click=rotate>"Replace key now"</ActionButton><ActionButton style=ButtonStyle::Ghost disabled=Signal::derive(move||busy.get()) on_click=move |_|confirm.set(false)>"Cancel"</ActionButton></div>})}
    </Card></div></section>}
}

/// Only retrieve the secret on an explicit reveal/copy action.
async fn load_api_key(api: Api, id: i64) -> Result<String, String> {
    let value = api.get(&format!("/api/users/{id}")).await?;
    value["api_key"]
        .as_str()
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "The server did not return an API key.".to_owned())
}

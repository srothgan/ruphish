//! Account workflows over the same HTTP boundary used by external clients.
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local_scoped_with_cancellation as spawn_local;
use ruphish_contracts::auth::{BrowserSession, User, UserEdit};
use ruphish_ui::session::{SessionAction, SessionMonitor};
use serde_json::Value;

#[derive(Clone, Copy)]
struct Api {
    monitor: RwSignal<SessionMonitor>,
}
impl Api {
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
    async fn response(
        self,
        path: &str,
        response: gloo_net::http::Response,
    ) -> Result<Value, String> {
        let status = response.status();
        self.monitor.update(|m| m.response(path, status));
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
        }
        let text = response
            .text()
            .await
            .map_err(|_| "Could not read the server response".to_owned())?;
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
        }
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if !(200..300).contains(&status) {
            return Err(value["message"].as_str().map(str::to_owned).unwrap_or_else(
                || match status {
                    401 => "Your session ended. Sign in again to continue.".into(),
                    429 => "Too many sign-in attempts. Wait a moment and try again.".into(),
                    503 => "Authentication is busy. Try again shortly.".into(),
                    _ => format!("The request failed ({status}). Try again."),
                },
            ));
        }
        Ok(value)
    }
    async fn get(self, path: &str) -> Result<Value, String> {
        if !self.active() {
            return Err("Your session ended. Sign in again to continue.".into());
        }
        self.response(
            path,
            Request::get(path)
                .header("x-ruphish-client", "browser")
                .header("accept", "application/json")
                .send()
                .await
                .map_err(|_| "Cannot reach the server".to_owned())?,
        )
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
        self.response(
            path,
            request
                .send()
                .await
                .map_err(|_| "Cannot reach the server".to_owned())?,
        )
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
            builder.body("")
        }
        .map_err(|_| "Cannot prepare the request".to_owned())?;
        self.response(
            path,
            request
                .send()
                .await
                .map_err(|_| "Cannot reach the server".to_owned())?,
        )
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
    // Only navigation changes remount the view; routine responses and focus checks
    // must preserve unsaved forms and editor content.
    let action = Memo::new(move |_| api.monitor.with(|m| m.action()));
    Effect::new(move |_| {
        let action = action.get();
        if let Some(window) = web_sys::window() {
            match action {
                SessionAction::Continue => {}
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
    view! { <div class="account-app">{move || if action.get() != SessionAction::Continue {
        view! { <main class="login-screen"><p role="status">"Refreshing your session..."</p><a href="/login">"Sign in again"</a></main> }.into_any()
    } else { match session.get() {
        None => view! { <main class="login-screen"><p role="status">"Opening Ruphish..."</p></main> }.into_any(),
        Some(Err(error)) => view! { <main class="login-screen"><h1>"Ruphish"</h1><p role="alert">{error}</p><a href="/login">"Try signing in again"</a></main> }.into_any(),
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
    let message = RwSignal::new(if expired {
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
        message.set(String::new());
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
                Err(error) => message.set(error),
            }
        });
    };
    view! { <main class="login-screen"><div class="login-brand">"Ruphish"</div><section class="login-form"><h1>"Sign in"</h1><p>"Manage your security awareness workspace."</p>
    <form on:submit=submit><label>"Username"<input name="username" autocomplete="username" required prop:value=move || username.get() on:input=move |e| username.set(event_target_value(&e))/></label>
    <label>"Password"<input name="password" type="password" autocomplete="current-password" required prop:value=move || password.get() on:input=move |e| password.set(event_target_value(&e))/></label>
    <p class="form-message" role="alert">{move || message.get()}</p><button class="primary" type="submit" disabled=move || busy.get()>{move || if busy.get() { "Signing in..." } else { "Sign in" }}</button></form></section></main> }
}
fn shell(api: Api, session: BrowserSession) -> impl IntoView {
    let user = session.user.clone();
    let username = user
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_default();
    let admin = user
        .as_ref()
        .is_some_and(|u| u.permissions.iter().any(|p| p == "modify_system"));
    let required = user.as_ref().is_some_and(|u| u.password_change_required);
    let token = StoredValue::new(session.csrf_token.clone());
    let message = RwSignal::new(String::new());
    let actor = session.actor.clone();
    let active = actor.is_some();
    let username_banner = username.clone();
    let action = move |path: &'static str| {
        spawn_local(async move {
            match api.form(path, &token.get_value(), &[]).await {
                Ok(value) => navigate(value["data"].as_str().unwrap_or("/login")),
                Err(error) => message.set(error),
            }
        });
    };
    let path = location();
    let content = if path == "/logout" {
        view! { <section class="account-panel"><h1>"Sign out?"</h1><p>"Your saved work will remain available the next time you sign in."</p><button class="primary" on:click=move |_| action("/logout")>"Sign out"</button><a class="cancel-link" href="/">"Stay signed in"</a></section> }.into_any()
    } else if required {
        password_form(api, session.csrf_token.clone(), true).into_any()
    } else if active && matches!(path.as_str(), "/settings" | "/users") {
        view! { <section class="account-panel"><h1>"Account management"</h1><p>"Exit impersonation to manage accounts and credentials."</p><button on:click=move |_| action("/impersonate/exit")>"Exit impersonation"</button></section> }.into_any()
    } else if path == "/settings" {
        settings(api, session.clone()).into_any()
    } else if path == "/users" && admin {
        users(
            api,
            user.as_ref().map(|u| u.id).unwrap_or_default(),
            session.csrf_token.clone(),
        )
        .into_any()
    } else {
        crate::editor::editor_workbench().into_any()
    };
    view! { <div class="admin-shell"><aside class="admin-nav"><a class="product-name" href="/">"Ruphish"</a><nav aria-label="Administration">
    <a href="/" aria-current=if path == "/" { Some("page") } else { None }>"Content editor"</a>
    <a href="/settings" aria-current=if path == "/settings" { Some("page") } else { None }>"My account"</a>
    {admin.then(|| view! { <a href="/users" aria-current=if path == "/users" { Some("page") } else { None }>"Users"</a> })}
    </nav><div class="signed-in"><span>"Signed in as"</span><strong>{username}</strong><a href="/logout">"Sign out"</a></div></aside>
    <div class="admin-content">{active.then(|| view! { <div class="impersonation" role="status"><span>"Viewing as "<strong>{username_banner}</strong>". Administrator: "{actor.unwrap_or_default()}</span><button on:click=move |_| action("/impersonate/exit")>"Exit impersonation"</button></div> })}
    <p class="form-message" role="alert">{move || message.get()}</p>{content}</div></div> }
}
fn password_form(api: Api, token: String, required: bool) -> impl IntoView {
    let token = StoredValue::new(token);
    let current = RwSignal::new(String::new());
    let candidate = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let message = RwSignal::new(String::new());
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.set(String::new());
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
                Ok(_) => message
                    .set("Password changed. Your other sessions have been signed out.".into()),
                Err(error) => message.set(error),
            }
        });
    };
    view! { <section class="account-panel"><h1>{if required { "Choose your password" } else { "Change password" }}</h1>
    <p>{if required { "Replace your temporary password to finish setting up your account." } else { "Use a unique password. Changing it signs out your other sessions." }}</p>
    <form class="account-form" on:submit=submit>
    {(!required).then(|| view! { <label>"Current password"<input type="password" autocomplete="current-password" required prop:value=move || current.get() on:input=move |e| current.set(event_target_value(&e))/></label> })}
    <label>"New password"<input type="password" autocomplete="new-password" required prop:value=move || candidate.get() on:input=move |e| candidate.set(event_target_value(&e))/></label>
    <label>"Confirm new password"<input type="password" autocomplete="new-password" required prop:value=move || confirm.get() on:input=move |e| confirm.set(event_target_value(&e))/></label>
    <p class="form-message" role="alert">{move || message.get()}</p><button class="primary" type="submit" disabled=move || busy.get()>{move || if busy.get() { "Saving..." } else { "Save password" }}</button></form></section> }
}
fn settings(api: Api, session: BrowserSession) -> impl IntoView {
    let token = StoredValue::new(session.csrf_token.clone());
    let id = session.user.as_ref().map(|u| u.id).unwrap_or_default();
    let key = RwSignal::new(None::<String>);
    let message = RwSignal::new(String::new());
    let confirm = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let reveal = move |_| {
        spawn_local(async move {
            match api.get(&format!("/api/users/{id}")).await {
                Ok(value) => key.set(value["api_key"].as_str().map(str::to_owned)),
                Err(error) => message.set(error),
            }
        });
    };
    let rotate = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        spawn_local(async move {
            match api.form("/api/reset", &token.get_value(), &[]).await {
                Ok(value) => {
                    key.set(value["data"].as_str().map(str::to_owned));
                    confirm.set(false);
                    message.set(
                        "API key replaced. Update integrations that used the previous key.".into(),
                    );
                }
                Err(error) => message.set(error),
            }
            busy.set(false);
        });
    };
    view! { <div class="settings-stack">{password_form(api, session.csrf_token, false)}<section class="account-panel"><h2>"API access"</h2><p>"Use your API key for integrations. Treat it like a password."</p>
    {move || key.get().map(|value| view! { <label>"API key"<input class="secret-value" readonly value=value on:focus=move |event| { event_target::<web_sys::HtmlInputElement>(&event).select(); }/></label> })}
    <div class="account-actions"><button on:click=reveal>"Reveal API key"</button><button on:click=move |_| key.set(None)>"Hide key"</button><button on:click=move |_| confirm.set(true)>"Replace API key..."</button></div>
    {move || confirm.get().then(|| view! { <div class="confirmation"><p>"The previous key will stop working immediately."</p><button class="danger" disabled=move || busy.get() on:click=rotate>"Replace key now"</button><button on:click=move |_| confirm.set(false)>"Cancel"</button></div> })}
    <p class="form-message" role="status">{move || message.get()}</p></section></div> }
}
fn users(api: Api, current_user: i64, token: String) -> impl IntoView {
    let token = StoredValue::new(token);
    let list = RwSignal::new(Vec::<User>::new());
    let message = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    let editing = RwSignal::new(None::<i64>);
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let role = RwSignal::new("user".to_owned());
    let locked = RwSignal::new(false);
    let reset = RwSignal::new(true);
    let deleting = RwSignal::new(None::<(i64, String)>);
    let refresh = move || {
        spawn_local(async move {
            match api.get("/api/users/").await.and_then(|v| {
                serde_json::from_value::<Vec<User>>(v).map_err(|_| "Invalid user response".into())
            }) {
                Ok(users) => {
                    list.set(users);
                    loaded.set(true);
                }
                Err(error) => message.set(error),
            }
        });
    };
    refresh();
    let clear = move || {
        editing.set(None);
        username.set(String::new());
        password.set(String::new());
        role.set("user".into());
        locked.set(false);
        reset.set(true);
    };
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.set(String::new());
        spawn_local(async move {
            let value = UserEdit {
                username: username.get_untracked(),
                password: password.get_untracked(),
                role: role.get_untracked(),
                password_change_required: reset.get_untracked(),
                account_locked: locked.get_untracked(),
            };
            let id = editing.get_untracked();
            let path = id
                .map(|id| format!("/api/users/{id}"))
                .unwrap_or_else(|| "/api/users/".into());
            let result = api
                .json(
                    if id.is_some() { "PUT" } else { "POST" },
                    &path,
                    &token.get_value(),
                    Some(&value),
                )
                .await;
            password.set(String::new());
            busy.set(false);
            match result {
                Ok(_) => {
                    clear();
                    refresh();
                    message.set("User saved.".into());
                }
                Err(error) => message.set(error),
            }
        });
    };
    let remove = move |_| {
        if busy.get_untracked() {
            return;
        }
        if let Some((id, _)) = deleting.get_untracked() {
            busy.set(true);
            spawn_local(async move {
                match api
                    .json(
                        "DELETE",
                        &format!("/api/users/{id}"),
                        &token.get_value(),
                        None,
                    )
                    .await
                {
                    Ok(_) => {
                        deleting.set(None);
                        clear();
                        refresh();
                        message.set("User and owned data deleted.".into());
                    }
                    Err(error) => message.set(error),
                }
                busy.set(false);
            });
        }
    };
    view! { <section class="users-page"><div class="account-heading"><h1>"Users"</h1><button on:click=move |_| { clear(); message.set(String::new()); }>"New user"</button></div><p>"Manage access, roles and account status."</p>
        <p class="form-message" role="alert">{move || message.get()}</p><div class="users-layout"><div class="user-table-wrap"><table class="user-table"><thead><tr><th scope="col">"User"</th><th scope="col">"Role"</th><th scope="col">"Status"</th><th scope="col">"Actions"</th></tr></thead><tbody>
        {move || list.get().into_iter().map(|user| { let id = user.id; let edit_user = user.clone(); let delete_name = user.username.clone(); let impersonate_name = user.username.clone(); let own_account = id == current_user; let can_impersonate = !user.account_locked && !user.password_change_required;
            view! { <tr><th scope="row">{user.username}</th><td>{user.role.name}</td><td>{if user.account_locked { "Locked" } else if user.password_change_required { "Password setup" } else { "Active" }}</td><td><div class="row-actions">
                <button on:click=move |_| { editing.set(Some(id)); username.set(edit_user.username.clone()); role.set(edit_user.role.slug.clone()); password.set(String::new()); locked.set(edit_user.account_locked); reset.set(edit_user.password_change_required); }>"Edit"</button>
                <button disabled=!can_impersonate on:click=move |_| { let name = impersonate_name.clone(); spawn_local(async move { match api.form("/impersonate", &token.get_value(), &[("username",name)]).await { Ok(_) => navigate("/"), Err(error) => message.set(error) } }); }>"View as"</button>
                <button class="text-danger" disabled=own_account title=if own_account { "You cannot delete your own account" } else { "Delete this user" } on:click=move |_| deleting.set(Some((id,delete_name.clone())))>"Delete..."</button></div></td></tr> }
        }).collect_view()}
        </tbody></table>{move || (!loaded.get()).then(|| view! { <p role="status">"Loading users..."</p> })}</div>
        <form class="account-panel user-edit account-form" on:submit=submit><h2>{move || if editing.get().is_some() { "Edit user" } else { "New user" }}</h2>
        <label>"Username"<input autocomplete="off" required prop:value=move || username.get() on:input=move |e| username.set(event_target_value(&e))/></label>
        <label>{move || if editing.get().is_some() { "New password (leave empty to keep current)" } else { "Temporary password" }}<input type="password" autocomplete="new-password" required=move || editing.get().is_none() prop:value=move || password.get() on:input=move |e| password.set(event_target_value(&e))/></label>
        <label>"Role"<select prop:value=move || role.get() on:change=move |e| role.set(event_target_value(&e))><option value="user">"User"</option><option value="admin">"Administrator"</option>{move || { let slug = role.get(); (!matches!(slug.as_str(), "user" | "admin")).then(|| view! { <option value=slug.clone()>{slug.clone()}</option> }) }}</select></label>
        <label class="checkbox-label"><input type="checkbox" prop:checked=move || reset.get() on:change=move |e| reset.set(event_target_checked(&e))/>"Require password change"</label>
        <label class="checkbox-label"><input type="checkbox" prop:checked=move || locked.get() on:change=move |e| locked.set(event_target_checked(&e))/>"Lock account"</label>
        <div class="account-actions"><button class="primary" disabled=move || busy.get() type="submit">{move || if busy.get() { "Saving..." } else { "Save user" }}</button><button type="button" on:click=move |_| clear()>"Clear"</button></div></form></div>
        {move || deleting.get().map(|(_,name)| view! { <div class="confirmation" role="alertdialog" aria-label="Confirm user deletion"><h2>"Delete "{name}"?"</h2><p>"This permanently removes the account and its owned campaigns, templates and other data."</p><div class="account-actions"><button class="danger" disabled=move || busy.get() on:click=remove>"Delete user and data"</button><button on:click=move |_| deleting.set(None)>"Cancel"</button></div></div> })}
    </section> }
}

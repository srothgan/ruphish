//! Separate user list and form routes; account APIs remain the authorization boundary.
use super::{Api, navigate};
use crate::components::{
    DataTable, DeleteConfirmation, EmptyState, Field, FormFields, FormFooter, ManagementPage,
    PageHeader, SaveButton,
};
use crate::feedback::Message;
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_contracts::auth::{User, UserEdit};

#[derive(Clone, Copy)]
pub(super) enum Screen {
    List,
    New,
    Edit,
}
#[derive(Clone, Copy)]
pub(super) struct Notice(pub RwSignal<Option<String>>);

pub(super) fn page(api: Api, current_user: i64, token: String, screen: Screen) -> impl IntoView {
    match screen {
        Screen::List => list(api, current_user, token).into_any(),
        Screen::New => form(api, token, None).into_any(),
        Screen::Edit => {
            let params = leptos_router::hooks::use_params_map();
            view!{<For each=move||vec![params.with(|p|p.get("id")).unwrap_or_default()] key=|id|id.clone() children=move|id|{
                if let Ok(id) = id.parse::<i64>() && id > 0 {
                    form(api,token.clone(),Some(id)).into_any()
                } else {
                    view!{<ManagementPage><h1>"Invalid user address"</h1><p role="alert">"Choose a user from the list to edit."</p><a href="/users">"Back to users"</a></ManagementPage>}.into_any()
                }
            }/>}.into_any()
        }
    }
}

fn form(api: Api, token: String, id: Option<i64>) -> impl IntoView {
    let token = StoredValue::new(token);
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let role = RwSignal::new("user".to_owned());
    let locked = RwSignal::new(false);
    let reset = RwSignal::new(true);
    let baseline = RwSignal::new((String::new(), "user".to_owned(), false, true));
    let loaded = RwSignal::new(id.is_none());
    let loading = RwSignal::new(id.is_some());
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let notice = use_context::<Notice>();
    let navigate = leptos_router::hooks::use_navigate();
    let cancel_navigation = navigate.clone();
    let dirty = Signal::derive(move || {
        loaded.get()
            && (!password.get().is_empty()
                || baseline.get() != (username.get(), role.get(), locked.get(), reset.get()))
    });
    crate::navigation::protect_draft(move || busy.get() || dirty.get());
    if let Some(id) = id {
        spawn_local(async move {
            match api
                .get(&format!("/api/users/{id}"))
                .await
                .and_then(|value| {
                    serde_json::from_value::<User>(value)
                        .map_err(|_| "Invalid user response".to_owned())
                }) {
                Ok(user) => {
                    baseline.set((
                        user.username.clone(),
                        user.role.slug.clone(),
                        user.account_locked,
                        user.password_change_required,
                    ));
                    username.set(user.username);
                    role.set(user.role.slug);
                    locked.set(user.account_locked);
                    reset.set(user.password_change_required);
                    loaded.set(true);
                }
                Err(error) => message.error(error),
            }
            loading.set(false);
        });
    }
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() || !loaded.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        let navigate = navigate.clone();
        spawn_local(async move {
            let value = UserEdit {
                username: username.get_untracked(),
                password: password.get_untracked(),
                role: role.get_untracked(),
                password_change_required: reset.get_untracked(),
                account_locked: locked.get_untracked(),
            };
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
                    baseline.set((
                        username.get_untracked(),
                        role.get_untracked(),
                        locked.get_untracked(),
                        reset.get_untracked(),
                    ));
                    if let Some(notice) = notice {
                        notice.0.set(Some(
                            if id.is_some() {
                                "User saved."
                            } else {
                                "User created."
                            }
                            .into(),
                        ));
                    }
                    navigate(
                        "/users",
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
    view! {<ManagementPage>
        <a class="breadcrumb" href="/users">"Back to users"</a>
        <PageHeader title=if id.is_some(){"Edit user".to_owned()}else{"New user".to_owned()}
            status=true subtitle=Signal::derive(move || if busy.get(){"Saving..."}else if loading.get(){"Loading user..."}else if !loaded.get(){"User could not be loaded"}else if dirty.get(){"Unsaved changes"}else if id.is_none(){"Not saved yet"}else{"No unsaved changes"}.to_owned())>
            <SaveButton kind="submit" form="user-form" disabled=Signal::derive(move || busy.get() || !loaded.get())
                label=Signal::derive(move || if busy.get(){"Saving..."}else if id.is_some(){"Save changes"}else{"Create user"}.to_owned())/>
        </PageHeader>
        {message.view()}
        <form id="user-form" on:submit=submit>
            <FormFields disabled=Signal::derive(move || busy.get() || !loaded.get())>
                <Field label="Username"><input autocomplete="off" required prop:value=move||username.get() on:input=move|e|username.set(event_target_value(&e))/></Field>
                <Field label=if id.is_some(){"New password (leave empty to keep current)"}else{"Temporary password"}><input type="password" autocomplete="new-password" required=id.is_none() prop:value=move||password.get() on:input=move|e|password.set(event_target_value(&e))/></Field>
                <Field label="Role"><select prop:value=move||role.get() on:change=move|e|role.set(event_target_value(&e))><option value="user">"User"</option><option value="admin">"Administrator"</option>{move||{let slug=role.get();(!matches!(slug.as_str(),"user"|"admin")).then(||view!{<option value=slug.clone()>{slug.clone()}</option>})}}</select></Field>
                <Field label="Require password change" checkbox=true><input type="checkbox" prop:checked=move||reset.get() on:change=move|e|reset.set(event_target_checked(&e))/></Field>
                <Field label="Lock account" checkbox=true><input type="checkbox" prop:checked=move||locked.get() on:change=move|e|locked.set(event_target_checked(&e))/></Field>
            </FormFields>
            <FormFooter><button type="button" class="text-danger" disabled=move||busy.get() on:click=move |_|{password.set(String::new());loaded.set(false);cancel_navigation("/users",Default::default());}>"Cancel changes"</button></FormFooter>
        </form>
    </ManagementPage>}
}

fn list(api: Api, current_user: i64, token: String) -> impl IntoView {
    let token = StoredValue::new(token);
    let list = RwSignal::new(Vec::<User>::new());
    let search = RwSignal::new(String::new());
    let failed = RwSignal::new(false);
    let message = Message::new(String::new());
    if let Some(notice) = use_context::<Notice>() {
        let mut saved = None;
        notice.0.update(|value| saved = value.take());
        if let Some(saved) = saved {
            message.status(saved);
        }
    }
    let busy = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    let deleting = RwSignal::new(None::<(i64, String)>);
    let refresh = move || {
        loaded.set(false);
        failed.set(false);
        spawn_local(async move {
            match api.get("/api/users/").await.and_then(|v| {
                serde_json::from_value::<Vec<User>>(v).map_err(|_| "Invalid user response".into())
            }) {
                Ok(users) => {
                    list.set(users);
                }
                Err(error) => {
                    failed.set(true);
                    message.error(error);
                }
            }
            loaded.set(true);
        });
    };
    refresh();
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
                        refresh();
                        message.status("User and owned data deleted.".into());
                    }
                    Err(error) => message.error(error),
                }
                busy.set(false);
            });
        }
    };
    let visible = Signal::derive(move || {
        let needle = search.get().to_lowercase();
        list.get()
            .into_iter()
            .filter(|user| user.username.to_lowercase().contains(&needle))
            .collect::<Vec<_>>()
    });
    view! { <ManagementPage>
        <PageHeader title="Users".to_owned() subtitle="Manage access, roles and account status.".to_owned()><a class="button primary" href="/users/new">"Create user"</a></PageHeader>
        {message.view()}
        <div class="management-toolbar"><Field label="Search by username"><input type="search" prop:value=move||search.get() on:input=move|e|search.set(event_target_value(&e))/></Field><span>{move||format!("{} users",visible.with(Vec::len))}</span></div>
        <Show when=move||!loaded.get()><p role="status">"Loading users..."</p></Show>
        <DataTable label="Users" headers=vec!["User","Role","Status","Actions"]>
        {move || visible.get().into_iter().map(|user| { let id = user.id; let delete_name = user.username.clone(); let impersonate_name = user.username.clone(); let own_account = id == current_user; let can_impersonate = !user.account_locked && !user.password_change_required;
            view! { <tr><th scope="row"><a href=format!("/users/{id}/edit")>{user.username}</a></th><td>{user.role.name}</td><td>{if user.account_locked { "Locked" } else if user.password_change_required { "Password setup" } else { "Active" }}</td><td><div class="row-actions">
                <a href=format!("/users/{id}/edit")>"Edit"</a>
                <button disabled=!can_impersonate on:click=move |_| { let name = impersonate_name.clone(); spawn_local(async move { match api.form("/impersonate", &token.get_value(), &[("username",name)]).await { Ok(_) => navigate("/"), Err(error) => message.error(error) } }); }>"View as"</button>
                <button class="text-danger" disabled=own_account title=if own_account { "You cannot delete your own account" } else { "Delete this user" } on:click=move |_| {message.clear();deleting.set(Some((id,delete_name.clone())))}>"Delete"</button></div></td></tr> }
        }).collect_view()}
        </DataTable>
        <Show when=move||loaded.get()&&!failed.get()&&visible.with(Vec::is_empty)><EmptyState title="No users found"><p>"Try another search or create a user."</p></EmptyState></Show>
        {move || deleting.get().map(|(_,name)| view! { <DeleteConfirmation title=format!("Delete {name}?") description="This permanently removes the account and its owned campaigns, templates and other data." confirm_label="Delete user and data" busy=busy on_confirm=Callback::new(remove) on_cancel=Callback::new(move|()|deleting.set(None)) message=message/> })}
    </ManagementPage> }
}

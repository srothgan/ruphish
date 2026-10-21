//! Separate user list and form routes; account APIs remain the authorization boundary.
use super::{Api, navigate};
use crate::feedback::Message;
use crate::shared::components::*;
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_contracts::auth::{User, UserEdit};
use ruphish_ui::{tr, trs};

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
                    view!{<ManagementPage><h1>{tr!(ui.invalid_user_address)}</h1><p role="alert">{tr!(ui.choose_a_user_from_the_list_to_edit)}</p><a href="/users">{tr!(ui.back_to_users)}</a></ManagementPage>}.into_any()
                }
            }/>}.into_any()
        }
    }
}

fn form(api: Api, token: String, id: Option<i64>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

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
    let reload = RwSignal::new(0u64);
    if let Some(id) = id {
        Effect::new(move |_| {
            reload.get();
            loading.set(true);
            message.clear();
            spawn_local(async move {
                match api
                    .get(&format!("/api/users/{id}"))
                    .await
                    .and_then(|value| {
                        serde_json::from_value::<User>(value).map_err(|_| {
                            leptos_i18n::t_string!(i18n, ui.invalid_user_response).to_owned()
                        })
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
                                leptos_i18n::t_string!(i18n, ui.user_saved)
                            } else {
                                leptos_i18n::t_string!(i18n, ui.user_created)
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
        <BackLink href="/users">{tr!(ui.back_to_users)}</BackLink>
        <PageHeader title=Signal::derive(move || (if id.is_some(){leptos_i18n::t_string!(i18n, ui.edit_user).to_owned()}else{leptos_i18n::t_string!(i18n, ui.new_user).to_owned()}).to_owned())
            status=true subtitle=Signal::derive(move || if busy.get(){leptos_i18n::t_string!(i18n, ui.saving)}else if loading.get(){leptos_i18n::t_string!(i18n, ui.loading_user)}else if !loaded.get(){leptos_i18n::t_string!(i18n, ui.user_could_not_be_loaded)}else if dirty.get(){leptos_i18n::t_string!(i18n, ui.unsaved_changes)}else if id.is_none(){leptos_i18n::t_string!(i18n, ui.not_saved_yet)}else{leptos_i18n::t_string!(i18n, ui.no_unsaved_changes)}.to_owned())>
            <SaveButton kind="submit" form="user-form" disabled=Signal::derive(move || busy.get() || !loaded.get())
                label=Signal::derive(move || if busy.get(){leptos_i18n::t_string!(i18n, ui.saving)}else if id.is_some(){leptos_i18n::t_string!(i18n, ui.save_changes)}else{leptos_i18n::t_string!(i18n, ui.create_user)}.to_owned())/>
        </PageHeader>
        {message.view()}
        <Show when=move||!loaded.get()&&!loading.get()><ActionButton on_click=move |_|reload.update(|n|*n+=1)>{tr!(ui.retry_loading_user)}</ActionButton></Show>
        <form id="user-form" on:submit=submit>
            <FormFields disabled=Signal::derive(move || busy.get() || !loaded.get())>
                <Field label=trs!(ui.username)><TextInput autocomplete="off" required=true value=Signal::derive(move||username.get()) on_input=move|e|username.set(event_target_value(&e))/></Field>
                <Field label=Signal::derive(move || (if id.is_some(){leptos_i18n::t_string!(i18n, ui.new_password_leave_empty_to_keep_current)}else{leptos_i18n::t_string!(i18n, ui.temporary_password)}).to_owned())><TextInput input_type="password" autocomplete="new-password" required=id.is_none() value=Signal::derive(move||password.get()) on_input=move|e|password.set(event_target_value(&e))/></Field>
                <Select label=trs!(ui.role) value=role on_change=Callback::new(move|v|role.set(v))><option value="user">{tr!(ui.user)}</option><option value="admin">{tr!(ui.administrator_2)}</option>{move||{let slug=role.get();(!matches!(slug.as_str(),"user"|"admin")).then(||view!{<option value=slug.clone()>{slug.clone()}</option>})}}</Select>
                <Checkbox label=trs!(ui.require_password_change) checked=reset on_toggle=Callback::new(move|v|reset.set(v))/>
                <Checkbox label=trs!(ui.lock_account) checked=locked on_toggle=Callback::new(move|v|locked.set(v))/>
            </FormFields>
            <FormFooter><ActionButton kind="button" style=ButtonStyle::Ghost disabled=Signal::derive(move||busy.get()) on_click=move |_|{password.set(String::new());loaded.set(false);cancel_navigation("/users",Default::default());}>{tr!(ui.cancel_changes)}</ActionButton></FormFooter>
        </form>
    </ManagementPage>}
}

fn list(api: Api, current_user: i64, token: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(token);
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
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
    let generation = StoredValue::new(0u64);
    let refresh = move || {
        generation.update_value(|n| *n += 1);
        let ticket = generation.get_value();
        loaded.set(false);
        failed.set(false);
        tasks.spawn(async move {
            let result = api.get("/api/users/").await.and_then(|v| {
                serde_json::from_value::<Vec<User>>(v)
                    .map_err(|_| leptos_i18n::t_string!(i18n, ui.invalid_user_response).into())
            });
            if generation.get_value() != ticket {
                return;
            }
            match result {
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
            generation.update_value(|n| *n += 1);
            loaded.set(true);
            busy.set(true);
            tasks.spawn(async move {
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
                        list.update(|rows| rows.retain(|user| user.id != id));
                        deleting.set(None);
                        refresh();
                        message.status(
                            leptos_i18n::t_string!(i18n, ui.user_and_owned_data_deleted).to_owned(),
                        );
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
        <PageHeader title=trs!(ui.users) subtitle=trs!(ui.manage_access_roles_and_account_status)><ActionLink style=ButtonStyle::Primary href="/users/new">{tr!(ui.create_user)}</ActionLink></PageHeader>
        {message.view()}
        <ListToolbar search=search count=Signal::derive(move || visible.with(Vec::len)) label=trs!(ui.search_by_username) noun=trs!(ui.users_2)/>
        <Show when=move||!loaded.get()><p role="status">{tr!(ui.loading_users)}</p></Show>
        <Show when=move||loaded.get()&&failed.get()><ActionButton kind="button" on_click=move |_|refresh()>{tr!(ui.retry_loading_users)}</ActionButton></Show>
        <DataTable label=trs!(ui.users) empty=Signal::derive(move || visible.with(Vec::is_empty)) headers=vec![trs!(ui.user),trs!(ui.role),trs!(ui.status),trs!(ui.actions)]>
        {move || visible.get().into_iter().map(|user| { let id = user.id; let delete_name = user.username.clone(); let impersonate_name = user.username.clone(); let own_account = id == current_user; let can_impersonate = !user.account_locked && !user.password_change_required;
            view! { <tr><th scope="row"><a href=format!("/users/{id}/edit")>{user.username}</a></th><td>{ruphish_ui::localization::role_label(i18n, &user.role)}</td><td>{move || if user.account_locked { leptos_i18n::t_string!(i18n, ui.locked) } else if user.password_change_required { leptos_i18n::t_string!(i18n, ui.password_setup) } else { leptos_i18n::t_string!(i18n, ui.active) }}</td><td><div class="row-actions">
                <a href=format!("/users/{id}/edit")>{tr!(ui.edit)}</a>
                <ActionButton disabled=Signal::derive(move||!can_impersonate||busy.get()) on_click=move |_| { if busy.get_untracked(){return;} busy.set(true); let name = impersonate_name.clone(); tasks.spawn(async move { match api.form("/impersonate", &token.get_value(), &[("username",name)]).await { Ok(_) => navigate("/"), Err(error) => message.error(error) } busy.set(false); }); }>{tr!(ui.view_as)}</ActionButton>
                <ActionButton style=ButtonStyle::Ghost size=ControlSize::Small disabled=Signal::derive(move||own_account||busy.get()) title=Signal::derive(move || (if own_account { leptos_i18n::t_string!(i18n, ui.you_cannot_delete_your_own_account) } else { leptos_i18n::t_string!(i18n, ui.delete_this_user) }).to_owned()) on_click=move |_| {message.clear();deleting.set(Some((id,delete_name.clone())))}>{tr!(ui.delete)}</ActionButton></div></td></tr> }
        }).collect_view()}
        </DataTable>
        <ListEmptyState ready=Signal::derive(move || loaded.get() && !failed.get()) count=Signal::derive(move || visible.with(Vec::len)) search=search title=trs!(ui.no_users_found) guidance=trs!(ui.create_a_user_to_get_started)/>
        {move || deleting.get().map(|(_,name)| view! { <DeleteConfirmation title=leptos_i18n::t_string!(i18n, messages.delete_arg0, arg0 = (name).to_string()) description=trs!(ui.this_permanently_removes_the_account_and_its_owned_campaigns_templates_and_other) confirm_label=trs!(ui.delete_user_and_data) busy=busy on_confirm=Callback::new(remove) on_cancel=Callback::new(move|()|deleting.set(None)) message=message/> })}
    </ManagementPage> }
}

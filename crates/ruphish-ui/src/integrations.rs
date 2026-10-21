//! Webhook administration and reporting mailbox resources share presentation primitives.
use crate::{
    account::Api,
    campaigns::{array, text},
    feedback::Message,
    resources::ResourceAccess,
    shared::components::*,
    time_display::instant,
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map};
use ruphish_ui::view_tasks::ViewTasks;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Webhook,
    Mailbox,
}
impl Kind {
    fn path(self) -> &'static str {
        match self {
            Self::Webhook => "/webhooks",
            Self::Mailbox => "/reporting-mailboxes",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Webhook => "Webhooks",
            Self::Mailbox => "Reporting mailboxes",
        }
    }
    fn singular(self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::Mailbox => "reporting mailbox",
        }
    }
    fn plural(self) -> &'static str {
        match self {
            Self::Webhook => "webhooks",
            Self::Mailbox => "reporting mailboxes",
        }
    }
    fn empty_title(self) -> &'static str {
        match self {
            Self::Webhook => "No webhooks yet",
            Self::Mailbox => "No reporting mailboxes yet",
        }
    }
    fn empty_guidance(self) -> &'static str {
        match self {
            Self::Webhook => "Add a webhook endpoint to receive campaign events as they happen.",
            Self::Mailbox => {
                "Connect a reporting mailbox to correlate the messages recipients report."
            }
        }
    }
    fn api(self) -> String {
        format!("/api{}", self.path())
    }
    fn webhook(self) -> bool {
        matches!(self, Self::Webhook)
    }
}

pub(super) fn list(api: Api, token: String, kind: Kind) -> impl IntoView {
    let rows = RwSignal::new(Vec::<Value>::new());
    let loading = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    let failed = RwSignal::new(false);
    let search = RwSignal::new(String::new());
    let refresh = RwSignal::new(0u64);
    let message = Message::new(String::new());
    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let deletion = ResourceDeletion::new();
    let generation = StoredValue::new(0u64);
    let deleted = Callback::new(move |id| {
        generation.update_value(|n| *n += 1);
        loading.set(false);
        failed.set(false);
        rows.update(|rows| rows.retain(|row| row["id"].as_i64() != Some(id)));
        message.status("Resource deleted.".into());
    });
    Effect::new(move |_| {
        refresh.get();
        generation.update_value(|n| *n += 1);
        let ticket = generation.get_value();
        loading.set(true);
        failed.set(false);
        spawn_local(async move {
            let result = api.get(&format!("{}/", kind.api())).await;
            if generation.get_value() != ticket {
                return;
            }
            match result {
                Ok(value) => {
                    if let Some(values) = value.as_array() {
                        rows.set(values.clone());
                        loaded.set(true);
                        message.clear();
                    } else {
                        failed.set(true);
                        message.error(
                            "The server returned an invalid resource list. Refresh to try again."
                                .into(),
                        );
                    }
                }
                Err(error) => {
                    failed.set(true);
                    message.error(error);
                }
            }
            loading.set(false);
        });
    });
    let visible = Signal::derive(move || {
        let needle = search.get().to_lowercase();
        rows.get()
            .into_iter()
            .filter(|row| text(row, "name").to_lowercase().contains(&needle))
            .collect::<Vec<_>>()
    });
    view! {
        <ManagementPage>
            <PageHeader title=kind.title() subtitle=if kind.webhook() { "Administrator-managed event notifications. Enabled endpoints receive events across campaigns." } else { "Owner-scoped IMAP resources that correlate reported campaign messages without retaining email content." }>
                <ActionButton on_click=move |_| refresh.update(|value| *value += 1) busy=Signal::derive(move || loading.get().to_string())>"Refresh"</ActionButton>
                {can_edit.then(|| view! { <ActionLink style=ButtonStyle::Primary href=format!("{}/new", kind.path())>{format!("Create {}", kind.singular())}</ActionLink> })}
            </PageHeader>
            {message.view()}
            <ResourceDeleteDialog deletion=deletion api=api token=token on_deleted=deleted/>
            <ListToolbar search=search count=Signal::derive(move || visible.with(Vec::len)) noun=kind.plural()/>
            <Show when=move || loading.get()><p role="status">{format!("Loading {}…", kind.plural())}</p></Show>
            <DataTable label=kind.title() empty=Signal::derive(move || visible.with(Vec::is_empty)) headers=vec!["Name", "Destination", "Enabled", "Status", "Actions"]>
                {move || visible.get().into_iter().map(|row| {
                    let href = format!("{}/{}", kind.path(), row["id"]);
                    let target = DeleteTarget {id:row["id"].as_i64().unwrap_or(0),name:text(&row,"name"),path:format!("{}/{}",kind.api(),row["id"]),revision:row["revision"].as_i64().map(|v|v.to_string()),description:"This removes the configuration and its processing history. Already dispatched requests cannot be recalled."};
                    view! {
                        <tr><th scope="row"><a href=href.clone()>{text(&row, "name")}</a></th>
                            <td>{text(&row, if kind.webhook() { "url" } else { "host" })}</td>
                            <td>{if row[if kind.webhook() { "is_active" } else { "enabled" }].as_bool().unwrap_or(false) { "Yes" } else { "No" }}</td>
                            <td>{if row["running"] == true { "Checking".into() } else { text(&row, "code") }}</td>
                            <td><ResourceActions view_href=href.clone() edit_href=format!("{href}/edit") can_edit=can_edit size=ControlSize::Small disabled=deletion.busy on_delete=Callback::new(move |()|deletion.select(target.clone()))/></td>
                        </tr>
                    }
                }).collect_view()}
            </DataTable>
            <ListEmptyState ready=Signal::derive(move || loaded.get() && !loading.get() && !failed.get()) count=Signal::derive(move || visible.with(Vec::len)) search=search title=kind.empty_title() guidance=kind.empty_guidance()/>
        </ManagementPage>
    }
}

pub(super) fn page(api: Api, token: String, kind: Kind, editing: bool) -> impl IntoView {
    let params = use_params_map();
    let token = StoredValue::new(token);
    view! {
        <For each=move || vec![params.with(|p| p.get("id")).unwrap_or_default()] key=|id| id.clone() children=move |id| {
            if id == "new" { form(api, token.get_value(), kind, None).into_any() }
            else if let Ok(id) = id.parse::<i64>() {
                if editing { form(api, token.get_value(), kind, Some(id)).into_any() }
                else { detail(api, token.get_value(), kind, id).into_any() }
            } else { view! { <p role="alert">"Invalid resource address"</p> }.into_any() }
        }/>
    }
}

fn field(
    draft: RwSignal<Value>,
    key: &'static str,
    label: &'static str,
    input_type: &'static str,
) -> impl IntoView {
    view! {
        <Field label=label><TextInput input_type=input_type value=Signal::derive(move || {
            let value = draft.get(); value[key].as_str().map(str::to_owned).unwrap_or_else(|| value[key].as_u64().map(|n| n.to_string()).unwrap_or_default())
        }) on_input=move |event| {
            let value = event_target_value(&event);
            draft.update(|draft| draft[key] = if input_type == "number" { value.parse::<u32>().map_or(Value::Null, |n| json!(n)) } else { json!(value) });
        }/></Field>
    }
}

fn checkbox(draft: RwSignal<Value>, key: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <Checkbox label=label checked=Signal::derive(move || draft.get()[key].as_bool().unwrap_or(false))
            on_toggle=Callback::new(move |value| draft.update(|draft| draft[key] = json!(value)))/>
    }
}

fn edit_value(kind: Kind, value: &Value) -> Value {
    let keys: &[&str] = if kind.webhook() {
        &["name", "url", "is_active", "revision"]
    } else {
        &[
            "name",
            "enabled",
            "host",
            "port",
            "username",
            "tls",
            "folder",
            "restrict_domain",
            "delete_reported",
            "frequency",
            "revision",
        ]
    };
    let mut edit = serde_json::Map::new();
    for key in keys {
        edit.insert((*key).into(), value[*key].clone());
    }
    Value::Object(edit)
}

fn form(api: Api, token: String, kind: Kind, id: Option<i64>) -> impl IntoView {
    let initial = if kind.webhook() {
        json!({"name":"","url":"","is_active":true,"revision":null})
    } else {
        json!({"name":"","enabled":true,"host":"","port":993,"username":"","tls":true,"folder":"INBOX","restrict_domain":"","delete_reported":false,"frequency":60,"revision":null})
    };
    let draft = RwSignal::new(initial.clone());
    let baseline = RwSignal::new(initial);
    let loaded = RwSignal::new(id.is_none());
    let has_saved_secret = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let refresh = RwSignal::new(0u64);
    let message = Message::new(String::new());
    let tasks = ViewTasks::default();
    let token = StoredValue::new(token);
    let navigate = StoredValue::new(use_navigate());
    let editable = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    crate::navigation::protect_draft(move || {
        loaded.get() && draft.get() != baseline.get() || busy.get()
    });
    if let Some(id) = id {
        Effect::new(move |_| {
            refresh.get();
            spawn_local(async move {
                match api.get(&format!("{}/{id}", kind.api())).await {
                    Ok(value) => {
                        has_saved_secret.set(
                            value[if kind.webhook() {
                                "has_secret"
                            } else {
                                "has_password"
                            }]
                            .as_bool()
                            .unwrap_or(false),
                        );
                        let value = edit_value(kind, &value);
                        baseline.set(value.clone());
                        draft.set(value);
                        loaded.set(true);
                        message.clear();
                    }
                    Err(error) => message.error(error),
                }
            });
        });
    }
    let save = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() || !loaded.get_untracked() || !editable {
            return;
        }
        let mut value = draft.get_untracked();
        let secret = if kind.webhook() { "secret" } else { "password" };
        if value[secret].as_str().is_none_or(str::is_empty) {
            value.as_object_mut().map(|object| object.remove(secret));
        }
        busy.set(true);
        tasks.spawn(async move {
            let path = id.map_or_else(
                || format!("{}/", kind.api()),
                |id| format!("{}/{id}", kind.api()),
            );
            match api
                .resource(
                    if id.is_some() { "PUT" } else { "POST" },
                    &path,
                    &token.get_value(),
                    Some(&value),
                    None,
                )
                .await
            {
                Ok((saved, _)) => {
                    baseline.set(draft.get_untracked());
                    busy.set(false);
                    navigate.with_value(|navigate| {
                        navigate(
                            &format!("{}/{}", kind.path(), saved["id"]),
                            Default::default(),
                        )
                    });
                }
                Err(error) => {
                    busy.set(false);
                    message.error(error);
                }
            }
        });
    };
    view! {
        <ManagementPage><BackLink href=kind.path()>{format!("Back to {}",kind.title().to_lowercase())}</BackLink><PageHeader title=format!("{} {}", if id.is_some() { "Edit" } else { "Create" }, kind.singular())/>
            {message.view()}
            <Show when=move || !loaded.get()><ActionButton on_click=move |_| refresh.update(|r| *r += 1)>"Retry loading"</ActionButton></Show>
            <form on:submit=save><fieldset disabled=move || busy.get() || !loaded.get() || !editable>
                <FormFields disabled=Signal::derive(move || busy.get() || !loaded.get() || !editable)>
                    {field(draft, "name", "Name", "text")}
                    {if kind.webhook() {
                        view! {
                            {field(draft, "url", "Endpoint URL", "url")}
                            <SecretField label="Signing secret" saved_hint="Leave empty to keep the saved secret."
                                has_saved_value=has_saved_secret
                                value=Signal::derive(move || text(&draft.get(), "secret"))
                                on_input=Callback::new(move |value: String| draft.update(|draft| draft["secret"] = json!(value)))/>
                            {checkbox(draft, "is_active", "Enabled")}
                            <p>"Delivery is at least once. Receivers should deduplicate X-Ruphish-Event-ID. Saving configuration changes cancels pending deliveries for the previous configuration."</p>
                        }.into_any()
                    } else {
                        view! {
                            {field(draft, "host", "IMAP hostname", "text")}
                            {field(draft, "port", "Port", "number")}
                            {field(draft, "username", "Service account username", "text")}
                            <SecretField label="Service account password" saved_hint="Leave empty to keep the saved password."
                                has_saved_value=has_saved_secret
                                value=Signal::derive(move || text(&draft.get(), "password"))
                                on_input=Callback::new(move |value: String| draft.update(|draft| draft["password"] = json!(value)))/>
                            {field(draft, "folder", "Folder", "text")}
                            {field(draft, "frequency", "Check interval in seconds (minimum 30)", "number")}
                            {field(draft, "restrict_domain", "Optional sender-domain filter", "text")}
                            {checkbox(draft, "tls", "Use implicit TLS (otherwise STARTTLS is required)")}
                            {checkbox(draft, "delete_reported", "Delete successfully matched messages after processing")}
                            {checkbox(draft, "enabled", "Enabled")}
                            <p>"The first successful check starts monitoring new arrivals. Existing mail is left untouched. Message bodies are processed temporarily and never retained. The From-domain filter does not authenticate the reporter."</p>
                        }.into_any()
                    }}
                </FormFields>
                <FormFooter spread=true><ActionLink style=ButtonStyle::Ghost disabled=busy href=id.map_or_else(|| kind.path().into(), |id| format!("{}/{id}", kind.path()))>"Cancel changes"</ActionLink><SaveButton kind="submit" disabled=Signal::derive(move || busy.get() || !loaded.get() || !editable) label=Signal::derive(move || if busy.get() { "Saving\u{2026}".to_owned() } else if id.is_some() { "Save changes".to_owned() } else { format!("Create {}",kind.singular()) })/></FormFooter>
            </fieldset></form>
        </ManagementPage>
    }
}

fn detail(api: Api, token: String, kind: Kind, id: i64) -> impl IntoView {
    let value = RwSignal::new(Value::Null);
    let refresh = RwSignal::new(0u64);
    let loading = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let deletion = ResourceDeletion::new();
    let unavailable =
        Signal::derive(move || busy.get() || deletion.busy.get() || !value.with(Value::is_object));
    let resuming = RwSignal::new(false);
    let message = Message::new(String::new());
    let tasks = ViewTasks::default();
    let token = StoredValue::new(token);
    let navigate = StoredValue::new(use_navigate());
    let deleted = Callback::new(move |_| {
        navigate.with_value(|navigate| navigate(kind.path(), Default::default()))
    });
    let select_delete = Callback::new(move |()| {
        if unavailable.get_untracked() {
            return;
        }
        let data = value.get_untracked();
        deletion.select(DeleteTarget {id,name:text(&data,"name"),path:format!("{}/{id}",kind.api()),revision:data["revision"].as_i64().map(|v|v.to_string()),description:"This removes the configuration and its processing history. Already dispatched requests cannot be recalled."});
    });
    let editable = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    Effect::new(move |_| {
        refresh.get();
        if deletion.busy.get_untracked() {
            return;
        }
        loading.set(true);
        spawn_local(async move {
            match api.get(&format!("{}/{id}", kind.api())).await {
                Ok(data) => value.set(data),
                Err(error) => message.error(error),
            }
            loading.set(false);
        });
    });
    let interval = set_interval_with_handle(
        move || {
            if !loading.get_untracked() && !busy.get_untracked() && !deletion.busy.get_untracked() {
                refresh.update(|r| *r += 1);
            }
        },
        std::time::Duration::from_secs(5),
    )
    .ok();
    on_cleanup(move || {
        if let Some(interval) = interval {
            interval.clear();
        }
    });
    let action = move |operation: &'static str| {
        if unavailable.get_untracked() || !editable {
            return;
        }
        busy.set(true);
        message.clear();
        tasks.spawn(async move {
            let path = format!("{}/{id}/{operation}", kind.api());
            let body = (operation == "resume")
                .then(|| json!({"revision":value.get_untracked()["revision"]}));
            match api
                .resource("POST", &path, &token.get_value(), body.as_ref(), None)
                .await
            {
                Ok(_) => {
                    message.status(
                        if operation == "validate" {
                            "Connection validated successfully."
                        } else {
                            "Check queued. Results refresh automatically every five seconds."
                        }
                        .into(),
                    );
                    refresh.update(|r| *r += 1);
                    resuming.set(false);
                }
                Err(error) => message.error(error),
            }
            busy.set(false);
        });
    };
    view! {
        <ManagementPage><BackLink href=kind.path()>{format!("Back to {}", kind.title().to_lowercase())}</BackLink>
            <PageHeader title=Signal::derive(move || text(&value.get(), "name"))>
                <ActionButton on_click=move |_| refresh.update(|r| *r += 1) busy=Signal::derive(move || loading.get().to_string())>"Refresh"</ActionButton>
                {editable.then(|| view! {
                    <ActionButton disabled=unavailable on_click=move |_| action("validate")>{move || if busy.get() { "Working…" } else { "Validate connection" }}</ActionButton>
                    {(!kind.webhook()).then(|| view! { <ActionButton disabled=Signal::derive(move || unavailable.get() || value.get()["running"] == true || value.get()["enabled"] != true) on_click=move |_| action("check")>"Check mailbox now"</ActionButton> })}
                    <ResourceActions edit_href=format!("{}/{id}/edit",kind.path()) can_edit=editable disabled=unavailable on_delete=select_delete/>
                })}
            </PageHeader>{message.view()}
            <Card class="integration-status">
                <dl><dt>"Destination"</dt><dd>{move || text(&value.get(), if kind.webhook() { "url" } else { "host" })}</dd>
                    <dt>"Enabled"</dt><dd>{move || if value.get()[if kind.webhook() { "is_active" } else { "enabled" }] == true { "Yes" } else { "No" }}</dd>
                    <dt>"Credential configured"</dt><dd>{move || if value.get()[if kind.webhook() { "has_secret" } else { "has_password" }] == true { "Yes" } else { "No" }}</dd>
                    {(!kind.webhook()).then(|| view! {
                        <dt>"Account"</dt><dd>{move || text(&value.get(), "username")}</dd>
                        <dt>"Folder"</dt><dd>{move || text(&value.get(), "folder")}</dd>
                        <dt>"Transport"</dt><dd>{move || format!("{} ? port {}", if value.get()["tls"] == true { "TLS" } else { "STARTTLS" }, value.get()["port"])}</dd>
                        <dt>"Check interval"</dt><dd>{move || format!("{} seconds",value.get()["frequency"])}</dd>
                        <dt>"Last outcome"</dt><dd>{move || text(&value.get(), "code")}</dd>
                        <dt>"Last successful check"</dt><dd>{move || if value.get()["last_success"] == 0 { "Not recorded".into() } else { instant(&value.get()["last_success"]) }}</dd>
                        <dt>"Next check"</dt><dd>{move || if value.get()["enabled"] != true { "Disabled".into() } else if value.get()["next_check"] == 0 { "Queued".into() } else { instant(&value.get()["next_check"]) }}</dd>
                    })}
                </dl>
                <Show when=move || value.get()["running"] == true><p role="status" aria-busy="true">"Checking reporting mailbox…"</p></Show>
            </Card>
            <Show when=move || !kind.webhook() && value.get()["code"] == "mailbox_uidvalidity_changed">
                <p role="alert">"The mail server changed its message identifiers. Processing is paused to protect existing report history."</p>
                {editable.then(|| view! { <ActionButton disabled=unavailable on_click=move |_| resuming.set(true)>"Resume with new arrivals"</ActionButton> })}
            </Show>
            <Show when=move || resuming.get()>
                <DeleteConfirmation title="Resume with new arrivals?".to_owned() description="Existing messages in the changed folder will be left untouched. Monitoring starts at the next successful check; saved campaign reports remain intact." confirm_label="Resume monitoring" busy=Signal::derive(move || busy.get()) on_confirm=Callback::new(move |()| action("resume")) on_cancel=Callback::new(move |()| resuming.set(false)) message=message/>
            </Show>
            <h2>{if kind.webhook() { "Recent delivery attempts" } else { "Recent mailbox checks" }}</h2>
            <DataTable label="Integration history" headers=if kind.webhook() { vec!["Started", "Finished", "Outcome", "HTTP status", "Event", "Attempt"] } else { vec!["Started", "Finished", "Outcome", "Matched", "Rejected"] }>
                {move || array(&value.get(), "history").into_iter().map(|run| view! {
                    <tr><td class="data-value">{instant(&run["started"])}</td><td>{if run["finished"].is_null() { "Running".into() } else { instant(&run["finished"]) }}</td>
                        <td>{text(&run, "code")}</td>{if kind.webhook() { view! { <td>{run["status"].as_i64().map(|s| s.to_string()).unwrap_or_default()}</td><td>{run["event_id"].as_i64().map(|s| s.to_string()).unwrap_or_default()}</td><td>{run["attempt"].as_i64().map(|s| s.to_string()).unwrap_or_default()}</td> }.into_any() } else { view! { <td>{run["matched"].as_i64().unwrap_or(0)}</td><td>{run["rejected"].as_i64().unwrap_or(0)}</td> }.into_any() }}
                    </tr>
                }).collect_view()}
            </DataTable>
            <ResourceDeleteDialog deletion=deletion api=api token=token.get_value() on_deleted=deleted/>
        </ManagementPage>
    }
}

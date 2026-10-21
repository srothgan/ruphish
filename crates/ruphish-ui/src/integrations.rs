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
use ruphish_ui::{tr, trs};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Webhook,
    Mailbox,
}
impl Kind {
    fn back_label(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();
        match self {
            Self::Webhook => leptos_i18n::t_string!(i18n, ui.back_to_webhooks),
            Self::Mailbox => leptos_i18n::t_string!(i18n, ui.back_to_mailboxes),
        }
    }
    fn path(self) -> &'static str {
        match self {
            Self::Webhook => "/webhooks",
            Self::Mailbox => "/reporting-mailboxes",
        }
    }
    fn title(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Webhook => leptos_i18n::t_string!(i18n, ui.webhooks),
            Self::Mailbox => leptos_i18n::t_string!(i18n, ui.reporting_mailboxes),
        }
    }
    fn singular(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Webhook => leptos_i18n::t_string!(i18n, ui.webhook),
            Self::Mailbox => leptos_i18n::t_string!(i18n, ui.reporting_mailbox_2),
        }
    }
    fn plural(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Webhook => leptos_i18n::t_string!(i18n, ui.webhooks),
            Self::Mailbox => leptos_i18n::t_string!(i18n, ui.reporting_mailboxes_2),
        }
    }
    fn empty_title(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Webhook => leptos_i18n::t_string!(i18n, ui.no_webhooks_yet),
            Self::Mailbox => leptos_i18n::t_string!(i18n, ui.no_reporting_mailboxes_yet),
        }
    }
    fn empty_guidance(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Webhook => leptos_i18n::t_string!(
                i18n,
                ui.add_a_webhook_endpoint_to_receive_campaign_events_as_they_happen
            ),
            Self::Mailbox => {
                leptos_i18n::t_string!(
                    i18n,
                    ui.connect_a_reporting_mailbox_to_correlate_the_messages_recipients_report
                )
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
    let i18n = ruphish_ui::localization::use_i18n();

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
        message.status(leptos_i18n::t_string!(i18n, ui.resource_deleted).to_owned());
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
                        message.error(leptos_i18n::t_string!(i18n, ui.the_server_returned_an_invalid_resource_list_refresh_to_try_again).to_owned());
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
            <PageHeader title=Signal::derive(move || kind.title().to_owned()) subtitle=Signal::derive(move || (if kind.webhook() { leptos_i18n::t_string!(i18n, ui.administrator_managed_event_notifications_enabled_endpoints_receive_events_acros) } else { leptos_i18n::t_string!(i18n, ui.owner_scoped_imap_resources_that_correlate_reported_campaign_messages_without_re) }).to_owned())>
                <ActionButton on_click=move |_| refresh.update(|value| *value += 1) busy=Signal::derive(move || loading.get().to_string())>{tr!(ui.refresh)}</ActionButton>
                {can_edit.then(|| view! { <ActionLink style=ButtonStyle::Primary href=format!("{}/new", kind.path())>{leptos_i18n::t!(i18n, messages.create_arg0, arg0 = (kind.singular()).to_string())}</ActionLink> })}
            </PageHeader>
            {message.view()}
            <ResourceDeleteDialog deletion=deletion api=api token=token on_deleted=deleted/>
            <ListToolbar search=search count=Signal::derive(move || visible.with(Vec::len)) noun=Signal::derive(move || kind.plural().to_owned())/>
            <Show when=move || loading.get()><p role="status">{leptos_i18n::t!(i18n, messages.loading_arg0, arg0 = (kind.plural()).to_string())}</p></Show>
            <DataTable label=Signal::derive(move || kind.title().to_owned()) empty=Signal::derive(move || visible.with(Vec::is_empty)) headers=vec![trs!(ui.name), trs!(ui.destination), trs!(ui.enabled), trs!(ui.status), trs!(ui.actions)]>
                {move || visible.get().into_iter().map(|row| {
                    let href = format!("{}/{}", kind.path(), row["id"]);
                    let target = DeleteTarget {id:row["id"].as_i64().unwrap_or(0),name:text(&row,"name"),path:format!("{}/{}",kind.api(),row["id"]),revision:row["revision"].as_i64().map(|v|v.to_string()),description:leptos_i18n::t_string!(i18n, ui.this_removes_the_configuration_and_its_processing_history_already_dispatched_req)};
                    view! {
                        <tr><th scope="row"><a href=href.clone()>{text(&row, "name")}</a></th>
                            <td>{text(&row, if kind.webhook() { "url" } else { "host" })}</td>
                            <td>{if row[if kind.webhook() { "is_active" } else { "enabled" }].as_bool().unwrap_or(false) { leptos_i18n::t_string!(i18n, ui.yes) } else { leptos_i18n::t_string!(i18n, ui.no) }}</td>
                            <td>{if row["running"] == true { leptos_i18n::t_string!(i18n, ui.checking).into() } else { text(&row, "code") }}</td>
                            <td><ResourceActions view_href=href.clone() edit_href=format!("{href}/edit") can_edit=can_edit size=ControlSize::Small disabled=deletion.busy on_delete=Callback::new(move |()|deletion.select(target.clone()))/></td>
                        </tr>
                    }
                }).collect_view()}
            </DataTable>
            <ListEmptyState ready=Signal::derive(move || loaded.get() && !loading.get() && !failed.get()) count=Signal::derive(move || visible.with(Vec::len)) search=search title=Signal::derive(move || kind.empty_title().to_owned()) guidance=Signal::derive(move || kind.empty_guidance().to_owned())/>
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
            } else { view! { <p role="alert">{tr!(ui.invalid_resource_address)}</p> }.into_any() }
        }/>
    }
}

// TODO: Evaluate consolidating resource and integration field helpers through the
// shared form components, keeping feature-specific validation and draft state local.
fn field(
    draft: RwSignal<Value>,
    key: &'static str,
    label: Signal<String>,
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

fn checkbox(draft: RwSignal<Value>, key: &'static str, label: Signal<String>) -> impl IntoView {
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
    let i18n = ruphish_ui::localization::use_i18n();

    let initial = if kind.webhook() {
        json!({"name":"","url":"","is_active":true,"revision":null})
    } else {
        json!({"name":"","enabled":true,"host":"","port":993,"username":"","tls":true,"folder":"INBOX","restrict_domain":"","delete_reported":false,"frequency":60,"revision":null})
    };
    let draft = RwSignal::new(initial.clone());
    let baseline = RwSignal::new(initial);
    let loaded = RwSignal::new(id.is_none());
    let loading = RwSignal::new(id.is_some());
    let load_failed = RwSignal::new(false);
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
            loading.set(true);
            load_failed.set(false);
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
                    Err(error) => {
                        load_failed.set(true);
                        message.error(error);
                    }
                }
                loading.set(false);
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
        <ManagementPage><BackLink href=kind.path()>{move || kind.back_label()}</BackLink><PageHeader status=true title=Signal::derive(move || {
                let name = text(&draft.get(), "name");
                if !name.is_empty() { name } else if kind.webhook() {
                    leptos_i18n::t_string!(i18n, ui.new_webhook).to_owned()
                } else { leptos_i18n::t_string!(i18n, ui.new_reporting_mailbox).to_owned() }
            }) subtitle=Signal::derive(move || {
                if busy.get() { leptos_i18n::t_string!(i18n, ui.saving) }
                else if loading.get() { leptos_i18n::t_string!(i18n, ui.loading_resource) }
                else if load_failed.get() { leptos_i18n::t_string!(i18n, ui.resource_could_not_be_loaded) }
                else if !editable { leptos_i18n::t_string!(i18n, ui.read_only_access) }
                else if draft.get() != baseline.get() { leptos_i18n::t_string!(i18n, ui.unsaved_changes) }
                else if id.is_none() { leptos_i18n::t_string!(i18n, ui.not_saved_yet) }
                else { leptos_i18n::t_string!(i18n, ui.no_unsaved_changes) }.to_owned()
            })/>
            <p class="resource-usage">{move || if kind.webhook() {
                leptos_i18n::t_string!(i18n, ui.webhook_configuration_context)
            } else { leptos_i18n::t_string!(i18n, ui.mailbox_configuration_context) }}</p>
            {message.view()}
            <Show when=move || load_failed.get() && !loading.get()><ActionButton on_click=move |_| refresh.update(|r| *r += 1)>{tr!(ui.retry_loading)}</ActionButton></Show>
            <form on:submit=save>
                <FormFields disabled=Signal::derive(move || busy.get() || !loaded.get() || !editable)>
                    {field(draft, "name", trs!(ui.name), "text")}
                    {if kind.webhook() {
                        view! {
                            <section class="resource-section"><h2>{tr!(ui.connection)}</h2>
                            {field(draft, "url", trs!(ui.endpoint_url), "url")}
                            <SecretField label=trs!(ui.signing_secret) saved_hint=trs!(ui.leave_empty_to_keep_the_saved_secret)
                                has_saved_value=has_saved_secret
                                value=Signal::derive(move || text(&draft.get(), "secret"))
                                on_input=Callback::new(move |value: String| draft.update(|draft| draft["secret"] = json!(value)))/>
                            </section><section class="resource-section"><h2>{tr!(ui.delivery)}</h2>
                            {checkbox(draft, "is_active", trs!(ui.enabled))}
                            <p>{tr!(ui.webhook_delivery_guidance)}</p></section>
                        }.into_any()
                    } else {
                        view! {
                            <section class="resource-section"><h2>{tr!(ui.connection)}</h2><div class="resource-columns">
                            {field(draft, "host", trs!(ui.imap_hostname), "text")}
                            {field(draft, "port", trs!(ui.port), "number")}
                            </div>
                            {checkbox(draft, "tls", trs!(ui.use_implicit_tls_otherwise_starttls_is_required))}
                            </section><section class="resource-section"><h2>{tr!(ui.service_account)}</h2><div class="resource-columns">
                            {field(draft, "username", trs!(ui.service_account_username), "text")}
                            <SecretField label=trs!(ui.service_account_password) saved_hint=trs!(ui.leave_empty_to_keep_the_saved_password)
                                has_saved_value=has_saved_secret
                                value=Signal::derive(move || text(&draft.get(), "password"))
                                on_input=Callback::new(move |value: String| draft.update(|draft| draft["password"] = json!(value)))/>
                            </div></section><section class="resource-section"><h2>{tr!(ui.monitoring)}</h2><div class="resource-columns">
                            {field(draft, "folder", trs!(ui.folder), "text")}
                            {field(draft, "frequency", trs!(ui.check_interval_in_seconds_minimum_30), "number")}
                            {field(draft, "restrict_domain", trs!(ui.optional_sender_domain_filter), "text")}
                            </div>
                            {checkbox(draft, "delete_reported", trs!(ui.delete_successfully_matched_messages_after_processing))}
                            {checkbox(draft, "enabled", trs!(ui.enabled))}
                            <p>{tr!(ui.mailbox_processing_guidance)}</p></section>
                        }.into_any()
                    }}
                </FormFields>
                <FormFooter spread=true><ActionButton style=ButtonStyle::Ghost disabled=busy on_click=move |_| {
                    draft.set(baseline.get_untracked());
                    navigate.with_value(|navigate| navigate(&id.map_or_else(|| kind.path().into(), |id| format!("{}/{id}", kind.path())), Default::default()));
                }>{tr!(ui.cancel_changes)}</ActionButton><SaveButton kind="submit" disabled=Signal::derive(move || busy.get() || !loaded.get() || !editable) label=Signal::derive(move || if busy.get() { leptos_i18n::t_string!(i18n, ui.saving).to_owned() } else if id.is_some() { leptos_i18n::t_string!(i18n, ui.save_changes).to_owned() } else { leptos_i18n::t_string!(i18n, messages.create_arg0, arg0 = (kind.singular()).to_string()) })/></FormFooter>
            </form>
        </ManagementPage>
    }
}

fn detail(api: Api, token: String, kind: Kind, id: i64) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

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
        deletion.select(DeleteTarget {
            id,
            name: text(&data, "name"),
            path: format!("{}/{id}", kind.api()),
            revision: data["revision"].as_i64().map(|v| v.to_string()),
            description: leptos_i18n::t_string!(
                i18n,
                ui.this_removes_the_configuration_and_its_processing_history_already_dispatched_req
            ),
        });
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
                            leptos_i18n::t_string!(i18n, ui.connection_validated_successfully)
                        } else {
                            leptos_i18n::t_string!(
                                i18n,
                                ui.check_queued_results_refresh_automatically_every_five_seconds
                            )
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
        <ManagementPage><BackLink href=kind.path()>{move || kind.back_label()}</BackLink>
            <PageHeader title=Signal::derive(move || text(&value.get(), "name"))>
                <ActionButton on_click=move |_| refresh.update(|r| *r += 1) busy=Signal::derive(move || loading.get().to_string())>{tr!(ui.refresh)}</ActionButton>
                {editable.then(|| view! {
                    <ActionButton disabled=unavailable on_click=move |_| action("validate")>{move || if busy.get() { leptos_i18n::t_string!(i18n, ui.working) } else { leptos_i18n::t_string!(i18n, ui.validate_connection) }}</ActionButton>
                    {(!kind.webhook()).then(|| view! { <ActionButton disabled=Signal::derive(move || unavailable.get() || value.get()["running"] == true || value.get()["enabled"] != true) on_click=move |_| action("check")>{tr!(ui.check_mailbox_now)}</ActionButton> })}
                    <ResourceActions edit_href=format!("{}/{id}/edit",kind.path()) can_edit=editable disabled=unavailable on_delete=select_delete/>
                })}
            </PageHeader>{message.view()}
            <Card class="integration-status">
                <dl><dt>{tr!(ui.destination)}</dt><dd>{move || text(&value.get(), if kind.webhook() { "url" } else { "host" })}</dd>
                    <dt>{tr!(ui.enabled)}</dt><dd>{move || if value.get()[if kind.webhook() { "is_active" } else { "enabled" }] == true { leptos_i18n::t_string!(i18n, ui.yes) } else { leptos_i18n::t_string!(i18n, ui.no) }}</dd>
                    <dt>{tr!(ui.credential_configured)}</dt><dd>{move || if value.get()[if kind.webhook() { "has_secret" } else { "has_password" }] == true { leptos_i18n::t_string!(i18n, ui.yes) } else { leptos_i18n::t_string!(i18n, ui.no) }}</dd>
                    {(!kind.webhook()).then(|| view! {
                        <dt>{tr!(ui.account)}</dt><dd>{move || text(&value.get(), "username")}</dd>
                        <dt>{tr!(ui.folder)}</dt><dd>{move || text(&value.get(), "folder")}</dd>
                        <dt>{tr!(ui.transport)}</dt><dd>{move || leptos_i18n::t_string!(i18n, messages.transport_port, transport = if value.get()["tls"] == true { "TLS" } else { "STARTTLS" }, port = value.get()["port"].to_string())}</dd>
                        <dt>{tr!(ui.check_interval)}</dt><dd>{move || leptos_i18n::t_string!(i18n, messages.count_seconds, count = value.get()["frequency"].to_string())}</dd>
                        <dt>{tr!(ui.last_outcome)}</dt><dd>{move || text(&value.get(), "code")}</dd>
                        <dt>{tr!(ui.last_successful_check)}</dt><dd>{move || if value.get()["last_success"] == 0 { leptos_i18n::t_string!(i18n, ui.not_recorded).into() } else { instant(&value.get()["last_success"]) }}</dd>
                        <dt>{tr!(ui.next_check)}</dt><dd>{move || if value.get()["enabled"] != true { leptos_i18n::t_string!(i18n, ui.disabled).into() } else if value.get()["next_check"] == 0 { leptos_i18n::t_string!(i18n, ui.queued).into() } else { instant(&value.get()["next_check"]) }}</dd>
                    })}
                </dl>
                <Show when=move || value.get()["running"] == true><p role="status" aria-busy="true">{tr!(ui.checking_reporting_mailbox)}</p></Show>
            </Card>
            <Show when=move || !kind.webhook() && value.get()["code"] == "mailbox_uidvalidity_changed">
                <p role="alert">{tr!(ui.the_mail_server_changed_its_message_identifiers_processing_is_paused_to_protect)}</p>
                {editable.then(|| view! { <ActionButton disabled=unavailable on_click=move |_| resuming.set(true)>{tr!(ui.resume_with_new_arrivals)}</ActionButton> })}
            </Show>
            <Show when=move || resuming.get()>
                <DeleteConfirmation title=trs!(ui.resume_with_new_arrivals_2) description=trs!(ui.existing_messages_in_the_changed_folder_will_be_left_untouched_monitoring_starts) confirm_label=trs!(ui.resume_monitoring) busy=Signal::derive(move || busy.get()) on_confirm=Callback::new(move |()| action("resume")) on_cancel=Callback::new(move |()| resuming.set(false)) message=message/>
            </Show>
            <h2>{move || if kind.webhook() { leptos_i18n::t_string!(i18n, ui.recent_delivery_attempts) } else { leptos_i18n::t_string!(i18n, ui.recent_mailbox_checks) }}</h2>
            <DataTable label=trs!(ui.integration_history) headers=if kind.webhook() { vec![trs!(ui.started), trs!(ui.finished), trs!(ui.outcome), trs!(ui.http_status), trs!(ui.event), trs!(ui.attempt)] } else { vec![trs!(ui.started), trs!(ui.finished), trs!(ui.outcome), trs!(ui.matched), trs!(ui.rejected)] }>
                {move || array(&value.get(), "history").into_iter().map(|run| view! {
                    <tr><td class="data-value">{instant(&run["started"])}</td><td>{if run["finished"].is_null() { leptos_i18n::t_string!(i18n, ui.running).into() } else { instant(&run["finished"]) }}</td>
                        <td>{text(&run, "code")}</td>{if kind.webhook() { view! { <td>{run["status"].as_i64().map(|s| s.to_string()).unwrap_or_default()}</td><td>{run["event_id"].as_i64().map(|s| s.to_string()).unwrap_or_default()}</td><td>{run["attempt"].as_i64().map(|s| s.to_string()).unwrap_or_default()}</td> }.into_any() } else { view! { <td>{run["matched"].as_i64().unwrap_or(0)}</td><td>{run["rejected"].as_i64().unwrap_or(0)}</td> }.into_any() }}
                    </tr>
                }).collect_view()}
            </DataTable>
            <ResourceDeleteDialog deletion=deletion api=api token=token.get_value() on_deleted=deleted/>
        </ManagementPage>
    }
}

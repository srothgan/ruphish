//! Webhook administration and reporting mailbox resources share presentation primitives.
use crate::{
    campaigns::text, client::Api, feedback::Message, management_queries::ManagementQueries,
    resources::ResourceAccess, shared::components::*, time_display::instant,
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map};
use ruphish_ui::view_tasks::ViewTasks;
use ruphish_ui::{tr, trs};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    pub(crate) fn api(self) -> String {
        format!("/api{}", self.path())
    }
    pub(crate) fn webhook(self) -> bool {
        matches!(self, Self::Webhook)
    }
}

/// Confirmed server records stay typed; only editable form inputs use JSON.
#[derive(Clone, Debug)]
pub(crate) enum Record {
    Webhook(ruphish_contracts::integrations::Webhook),
    Mailbox(ruphish_contracts::integrations::Mailbox),
}
impl Record {
    pub fn id(&self) -> i64 {
        match self {
            Self::Webhook(v) => v.id,
            Self::Mailbox(v) => v.id,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Webhook(v) => &v.name,
            Self::Mailbox(v) => &v.name,
        }
    }
    pub fn revision(&self) -> i64 {
        match self {
            Self::Webhook(v) => v.revision,
            Self::Mailbox(v) => v.revision,
        }
    }
    fn destination(&self) -> &str {
        match self {
            Self::Webhook(v) => &v.url,
            Self::Mailbox(v) => &v.host,
        }
    }
    fn enabled(&self) -> bool {
        match self {
            Self::Webhook(v) => v.is_active,
            Self::Mailbox(v) => v.enabled,
        }
    }
    fn has_secret(&self) -> bool {
        match self {
            Self::Webhook(v) => v.has_secret,
            Self::Mailbox(v) => v.has_password,
        }
    }
    fn running(&self) -> bool {
        matches!(self, Self::Mailbox(v) if v.running)
    }
    fn code(&self) -> &str {
        match self {
            Self::Webhook(_) => "",
            Self::Mailbox(v) => &v.code,
        }
    }
    fn history(self) -> Vec<ruphish_contracts::integrations::Run> {
        match self {
            Self::Webhook(v) => v.history,
            Self::Mailbox(v) => v.history,
        }
    }
    fn edit_value(&self) -> Value {
        match self {
            Self::Webhook(v) => {
                json!({"name":v.name,"url":v.url,"is_active":v.is_active,"revision":v.revision})
            }
            Self::Mailbox(v) => {
                json!({"name":v.name,"enabled":v.enabled,"host":v.host,"port":v.port,"username":v.username,"tls":v.tls,"folder":v.folder,"restrict_domain":v.restrict_domain,"delete_reported":v.delete_reported,"frequency":v.frequency,"revision":v.revision})
            }
        }
    }
}

pub(super) fn list(api: Api, token: String, kind: Kind) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<ManagementQueries>();
    let list = queries.integrations.observe(move || kind);
    let confirmed = Signal::derive(move || list.data.with(Option::is_some));
    let search = RwSignal::new(String::new());
    let message = Message::new(String::new());
    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let deletion = ResourceDeletion::new();
    let deleted = Callback::new(move |id| {
        queries.integration_deleted(kind, id);
        message.status(leptos_i18n::t_string!(i18n, ui.resource_deleted).to_owned());
    });
    let visible = Signal::derive(move || {
        let needle = search.get().to_lowercase();
        list.data
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|row| row.name().to_lowercase().contains(&needle))
            .collect::<Vec<_>>()
    });
    view! {
        <ManagementPage>
            <PageHeader
                title=Signal::derive(move || kind.title().to_owned())
                subtitle=Signal::derive(move || {
                    (if kind.webhook() {
                        leptos_i18n::t_string!(
                            i18n, ui.administrator_managed_event_notifications_enabled_endpoints_receive_events_acros
                        )
                    } else {
                        leptos_i18n::t_string!(
                            i18n, ui.owner_scoped_imap_resources_that_correlate_reported_campaign_messages_without_re
                        )
                    })
                        .to_owned()
                })
            >
                <ActionButton on_click=move |_| list.refresh.run(()) disabled=list.fetching>
                    {tr!(ui.refresh)}
                </ActionButton>
                {can_edit
                    .then(|| {
                        view! {
                            <ActionLink
                                style=ButtonStyle::Primary
                                href=format!("{}/new", kind.path())
                            >
                                {leptos_i18n::t!(
                                    i18n, messages.create_arg0, arg0 = (kind.singular()).to_string()
                                )}
                            </ActionLink>
                        }
                    })}
            </PageHeader>
            {message.view()}
            <ResourceDeleteDialog deletion=deletion api=api token=token on_deleted=deleted />
            <ListToolbar
                search=search
                count=Signal::derive(move || visible.with(Vec::len))
                count_known=confirmed
                noun=Signal::derive(move || kind.plural().to_owned())
            />
            <QueryStatus
                fetching=list.fetching
                has_data=confirmed
                error=list.error
                noun=Signal::derive(move || kind.plural().to_owned())
                refresh=list.refresh
            />
            <DataTable
                label=Signal::derive(move || kind.title().to_owned())
                empty=Signal::derive(move || visible.with(Vec::is_empty))
                headers=vec![
                    trs!(ui.name),
                    trs!(ui.destination),
                    trs!(ui.enabled),
                    trs!(ui.status),
                    trs!(ui.actions),
                ]
            >
                {move || {
                    visible
                        .get()
                        .into_iter()
                        .map(|row| {
                            let href = format!("{}/{}", kind.path(), row.id());
                            let target = DeleteTarget {
                                id: row.id(),
                                name: row.name().to_owned(),
                                path: format!("{}/{}", kind.api(), row.id()),
                                revision: Some(row.revision().to_string()),
                                description: leptos_i18n::t_string!(
                                    i18n, ui.this_removes_the_configuration_and_its_processing_history_already_dispatched_req
                                ),
                            };
                            view! {
                                <tr>
                                    <th scope="row">
                                        <a href=href.clone()>{row.name().to_owned()}</a>
                                    </th>
                                    <td>{row.destination().to_owned()}</td>
                                    <td>
                                        {if row.enabled() {
                                            leptos_i18n::t_string!(i18n, ui.yes)
                                        } else {
                                            leptos_i18n::t_string!(i18n, ui.no)
                                        }}
                                    </td>
                                    <td>
                                        {if row.running() {
                                            leptos_i18n::t_string!(i18n, ui.checking).into()
                                        } else {
                                            row.code().to_owned()
                                        }}
                                    </td>
                                    <td>
                                        <ResourceActions
                                            view_href=href.clone()
                                            edit_href=format!("{href}/edit")
                                            can_edit=can_edit
                                            size=ControlSize::Small
                                            disabled=deletion.busy
                                            on_delete=Callback::new(move |()| {
                                                deletion.select(target.clone())
                                            })
                                        />
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()
                }}
            </DataTable>
            <ListEmptyState
                ready=Signal::derive(move || {
                    confirmed.get() && !list.fetching.get() && list.error.get().is_none()
                })
                count=Signal::derive(move || visible.with(Vec::len))
                search=search
                title=Signal::derive(move || kind.empty_title().to_owned())
                guidance=Signal::derive(move || kind.empty_guidance().to_owned())
            />
        </ManagementPage>
    }
}

pub(super) fn page(api: Api, token: String, kind: Kind, editing: bool) -> impl IntoView {
    let params = use_params_map();
    let token = StoredValue::new(token);
    view! {
        <For
            each=move || vec![params.with(|p| p.get("id")).unwrap_or_default()]
            key=|id| id.clone()
            children=move |id| {
                if id == "new" {
                    form(api, token.get_value(), kind, None).into_any()
                } else if let Ok(id) = id.parse::<i64>() && id > 0 {
                    if editing {
                        form(api, token.get_value(), kind, Some(id)).into_any()
                    } else {
                        detail(api, token.get_value(), kind, id).into_any()
                    }
                } else {
                    view! {
                        <NotFoundPage
                            back_href=kind.path()
                            back_label=Signal::derive(move || kind.back_label().to_owned())
                        />
                    }
                        .into_any()
                }
            }
        />
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
        <Field label=label>
            <TextInput
                input_type=input_type
                value=Signal::derive(move || {
                    let value = draft.get();
                    value[key]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            value[key].as_u64().map(|n| n.to_string()).unwrap_or_default()
                        })
                })
                on_input=move |event| {
                    let value = event_target_value(&event);
                    draft
                        .update(|draft| {
                            draft[key] = if input_type == "number" {
                                value.parse::<u32>().map_or(Value::Null, |n| json!(n))
                            } else {
                                json!(value)
                            };
                        });
                }
            />
        </Field>
    }
}

fn checkbox(draft: RwSignal<Value>, key: &'static str, label: Signal<String>) -> impl IntoView {
    view! {
        <Checkbox
            label=label
            checked=Signal::derive(move || draft.get()[key].as_bool().unwrap_or(false))
            on_toggle=Callback::new(move |value| draft.update(|draft| draft[key] = json!(value)))
        />
    }
}

fn form(api: Api, token: String, kind: Kind, id: Option<i64>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<ManagementQueries>();
    let initial = if kind.webhook() {
        json!({"name":"","url":"","is_active":true,"revision":null})
    } else {
        json!({"name":"","enabled":true,"host":"","port":993,"username":"","tls":true,"folder":"INBOX","restrict_domain":"","delete_reported":false,"frequency":60,"revision":null})
    };
    let draft = RwSignal::new(initial.clone());
    let baseline = RwSignal::new(initial);
    let loaded = RwSignal::new(id.is_none());
    let load_error = RwSignal::new(None::<ruphish_ui::api_error::ApiError>);
    let loading = RwSignal::new(id.is_some());
    let load_failed = Signal::derive(move || load_error.get().is_some());
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

            spawn_local(async move {
                match ManagementQueries::read_integration(api, kind, id).await {
                    Ok(value) => {
                        load_error.set(None);
                        has_saved_secret.set(value.has_secret());
                        let value = value.edit_value();
                        baseline.set(value.clone());
                        draft.set(value);
                        loaded.set(true);
                        message.clear();
                    }
                    Err(error) => {
                        load_error.set(Some(error));
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
                .and_then(|(saved, revision)| {
                    ManagementQueries::validate_integration(api, kind, saved, id)
                        .map(|saved| (saved, revision))
                        .map_err(|e| e.message)
                }) {
                Ok((saved, _)) => {
                    queries.integration_saved(kind, saved.id());
                    baseline.set(draft.get_untracked());
                    busy.set(false);
                    navigate.with_value(|navigate| {
                        navigate(
                            &format!("{}/{}", kind.path(), saved.id()),
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
    let content = view! {
        <ManagementPage>
            <BackLink href=kind.path()>{move || kind.back_label()}</BackLink>
            <PageHeader
                status=true
                title=Signal::derive(move || {
                    let name = text(&draft.get(), "name");
                    if !name.is_empty() {
                        name
                    } else if kind.webhook() {
                        leptos_i18n::t_string!(i18n, ui.new_webhook).to_owned()
                    } else {
                        leptos_i18n::t_string!(i18n, ui.new_reporting_mailbox).to_owned()
                    }
                })
                subtitle=Signal::derive(move || {
                    if busy.get() {
                        leptos_i18n::t_string!(i18n, ui.saving)
                    } else if loading.get() {
                        leptos_i18n::t_string!(i18n, ui.loading_resource)
                    } else if load_failed.get() {
                        leptos_i18n::t_string!(i18n, ui.resource_could_not_be_loaded)
                    } else if !editable {
                        leptos_i18n::t_string!(i18n, ui.read_only_access)
                    } else if draft.get() != baseline.get() {
                        leptos_i18n::t_string!(i18n, ui.unsaved_changes)
                    } else if id.is_none() {
                        leptos_i18n::t_string!(i18n, ui.not_saved_yet)
                    } else {
                        leptos_i18n::t_string!(i18n, ui.no_unsaved_changes)
                    }
                        .to_owned()
                })
            />
            <p class="resource-usage">
                {move || {
                    if kind.webhook() {
                        leptos_i18n::t_string!(i18n, ui.webhook_configuration_context)
                    } else {
                        leptos_i18n::t_string!(i18n, ui.mailbox_configuration_context)
                    }
                }}
            </p>
            {message.view()}
            <form on:submit=save>
                <FormFields disabled=Signal::derive(move || {
                    busy.get() || !loaded.get() || !editable
                })>
                    {field(draft, "name", trs!(ui.name), "text")}
                    {if kind.webhook() {
                        view! {
                            <section class="resource-section">
                                <h2>{tr!(ui.connection)}</h2>
                                {field(draft, "url", trs!(ui.endpoint_url), "url")}
                                <SecretField
                                    label=trs!(ui.signing_secret)
                                    saved_hint=trs!(ui.leave_empty_to_keep_the_saved_secret)
                                    has_saved_value=has_saved_secret
                                    value=Signal::derive(move || text(&draft.get(), "secret"))
                                    on_input=Callback::new(move |value: String| {
                                        draft.update(|draft| draft["secret"] = json!(value))
                                    })
                                />
                            </section>
                            <section class="resource-section">
                                <h2>{tr!(ui.delivery)}</h2>
                                {checkbox(draft, "is_active", trs!(ui.enabled))}
                                <p>{tr!(ui.webhook_delivery_guidance)}</p>
                            </section>
                        }
                            .into_any()
                    } else {
                        view! {
                            <section class="resource-section">
                                <h2>{tr!(ui.connection)}</h2>
                                <div class="resource-columns">
                                    {field(draft, "host", trs!(ui.imap_hostname), "text")}
                                    {field(draft, "port", trs!(ui.port), "number")}
                                </div>
                                {checkbox(
                                    draft,
                                    "tls",
                                    trs!(ui.use_implicit_tls_otherwise_starttls_is_required),
                                )}
                            </section>
                            <section class="resource-section">
                                <h2>{tr!(ui.service_account)}</h2>
                                <div class="resource-columns">
                                    {field(
                                        draft,
                                        "username",
                                        trs!(ui.service_account_username),
                                        "text",
                                    )}
                                    <SecretField
                                        label=trs!(ui.service_account_password)
                                        saved_hint=trs!(ui.leave_empty_to_keep_the_saved_password)
                                        has_saved_value=has_saved_secret
                                        value=Signal::derive(move || text(&draft.get(), "password"))
                                        on_input=Callback::new(move |value: String| {
                                            draft.update(|draft| draft["password"] = json!(value))
                                        })
                                    />
                                </div>
                            </section>
                            <section class="resource-section">
                                <h2>{tr!(ui.monitoring)}</h2>
                                <div class="resource-columns">
                                    {field(draft, "folder", trs!(ui.folder), "text")}
                                    {field(
                                        draft,
                                        "frequency",
                                        trs!(ui.check_interval_in_seconds_minimum_30),
                                        "number",
                                    )}
                                    {field(
                                        draft,
                                        "restrict_domain",
                                        trs!(ui.optional_sender_domain_filter),
                                        "text",
                                    )}
                                </div>
                                {checkbox(
                                    draft,
                                    "delete_reported",
                                    trs!(ui.delete_successfully_matched_messages_after_processing),
                                )}
                                {checkbox(draft, "enabled", trs!(ui.enabled))}
                                <p>{tr!(ui.mailbox_processing_guidance)}</p>
                            </section>
                        }
                            .into_any()
                    }}
                </FormFields>
                <FormFooter spread=true>
                    <ActionButton
                        style=ButtonStyle::Ghost
                        disabled=busy
                        on_click=move |_| {
                            draft.set(baseline.get_untracked());
                            navigate
                                .with_value(|navigate| navigate(
                                    &id
                                        .map_or_else(
                                            || kind.path().into(),
                                            |id| format!("{}/{id}", kind.path()),
                                        ),
                                    Default::default(),
                                ));
                        }
                    >
                        {tr!(ui.cancel_changes)}
                    </ActionButton>
                    <SaveButton
                        kind="submit"
                        disabled=Signal::derive(move || {
                            busy.get() || !loaded.get() || !editable
                        })
                        label=Signal::derive(move || {
                            if busy.get() {
                                leptos_i18n::t_string!(i18n, ui.saving).to_owned()
                            } else if id.is_some() {
                                leptos_i18n::t_string!(i18n, ui.save_changes).to_owned()
                            } else {
                                leptos_i18n::t_string!(
                                    i18n, messages.create_arg0, arg0 = (kind.singular()).to_string()
                                )
                            }
                        })
                    />
                </FormFooter>
            </form>
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=load_error
            has_data=Signal::derive(move || loaded.get())
            fetching=loading
            retry=Callback::new(move |()| refresh.update(|n| *n += 1))
            back_href=kind.path()
            back_label=Signal::derive(move || kind.back_label().to_owned())
        >
            {content}
        </ReadBoundary>
    }
}

fn detail(api: Api, token: String, kind: Kind, id: i64) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<ManagementQueries>();
    let read = queries.integration.observe(move || (kind, id));
    let confirmed = Signal::derive(move || read.data.with(Option::is_some));
    let value = read.data;
    let mailbox = Signal::derive(move || {
        value.get().and_then(|v| match v {
            Record::Mailbox(v) => Some(v),
            Record::Webhook(_) => None,
        })
    });
    let busy = RwSignal::new(false);
    let deletion = ResourceDeletion::new();
    let unavailable = Signal::derive(move || busy.get() || deletion.busy.get() || !confirmed.get());
    let resuming = RwSignal::new(false);
    let message = Message::new(String::new());
    let tasks = ViewTasks::default();
    let token = StoredValue::new(token);
    let navigate = StoredValue::new(use_navigate());
    let deleted = Callback::new(move |_| {
        queries.integration_deleted(kind, id);
        navigate.with_value(|navigate| navigate(kind.path(), Default::default()))
    });
    let select_delete = Callback::new(move |()| {
        if unavailable.get_untracked() {
            return;
        }
        let Some(data) = value.get_untracked() else {
            return;
        };
        deletion.select(DeleteTarget {
            id,
            name: data.name().to_owned(),
            path: format!("{}/{id}", kind.api()),
            revision: Some(data.revision().to_string()),
            description: leptos_i18n::t_string!(
                i18n,
                ui.this_removes_the_configuration_and_its_processing_history_already_dispatched_req
            ),
        });
    });
    let editable = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let interval = set_interval_with_handle(
        move || {
            if !read
                .failure
                .with_untracked(|e| e.as_ref().is_some_and(|e| e.is_terminal()))
                && !read.fetching.get_untracked()
                && !busy.get_untracked()
                && !deletion.busy.get_untracked()
            {
                read.refresh.run(());
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
                .then(|| json!({"revision":value.get_untracked().map(|v| v.revision())}));
            let result = api
                .resource("POST", &path, &token.get_value(), body.as_ref(), None)
                .await
                .and_then(|(saved, _)| {
                    ManagementQueries::validate_integration(api, kind, saved, Some(id))
                        .map_err(|e| e.message)
                });
            match result {
                Ok(saved) => {
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
                    queries.integration_command(kind, id, operation, saved);
                    resuming.set(false);
                }
                Err(error) => message.error(error),
            }
            busy.set(false);
        });
    };
    let content = view! {
        <ManagementPage>
            <BackLink href=kind.path()>{move || kind.back_label()}</BackLink>
            <PageHeader title=Signal::derive(move || {
                value.get().map(|v| v.name().to_owned()).unwrap_or_default()
            })>
                <ActionButton on_click=move |_| read.refresh.run(()) disabled=read.fetching>
                    {tr!(ui.refresh)}
                </ActionButton>
                {editable
                    .then(|| {
                        view! {
                            <ActionButton disabled=unavailable on_click=move |_| action("validate")>
                                {move || {
                                    if busy.get() {
                                        leptos_i18n::t_string!(i18n, ui.working)
                                    } else {
                                        leptos_i18n::t_string!(i18n, ui.validate_connection)
                                    }
                                }}
                            </ActionButton>
                            {(!kind.webhook())
                                .then(|| {
                                    view! {
                                        <ActionButton
                                            disabled=Signal::derive(move || {
                                                unavailable.get()
                                                    || value.get().is_some_and(|v| v.running())
                                                    || !value.get().is_some_and(|v| v.enabled())
                                            })
                                            on_click=move |_| action("check")
                                        >
                                            {tr!(ui.check_mailbox_now)}
                                        </ActionButton>
                                    }
                                })}
                            <ResourceActions
                                edit_href=format!("{}/{id}/edit", kind.path())
                                can_edit=editable
                                disabled=unavailable
                                on_delete=select_delete
                            />
                        }
                    })}
            </PageHeader>
            {message.view()}
            <QueryStatus
                fetching=read.fetching
                has_data=confirmed
                error=read.error
                noun=Signal::derive(move || kind.singular().to_owned())
                refresh=read.refresh
            />
            <Show when=move || confirmed.get()>
                <Card class="integration-status">
                    <dl>
                        <dt>{tr!(ui.destination)}</dt>
                        <dd>
                            {move || {
                                value.get().map(|v| v.destination().to_owned()).unwrap_or_default()
                            }}
                        </dd>
                        <dt>{tr!(ui.enabled)}</dt>
                        <dd>
                            {move || {
                                if value.get().is_some_and(|v| v.enabled()) {
                                    leptos_i18n::t_string!(i18n, ui.yes)
                                } else {
                                    leptos_i18n::t_string!(i18n, ui.no)
                                }
                            }}
                        </dd>
                        <dt>{tr!(ui.credential_configured)}</dt>
                        <dd>
                            {move || {
                                if value.get().is_some_and(|v| v.has_secret()) {
                                    leptos_i18n::t_string!(i18n, ui.yes)
                                } else {
                                    leptos_i18n::t_string!(i18n, ui.no)
                                }
                            }}
                        </dd>
                        {(!kind.webhook())
                            .then(|| {
                                view! {
                                    <dt>{tr!(ui.account)}</dt>
                                    <dd>
                                        {move || {
                                            mailbox.get().map(|v| v.username).unwrap_or_default()
                                        }}
                                    </dd>
                                    <dt>{tr!(ui.folder)}</dt>
                                    <dd>
                                        {move || {
                                            mailbox.get().map(|v| v.folder).unwrap_or_default()
                                        }}
                                    </dd>
                                    <dt>{tr!(ui.transport)}</dt>
                                    <dd>
                                        {move || {
                                            leptos_i18n::t_string!(
                                                i18n, messages.transport_port, transport = if mailbox.get().is_some_and(|v| v.tls) { "TLS" } else { "STARTTLS" }, port = mailbox.get().map(|v| v.port.to_string()).unwrap_or_default()
                                            )
                                        }}
                                    </dd>
                                    <dt>{tr!(ui.check_interval)}</dt>
                                    <dd>
                                        {move || {
                                            leptos_i18n::t_string!(
                                                i18n, messages.count_seconds, count = mailbox.get().map(|v| v.frequency.to_string()).unwrap_or_default()
                                            )
                                        }}
                                    </dd>
                                    <dt>{tr!(ui.last_outcome)}</dt>
                                    <dd>
                                        {move || {
                                            value.get().map(|v| v.code().to_owned()).unwrap_or_default()
                                        }}
                                    </dd>
                                    <dt>{tr!(ui.last_successful_check)}</dt>
                                    <dd>
                                        {move || {
                                            if mailbox.get().is_some_and(|v| v.last_success == 0) {
                                                leptos_i18n::t_string!(i18n, ui.not_recorded).into()
                                            } else {
                                                instant(&json!(mailbox.get().map(|v| v.last_success)))
                                            }
                                        }}
                                    </dd>
                                    <dt>{tr!(ui.next_check)}</dt>
                                    <dd>
                                        {move || {
                                            if !value.get().is_some_and(|v| v.enabled()) {
                                                leptos_i18n::t_string!(i18n, ui.disabled).into()
                                            } else if mailbox.get().is_some_and(|v| v.next_check == 0) {
                                                leptos_i18n::t_string!(i18n, ui.queued).into()
                                            } else {
                                                instant(&json!(mailbox.get().map(|v| v.next_check)))
                                            }
                                        }}
                                    </dd>
                                }
                            })}
                    </dl>
                    <Show when=move || value.get().is_some_and(|v| v.running())>
                        <p role="status" aria-busy="true">
                            {tr!(ui.checking_reporting_mailbox)}
                        </p>
                    </Show>
                </Card>
                <Show when=move || {
                    !kind.webhook()
                        && value.get().is_some_and(|v| v.code() == "mailbox_uidvalidity_changed")
                }>
                    <p role="alert">
                        {tr!(
                            ui.the_mail_server_changed_its_message_identifiers_processing_is_paused_to_protect
                        )}
                    </p>
                    {editable
                        .then(|| {
                            view! {
                                <ActionButton
                                    disabled=unavailable
                                    on_click=move |_| resuming.set(true)
                                >
                                    {tr!(ui.resume_with_new_arrivals)}
                                </ActionButton>
                            }
                        })}
                </Show>
                <Show when=move || resuming.get()>
                    <DeleteConfirmation
                        title=trs!(ui.resume_with_new_arrivals_2)
                        description=trs!(
                            ui.existing_messages_in_the_changed_folder_will_be_left_untouched_monitoring_starts
                        )
                        confirm_label=trs!(ui.resume_monitoring)
                        busy=Signal::derive(move || busy.get())
                        on_confirm=Callback::new(move |()| action("resume"))
                        on_cancel=Callback::new(move |()| resuming.set(false))
                        message=message
                    />
                </Show>
                <h2>
                    {move || {
                        if kind.webhook() {
                            leptos_i18n::t_string!(i18n, ui.recent_delivery_attempts)
                        } else {
                            leptos_i18n::t_string!(i18n, ui.recent_mailbox_checks)
                        }
                    }}
                </h2>
                <DataTable
                    label=trs!(ui.integration_history)
                    headers=if kind.webhook() {
                        vec![
                            trs!(ui.started),
                            trs!(ui.finished),
                            trs!(ui.outcome),
                            trs!(ui.http_status),
                            trs!(ui.event),
                            trs!(ui.attempt),
                        ]
                    } else {
                        vec![
                            trs!(ui.started),
                            trs!(ui.finished),
                            trs!(ui.outcome),
                            trs!(ui.matched),
                            trs!(ui.rejected),
                        ]
                    }
                >
                    {move || {
                        value
                            .get()
                            .map(Record::history)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|run| {
                                view! {
                                    <tr>
                                        <td class="data-value">{instant(&json!(run.started))}</td>
                                        <td>
                                            {if run.finished.is_none() {
                                                leptos_i18n::t_string!(i18n, ui.running).into()
                                            } else {
                                                instant(&json!(run.finished))
                                            }}
                                        </td>
                                        <td>{run.code}</td>
                                        {if kind.webhook() {
                                            view! {
                                                <td>
                                                    {run.status.map(|s| s.to_string()).unwrap_or_default()}
                                                </td>
                                                <td>
                                                    {run.event_id.map(|s| s.to_string()).unwrap_or_default()}
                                                </td>
                                                <td>
                                                    {run.attempt.map(|s| s.to_string()).unwrap_or_default()}
                                                </td>
                                            }
                                                .into_any()
                                        } else {
                                            view! {
                                                <td>{run.matched}</td>
                                                <td>{run.rejected}</td>
                                            }
                                                .into_any()
                                        }}
                                    </tr>
                                }
                            })
                            .collect_view()
                    }}
                </DataTable>
            </Show>
            <ResourceDeleteDialog
                deletion=deletion
                api=api
                token=token.get_value()
                on_deleted=deleted
            />
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=read.failure
            has_data=Signal::derive(move || read.data.with(Option::is_some))
            fetching=read.fetching
            retry=read.refresh
            back_href=kind.path()
            back_label=Signal::derive(move || kind.back_label().to_owned())
        >
            {content}
        </ReadBoundary>
    }
}

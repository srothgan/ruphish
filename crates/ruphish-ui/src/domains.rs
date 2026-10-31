//! Domain management uses the shared resource shell, forms and confirmation dialog.
use crate::{
    client::Api, feedback::Message, management_queries::ManagementQueries,
    resources::ResourceAccess, shared::components::*,
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_contracts::domains::{Domain, Edit};
use ruphish_domain::reporting::SortDirection;
use ruphish_ui::table_state::Value as SortValue;
use ruphish_ui::{tr, trs};
use serde_json::json;

fn editable() -> bool {
    use_context::<ResourceAccess>().is_some_and(|v| v.0)
}
fn state(d: &Domain) -> &'static str {
    let i18n = ruphish_ui::localization::use_i18n();

    match d.verification.status.as_str() {
        "unsupported" => leptos_i18n::t_string!(i18n, ui.unavailable),
        "not_required" => leptos_i18n::t_string!(i18n, ui.verification_not_required),
        "verified" => leptos_i18n::t_string!(i18n, ui.verified),
        "expired" => leptos_i18n::t_string!(i18n, ui.expired),
        "failed" => leptos_i18n::t_string!(i18n, ui.failed),
        _ => leptos_i18n::t_string!(i18n, ui.not_verified),
    }
}
fn status_tone(status: &str) -> StatusTone {
    match status {
        "verified" | "passed" => StatusTone::Positive,
        "failed" => StatusTone::Negative,
        "expired" | "timed_out" => StatusTone::Warning,
        _ => StatusTone::Neutral,
    }
}
fn instant(seconds: i64) -> String {
    let i18n = ruphish_ui::localization::use_i18n();

    if seconds == 0 {
        leptos_i18n::t_string!(i18n, ui.not_recorded).into()
    } else {
        crate::time_display::instant(&json!(seconds))
    }
}
fn outcome(value: &str) -> &'static str {
    let i18n = ruphish_ui::localization::use_i18n();

    match value {
        "passed" => leptos_i18n::t_string!(i18n, ui.passed),
        "failed" => leptos_i18n::t_string!(i18n, ui.failed),
        "timed_out" => leptos_i18n::t_string!(i18n, ui.timed_out),
        _ => leptos_i18n::t_string!(i18n, ui.not_recorded),
    }
}
fn check_result(d: &Domain, kind: &str, passed: bool) -> &'static str {
    let i18n = ruphish_ui::localization::use_i18n();

    if d.checked_at == 0 {
        return leptos_i18n::t_string!(i18n, ui.not_checked);
    }
    if passed {
        return leptos_i18n::t_string!(i18n, ui.passed);
    }
    let code = d.code.as_str();
    let control_failed = matches!(
        code,
        "domain_txt_unavailable" | "domain_txt_mismatch" | "domain_challenge_expired"
    );
    let dns_failed = matches!(
        code,
        "domain_dns_unavailable" | "domain_dns_address_limit" | "domain_destination_denied"
    );
    match kind {
        "control" if control_failed => leptos_i18n::t_string!(i18n, ui.failed),
        _ if control_failed => leptos_i18n::t_string!(i18n, ui.not_checked),
        "DNS" if dns_failed => leptos_i18n::t_string!(i18n, ui.failed),
        "HTTPS" | "routing" if dns_failed => leptos_i18n::t_string!(i18n, ui.not_checked),
        "HTTPS" if code == "domain_https_unavailable" => leptos_i18n::t_string!(i18n, ui.failed),
        "routing" if code == "domain_https_unavailable" => {
            leptos_i18n::t_string!(i18n, ui.not_checked)
        }
        "routing"
            if matches!(
                code,
                "domain_routing_mismatch" | "domain_routing_unavailable"
            ) =>
        {
            leptos_i18n::t_string!(i18n, ui.failed)
        }
        _ => leptos_i18n::t_string!(i18n, ui.not_confirmed),
    }
}
pub(super) fn list(api: Api, token: String) -> impl IntoView {
    let sorting = TableSorting::new("domains", "name", SortDirection::Asc);
    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<ManagementQueries>();
    let list = queries.domains.observe(|| ());
    let confirmed = Signal::derive(move || list.data.with(Option::is_some));
    let search = RwSignal::new(String::new());
    let message = Message::new(String::new());
    let can_edit = editable();
    let deletion = ResourceDeletion::new();
    let deleted = Callback::new(move |id| {
        queries.domain_deleted(id);
        message.status(leptos_i18n::t_string!(i18n, ui.domain_deleted).to_owned());
    });
    let visible = Signal::derive(move || {
        let query = search.get().to_lowercase();
        list.data
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|d| {
                format!("{} {}", d.name, d.hostname)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect::<Vec<_>>()
    });
    view! {
        <ManagementPage>
            <PageHeader title=trs!(ui.domains) subtitle=trs!(ui.domain_destinations)>
                <RefreshButton fetching=list.fetching on_refresh=list.refresh />
                {can_edit
                    .then(|| {
                        view! {
                            <ActionLink style=ButtonStyle::Primary href="/domains/new">
                                {tr!(ui.create_domain)}
                            </ActionLink>
                        }
                    })}
            </PageHeader>
            {message.view()}
            <ResourceDeleteDialog deletion=deletion api=api token=token on_deleted=deleted />
            <QueryBoundary
                fetching=list.fetching
                has_data=confirmed
                error=list.error
                noun=trs!(ui.domains_2)
                refresh=list.refresh
            >
                <QueryControls slot:controls>
                    <ListToolbar
                        search=search
                        count=Signal::derive(move || visible.with(Vec::len))
                        count_known=confirmed
                        label=trs!(ui.search_by_name_or_hostname)
                        noun=trs!(ui.domains_2)
                    />
                </QueryControls>
                <DataTable
                    sorting=sorting
                    columns=vec!["name", "origin", "status", "enabled", "campaigns", ""]
                    label=trs!(ui.domains)
                    empty=Signal::derive(move || visible.with(Vec::is_empty))
                    headers=vec![
                        trs!(ui.name),
                        trs!(ui.public_origin),
                        trs!(ui.status),
                        trs!(ui.enabled),
                        trs!(ui.campaigns),
                        trs!(ui.actions),
                    ]
                >
                    {move || {
                        sorting
                            .rows(
                                visible.get(),
                                |row, key| match key {
                                    "origin" => row.origin.clone().into(),
                                    "status" => state(row).into(),
                                    "enabled" => row.enabled.into(),
                                    "campaigns" => row.campaigns.into(),
                                    _ => row.name.clone().into(),
                                },
                                |row| row.id.into(),
                            )
                            .into_iter()
                            .map(|d| {
                                let id = d.id;
                                let label = state(&d);
                                view! {
                                    <tr>
                                        <td>
                                            <a href=format!("/domains/{id}")>{d.name.clone()}</a>
                                        </td>
                                        <td>{d.origin.clone()}</td>
                                        <td>
                                            <StatusBadge tone=status_tone(
                                                &d.verification.status,
                                            )>{label}</StatusBadge>
                                        </td>
                                        <td>
                                            {move || {
                                                if d.enabled {
                                                    leptos_i18n::t_string!(i18n, ui.yes)
                                                } else {
                                                    leptos_i18n::t_string!(i18n, ui.no)
                                                }
                                            }}
                                        </td>
                                        <td>{d.campaigns}</td>
                                        <td>
                                            <ResourceActions
                                                view_href=format!("/domains/{id}")
                                                edit_href=format!("/domains/{id}/edit")
                                                can_edit=can_edit
                                                size=ControlSize::Small
                                                disabled=deletion.busy
                                                delete_disabled={d.campaigns > 0}
                                                delete_reason=leptos_i18n::t_string!(
                                                    i18n, ui.domains_used_by_campaigns_cannot_be_deleted
                                                )
                                                on_delete=Callback::new(move |()| {
                                                    deletion
                                                        .select(DeleteTarget {
                                                            id,
                                                            name: d.name.clone(),
                                                            path: format!("/api/domains/{id}"),
                                                            revision: Some(format!("\"{}\"", d.revision)),
                                                            description: leptos_i18n::t_string!(
                                                                i18n, ui.only_unused_domains_can_be_deleted_this_removes_the_domain_configuration_and_its
                                                            ),
                                                        })
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
                    ready=Signal::derive(move || { confirmed.get() && list.error.get().is_none() })
                    count=Signal::derive(move || visible.with(Vec::len))
                    search=search
                    title=trs!(ui.no_domains_found)
                    guidance=trs!(
                        ui.add_a_hostname_prove_dns_control_and_check_routing_and_https_before_using_it_in
                    )
                />
            </QueryBoundary>
        </ManagementPage>
    }
}
pub(super) fn detail(api: Api, token: String, editing: bool) -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    view! {
        <For
            each=move || vec![params.with(|p| p.get("id")).unwrap_or_default()]
            key=|s| s.clone()
            children=move |s| {
                if s == "new" {
                    form(api, token.clone(), None).into_any()
                } else if let Ok(id) = s.parse::<i64>() && id > 0 {
                    if editing {
                        form(api, token.clone(), Some(id)).into_any()
                    } else {
                        show(api, token.clone(), id).into_any()
                    }
                } else {
                    view! {
                        <NotFoundPage back_href="/domains" back_label=trs!(ui.back_to_domains) />
                    }
                        .into_any()
                }
            }
        />
    }
}
fn form(api: Api, token: String, id: Option<i64>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<ManagementQueries>();
    let name = RwSignal::new(String::new());
    let origin = RwSignal::new(String::new());
    let enabled = RwSignal::new(true);
    let revision = RwSignal::new(None::<i64>);
    let baseline = RwSignal::new((String::new(), String::new(), true));
    let loaded = RwSignal::new(id.is_none());
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let token = StoredValue::new(token);
    let can_edit = editable();
    let dirty = Memo::new(move |_| {
        loaded.get() && baseline.get() != (name.get(), origin.get(), enabled.get())
    });
    crate::navigation::protect_draft(move || dirty.get() || busy.get());
    let reload = RwSignal::new(0u64);
    let load_error = RwSignal::new(None::<ruphish_ui::api_error::ApiError>);
    let loading = RwSignal::new(id.is_some());
    if let Some(id) = id {
        Effect::new(move |_| {
            reload.get();
            loading.set(true);
            message.clear();
            spawn_local(async move {
                match ManagementQueries::read_domain(api, id).await {
                    Ok(d) => {
                        load_error.set(None);
                        baseline.set((d.name.clone(), d.origin.clone(), d.enabled));
                        name.set(d.name);
                        origin.set(d.origin);
                        enabled.set(d.enabled);
                        revision.set(Some(d.revision));
                        loaded.set(true)
                    }
                    Err(e) => load_error.set(Some(e)),
                }
                loading.set(false);
            });
        });
    }
    let navigate = leptos_router::hooks::use_navigate();
    let cancel = navigate.clone();
    let validation = FormValidation::new(move || {
        ruphish_domain::domains::validate_edit(&name.get(), &origin.get())
            .err()
            .unwrap_or_default()
    });
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() || !loaded.get_untracked() || !can_edit {
            return;
        }
        if !validation.check() {
            return;
        }
        let ticket = validation.ticket();
        busy.set(true);
        let navigate = navigate.clone();
        spawn_local(async move {
            let edit = Edit {
                name: name.get_untracked(),
                origin: origin.get_untracked(),
                enabled: enabled.get_untracked(),
                revision: revision.get_untracked(),
            };
            let path = id
                .map(|id| format!("/api/domains/{id}"))
                .unwrap_or_else(|| "/api/domains/".into());
            match api
                .resource_response(
                    if id.is_some() { "PUT" } else { "POST" },
                    &path,
                    &token.get_value(),
                    Some(&json!(edit)),
                    None,
                )
                .await
                .and_then(|(v, _)| ManagementQueries::validate_domain(api, v, id))
            {
                Ok(d) => {
                    queries.domain_saved(d.id);
                    #[cfg(feature = "demo")]
                    api.notifications.show(if id.is_none() {
                        DemoNotice::Created
                    } else {
                        DemoNotice::Saved
                    });
                    loaded.set(false);
                    busy.set(false);
                    navigate(&format!("/domains/{}", d.id), Default::default());
                }
                Err(e) => {
                    busy.set(false);
                    validation.server(ticket, &e);
                    message.error(e.message)
                }
            }
        });
    };
    let content = view! {
        <ManagementPage>
            <BackLink href="/domains">{tr!(ui.back_to_domains)}</BackLink>
            <PageHeader
                title=Signal::derive(move || {
                    (if id.is_some() {
                        leptos_i18n::t_string!(i18n, ui.edit_domain)
                    } else {
                        leptos_i18n::t_string!(i18n, ui.create_domain)
                    })
                        .to_owned()
                })
                subtitle=Signal::derive(move || {
                    if dirty.get() {
                        leptos_i18n::t_string!(i18n, ui.unsaved_changes)
                    } else {
                        leptos_i18n::t_string!(i18n, ui.domain_destinations)
                    }
                        .to_owned()
                })
            ></PageHeader>
            <QueryBoundary
                fetching=loading.into()
                has_data=Signal::derive(move || loaded.get())
                error=Signal::derive(move || load_error.get().map(|e| e.message))
                noun=trs!(ui.domains)
                refresh=Callback::new(move |_| reload.update(|n| *n += 1))
            >
                {message.view()}
                <form id="domain-form" novalidate on:submit=submit>
                    <ValidationSummary form=validation />
                    <FormFields disabled=Signal::derive(move || {
                        busy.get() || !loaded.get() || !can_edit
                    })>
                        <Field label=trs!(ui.name)>
                            <TextInput
                                required=true
                                maxlength=200
                                validation_field="name"
                                value=Signal::derive(move || name.get())
                                on_input=move |e| name.set(event_target_value(&e))
                            />
                        </Field>
                        <p>{tr!(ui.domain_origin_verification_policy)}</p>
                        <Field label=trs!(ui.public_origin)>
                            <TextInput
                                input_type="url"
                                required=true
                                placeholder=trs!(ui.https_training_example_com)
                                readonly=id.is_some()
                                validation_field="origin"
                                value=Signal::derive(move || origin.get())
                                on_input=move |e| origin.set(event_target_value(&e))
                            />
                        </Field>
                        <p>{tr!(ui.domain_origin_help)}</p>
                        <Checkbox
                            label=trs!(ui.enable_this_domain)
                            checked=enabled
                            on_toggle=Callback::new(move |v| enabled.set(v))
                        />
                        <p>
                            {tr!(
                                ui.disabling_stops_new_deliveries_and_public_campaign_handling_for_this_domain_exis
                            )}
                        </p>
                    </FormFields>
                    <FormFooter spread=true>
                        <ActionButton
                            kind="button"
                            style=ButtonStyle::Ghost
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| {
                                loaded.set(false);
                                cancel(
                                    &id
                                        .map(|id| format!("/domains/{id}"))
                                        .unwrap_or_else(|| "/domains".into()),
                                    Default::default(),
                                );
                            }
                        >
                            {tr!(ui.cancel_changes)}
                        </ActionButton>
                        <SaveButton
                            kind="submit"
                            label=Signal::derive(move || {
                                (if id.is_some() {
                                    leptos_i18n::t_string!(i18n, ui.save_changes)
                                } else {
                                    leptos_i18n::t_string!(i18n, ui.create_domain)
                                })
                                    .to_owned()
                            })
                            disabled=Signal::derive(move || {
                                busy.get() || !loaded.get() || !can_edit
                            })
                        />
                    </FormFooter>
                </form>
            </QueryBoundary>
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=load_error
            has_data=Signal::derive(move || loaded.get())
            fetching=loading
            retry=Callback::new(move |()| reload.update(|n| *n += 1))
            back_href="/domains"
            back_label=trs!(ui.back_to_domains)
        >
            {content}
        </ReadBoundary>
    }
}
fn show(api: Api, token: String, id: i64) -> impl IntoView {
    let history_sort = TableSorting::new("domain-history", "finished", SortDirection::Desc);
    let checks_sort = TableSorting::new("domain-checks", "name", SortDirection::Asc);

    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<ManagementQueries>();
    let read = queries.domain.observe(move || id);
    let domain = read.data;
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let mutation = RwSignal::new(false);
    let deletion = ResourceDeletion::new();
    let busy = Signal::derive(move || deletion.busy.get() || mutation.get());
    let token = StoredValue::new(token);
    let message = Message::new(String::new());
    let clock = RwSignal::new(js_sys::Date::now() / 1000.0);
    let can_edit = editable();
    let ticks = StoredValue::new(0u64);
    if let Ok(timer) = set_interval_with_handle(
        move || {
            clock.set(js_sys::Date::now() / 1000.0);
            ticks.update_value(|v| *v += 1);
            let active =
                domain.with_untracked(|d| d.as_ref().is_some_and(|d| d.active_check.is_some()));
            let period = if active { 2 } else { 10 };
            if ticks.get_value().is_multiple_of(period)
                && web_sys::window()
                    .and_then(|w| w.document())
                    .is_some_and(|d| !d.hidden())
                && !read
                    .failure
                    .with_untracked(|e| e.as_ref().is_some_and(|e| e.is_terminal()))
                && !read.fetching.get_untracked()
                && !busy.get_untracked()
            {
                read.refresh.run(());
            }
        },
        std::time::Duration::from_secs(1),
    ) {
        on_cleanup(move || timer.clear());
    }
    let check = move |_| {
        if busy.get_untracked() || !can_edit {
            return;
        }
        if domain.with_untracked(|d| {
            d.as_ref().is_none_or(|d| {
                !d.enabled || d.verification.status == "unsupported" || d.active_check.is_some()
            })
        }) {
            return;
        }
        mutation.set(true);
        message.clear();
        tasks.spawn(async move {
            let result = api
                .resource(
                    "POST",
                    &format!("/api/domains/{id}/verify"),
                    &token.get_value(),
                    None,
                    None,
                )
                .await
                .and_then(|(v, _)| {
                    ManagementQueries::validate_domain(api, v, Some(id)).map_err(|e| e.message)
                });
            match result {
                Ok(value) => {
                    queries.domain_verified(value);
                    #[cfg(feature = "demo")]
                    api.notifications.show(DemoNotice::External);
                }
                Err(error) => message.error(ruphish_ui::localization::verification_error(
                    i18n, &error, true,
                )),
            }
            mutation.set(false);
        });
    };
    let navigate = leptos_router::hooks::use_navigate();
    let deleted = Callback::new(move |_| {
        queries.domain_deleted(id);
        navigate("/domains", Default::default());
    });
    let select_delete = Callback::new(move |()| {
        if busy.get_untracked() || !can_edit {
            return;
        }
        if let Some(d) = domain.get_untracked().filter(|d| d.campaigns == 0) {
            deletion.select(DeleteTarget { id, name:d.name, path:format!("/api/domains/{id}"), revision:Some(format!("\"{}\"",d.revision)),description:leptos_i18n::t_string!(i18n, ui.only_unused_domains_can_be_deleted_this_removes_the_domain_configuration_and_its) });
        }
    });
    let content = view! {
        <ManagementPage>
            <BackLink href="/domains">{tr!(ui.back_to_domains)}</BackLink>
            <PageHeader
                title=Signal::derive(move || {
                    domain
                        .get()
                        .map(|d| d.name)
                        .unwrap_or_else(|| leptos_i18n::t_string!(i18n, ui.domain).into())
                })
                subtitle=Signal::derive(move || {
                    domain
                        .get()
                        .map(|d| d.origin)
                        .unwrap_or_else(|| {
                            leptos_i18n::t_string!(i18n, ui.loading_domain).into()
                        })
                })
            >
                <RefreshButton fetching=read.fetching disabled=busy on_refresh=read.refresh />
                {can_edit
                    .then(|| {
                        view! {
                            <ActionButton
                                style=ButtonStyle::Primary
                                hidden=Signal::derive(move || {
                                    !api.features.get().verification_enabled
                                })
                                disabled=Signal::derive(move || {
                                    busy.get()
                                        || domain
                                            .with(|d| {
                                                d.as_ref()
                                                    .is_none_or(|d| {
                                                        !d.enabled || d.verification.status == "unsupported"
                                                            || d.active_check.is_some()
                                                    })
                                            })
                                })
                                on_click=check
                            >
                                {move || {
                                    if mutation.get() {
                                        leptos_i18n::t_string!(i18n, ui.requesting_verification)
                                    } else {
                                        leptos_i18n::t_string!(i18n, ui.verify_domain)
                                    }
                                }}
                            </ActionButton>
                            <ResourceActions
                                edit_href=format!("/domains/{id}/edit")
                                can_edit=can_edit
                                disabled=Signal::derive(move || {
                                    busy.get() || domain.with(Option::is_none)
                                })
                                delete_disabled=Signal::derive(move || {
                                    domain.with(|d| d.as_ref().is_none_or(|d| d.campaigns > 0))
                                })
                                delete_reason=leptos_i18n::t_string!(
                                    i18n, ui.domains_used_by_campaigns_cannot_be_deleted
                                )
                                on_delete=select_delete
                            />
                        }
                    })}
            </PageHeader>
            {message.view()}
            <QueryBoundary
                fetching=read.fetching
                has_data=Signal::derive(move || domain.with(Option::is_some))
                error=read.error
                noun=trs!(ui.domain)
                refresh=read.refresh
            >
                {move || {
                    domain
                        .get()
                        .filter(|d| d.verification.required)
                        .and_then(|d| d.active_check)
                        .map(|run| {
                            let running = run.state == "running";
                            let started = run.started_at.unwrap_or(run.scheduled_at);
                            view! {
                                <section
                                    class="domain-verification-progress"
                                    aria-label=tr!(ui.active_verification)
                                    aria-busy=running.to_string()
                                >
                                    <div class="domain-progress-title">
                                        {running
                                            .then(|| {
                                                view! {
                                                    <span class="domain-spinner" aria-hidden="true"></span>
                                                }
                                            })}
                                        <strong role="status">
                                            {move || {
                                                if running {
                                                    leptos_i18n::t_string!(i18n, ui.verifying_domain)
                                                } else {
                                                    leptos_i18n::t_string!(i18n, ui.verification_queued)
                                                }
                                            }}
                                        </strong>
                                    </div>
                                    {if running {
                                        view! {
                                            <p>
                                                {move || {
                                                    leptos_i18n::t_string!(
                                                        i18n, messages.running_for_arg0_seconds_results_appear_below_when_the_check_finishes, arg0 = ((clock.get() as i64-started).max(0)).to_string()
                                                    )
                                                }}
                                            </p>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <p>
                                                {move || {
                                                    if run.scheduled_at > clock.get() as i64 {
                                                        leptos_i18n::t_string!(
                                                            i18n, messages.scheduled_to_start_no_earlier_than_arg0, arg0 = (instant(run.scheduled_at)).to_string()
                                                        )
                                                    } else {
                                                        leptos_i18n::t_string!(
                                                            i18n, ui.waiting_for_the_verification_worker
                                                        )
                                                            .into()
                                                    }
                                                }}
                                            </p>
                                        }
                                            .into_any()
                                    }}
                                    <p>
                                        {tr!(
                                            ui.this_check_continues_if_you_leave_or_refresh_the_page
                                        )}
                                    </p>
                                </section>
                            }
                        })
                }}
                {move || {
                    domain
                        .get()
                        .map(|d| {
                            if !d.verification.required {
                                return view! {
                                    <Card class="resource-read-panel domain-context">
                                        <h2>{tr!(ui.domain_status)}</h2>
                                        <p>{tr!(ui.verification_not_required)}</p>
                                        <p>
                                            {if d.enabled {
                                                leptos_i18n::t_string!(i18n,ui.available_for_campaigns)
                                            } else {
                                                leptos_i18n::t_string!(
                                                    i18n,ui.disabled_unavailable_for_campaigns
                                                )
                                            }}
                                        </p>
                                    </Card>
                                }
                                    .into_any();
                            }
                            if d.verification.status == "unsupported" {
                                return view! {
                                    <Card class="resource-read-panel domain-context">
                                        <h2>{tr!(ui.domain_status)}</h2>
                                        <p>{tr!(ui.domain_origin_verification_policy)}</p>
                                    </Card>
                                }
                                    .into_any();
                            }
                            let has_result = d.checked_at > 0;
                            let domain_state = state(&d);
                            let domain_tone = status_tone(&d.verification.status);
                            let checking = d.active_check.is_some();
                            let control_result = check_result(&d, "control", d.control);
                            let check_rows = [
                                (
                                    "DNS",
                                    "DNS",
                                    d.dns,
                                    leptos_i18n::t_string!(
                                        i18n, ui.resolve_to_permitted_destinations
                                    ),
                                ),
                                (
                                    "routing",
                                    leptos_i18n::t_string!(i18n, ui.instance_routing),
                                    d.routing,
                                    leptos_i18n::t_string!(i18n, ui.reach_this_ruphish_instance),
                                ),
                                (
                                    "HTTPS",
                                    "HTTPS",
                                    d.tls,
                                    leptos_i18n::t_string!(
                                        i18n, ui.present_a_trusted_certificate_for_this_hostname
                                    ),
                                ),
                            ]
                                .map(|(kind, name, passed, help)| (
                                    name,
                                    check_result(&d, kind, passed),
                                    help,
                                ));
                            view! {
                                <Card
                                    class="resource-read-panel domain-context"
                                    label=trs!(ui.domain_status)
                                >
                                    <h2>{tr!(ui.domain_status)}</h2>
                                    <p>
                                        {move || {
                                            if !d.enabled {
                                                leptos_i18n::t_string!(
                                                    i18n, ui.disabled_unavailable_for_campaigns
                                                )
                                            } else if d.eligible {
                                                leptos_i18n::t_string!(i18n, ui.available_for_campaigns)
                                            } else {
                                                leptos_i18n::t_string!(
                                                    i18n, ui.unavailable_for_campaigns_until_verification_passes
                                                )
                                            }
                                        }}
                                    </p>
                                    <dl class="resource-facts">
                                        <div>
                                            <dt>{tr!(ui.status)}</dt>
                                            <dd>
                                                <StatusBadge tone=domain_tone>{domain_state}</StatusBadge>
                                            </dd>
                                        </div>
                                        <div>
                                            <dt>{tr!(ui.enabled)}</dt>
                                            <dd>
                                                {move || {
                                                    if d.enabled {
                                                        leptos_i18n::t_string!(i18n, ui.yes)
                                                    } else {
                                                        leptos_i18n::t_string!(i18n, ui.no)
                                                    }
                                                }}
                                            </dd>
                                        </div>
                                        <div>
                                            <dt>{tr!(ui.last_completed_check)}</dt>
                                            <dd class="data-value">{instant(d.checked_at)}</dd>
                                        </div>
                                        <div>
                                            <dt>{tr!(ui.verified_until)}</dt>
                                            <dd>
                                                {move || {
                                                    if let Some(expiry) = d.verification.expires_at {
                                                        instant(expiry)
                                                    } else if d.verification.status == "verified" {
                                                        leptos_i18n::t_string!(i18n,ui.verification_no_expiry)
                                                            .into()
                                                    } else {
                                                        leptos_i18n::t_string!(i18n, ui.not_verified).into()
                                                    }
                                                }}
                                            </dd>
                                        </div>
                                        <div>
                                            <dt>{tr!(ui.next_automatic_check)}</dt>
                                            <dd>
                                                {move || {
                                                    if !d.enabled {
                                                        leptos_i18n::t_string!(i18n, ui.disabled).into()
                                                    } else if checking {
                                                        leptos_i18n::t_string!(
                                                            i18n, ui.after_the_active_check_finishes
                                                        )
                                                            .into()
                                                    } else {
                                                        instant(d.next_check_at)
                                                    }
                                                }}
                                            </dd>
                                        </div>
                                    </dl>
                                </Card>
                                <section class="resource-section">
                                    <h2>{tr!(ui.verification_requirements)}</h2>
                                    <p>
                                        {leptos_i18n::t!(
                                            i18n, messages.used_by_arg0_campaigns_verification_checks_dns_control_permitted_destinations_in, arg0 = (d.campaigns).to_string()
                                        )}
                                    </p>
                                    {checking
                                        .then(|| {
                                            view! {
                                                <p>
                                                    {tr!(
                                                        ui.the_table_shows_the_previous_completed_result_while_verification_is_in_progress
                                                    )}
                                                </p>
                                            }
                                        })}
                                    <DataTable
                                        label=trs!(ui.domain_verification)
                                        class="domain-verification"
                                        sorting=checks_sort
                                        columns=vec!["name", "result", "help", ""]
                                        headers=vec![
                                            trs!(ui.check),
                                            trs!(ui.latest_result),
                                            trs!(ui.requirement),
                                            trs!(ui.dns_record),
                                        ]
                                    >
                                        {move || {
                                            let mut rows = vec![
                                                (
                                                    leptos_i18n::t_string!(i18n, ui.domain_control),
                                                    control_result,
                                                    leptos_i18n::t_string!(
                                                        i18n, ui.publish_this_txt_record_at_your_dns_provider_and_keep_it_for_periodic_checks
                                                    ),
                                                    true,
                                                ),
                                            ];
                                            rows.extend(
                                                check_rows
                                                    .iter()
                                                    .map(|(name, result, help)| (*name, *result, *help, false)),
                                            );
                                            checks_sort
                                                .rows(
                                                    rows,
                                                    |row, key| match key {
                                                        "result" => row.1.into(),
                                                        "help" => row.2.into(),
                                                        _ => row.0.into(),
                                                    },
                                                    |row| row.0.into(),
                                                )
                                                .into_iter()
                                                .map(|(name, result, help, control)| {
                                                    view! {
                                                        <tr>
                                                            <th scope="row">{name}</th>
                                                            <td>{result}</td>
                                                            <td>{help}</td>
                                                            <td>
                                                                {if control {
                                                                    view! {
                                                                        <div class="domain-copy-actions">
                                                                            <CopyButton
                                                                                label=trs!(ui.record_name)
                                                                                value=d.txt_name.clone()
                                                                                message=message
                                                                            />
                                                                            <CopyButton
                                                                                label=trs!(ui.txt_value)
                                                                                value=d.txt_value.clone()
                                                                                message=message
                                                                            />
                                                                        </div>
                                                                    }
                                                                        .into_any()
                                                                } else {
                                                                    view! { <span>"\u{2014}"</span> }.into_any()
                                                                }}
                                                            </td>
                                                        </tr>
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </DataTable>
                                    <p role="status">
                                        {move || {
                                            if has_result {
                                                check_explanation(&d.code)
                                            } else {
                                                leptos_i18n::t_string!(
                                                    i18n, ui.publish_the_dns_proof_configure_routing_and_https_then_verify_the_domain
                                                )
                                            }
                                        }}
                                    </p>
                                </section>
                                <section class="resource-section domain-history">
                                    <h2>{tr!(ui.verification_history)}</h2>
                                    <p>
                                        {tr!(
                                            ui.latest_ten_completed_checks_a_failed_or_timed_out_check_immediately_makes_the_do
                                        )}
                                    </p>
                                    <DataTable
                                        sorting=history_sort
                                        columns=vec!["finished", "state", "duration", "code"]
                                        label=trs!(ui.verification_history)
                                        headers=vec![
                                            trs!(ui.finished),
                                            trs!(ui.outcome),
                                            trs!(ui.duration),
                                            trs!(ui.details),
                                        ]
                                    >
                                        {move || {
                                            history_sort
                                                .rows(
                                                    d.history.clone(),
                                                    |row, key| match key {
                                                        "state" => outcome(&row.state).into(),
                                                        "duration" => {
                                                            row.started_at
                                                                .zip(row.finished_at)
                                                                .map_or(SortValue::Missing, |(a, b)| (b - a).max(0).into())
                                                        }
                                                        "code" => check_explanation(&row.code).into(),
                                                        _ => row.finished_at.map_or(SortValue::Missing, Into::into),
                                                    },
                                                    |row| row.id.into(),
                                                )
                                                .into_iter()
                                                .map(|r| {
                                                    view! {
                                                        <tr>
                                                            <td class="data-value">
                                                                {r
                                                                    .finished_at
                                                                    .map(instant)
                                                                    .unwrap_or_else(|| {
                                                                        leptos_i18n::t_string!(i18n, ui.not_recorded).into()
                                                                    })}
                                                            </td>
                                                            <td>
                                                                <StatusBadge tone=status_tone(
                                                                    &r.state,
                                                                )>{outcome(&r.state)}</StatusBadge>
                                                            </td>
                                                            <td>
                                                                {match (r.started_at, r.finished_at) {
                                                                    (Some(start), Some(end)) => {
                                                                        leptos_i18n::t_string!(
                                                                            i18n, messages.count_seconds, count = (end-start).max(0)
                                                                        )
                                                                    }
                                                                    _ => leptos_i18n::t_string!(i18n, ui.not_recorded).into(),
                                                                }}
                                                            </td>
                                                            <td>{check_explanation(&r.code)}</td>
                                                        </tr>
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </DataTable>
                                    {(!has_result)
                                        .then(|| {
                                            view! { <p>{tr!(ui.no_completed_checks_yet)}</p> }
                                        })}
                                </section>
                            }
                                .into_any()
                        })
                }}
                <ResourceDeleteDialog
                    deletion=deletion
                    api=api
                    token=token.get_value()
                    on_deleted=deleted
                />
            </QueryBoundary>
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=read.failure
            has_data=Signal::derive(move || read.data.with(Option::is_some))
            fetching=read.fetching
            retry=read.refresh
            back_href="/domains"
            back_label=trs!(ui.back_to_domains)
        >
            {content}
        </ReadBoundary>
    }
}

fn check_explanation(code: &str) -> &'static str {
    let i18n = ruphish_ui::localization::use_i18n();

    match code {
        "verified" => leptos_i18n::t_string!(i18n, ui.control_dns_routing_and_https_passed),
        "domain_txt_unavailable" => {
            leptos_i18n::t_string!(
                i18n,
                ui.the_dns_txt_record_could_not_be_retrieved_publish_the_record_from_the_domain_con
            )
        }
        "domain_txt_mismatch" => {
            leptos_i18n::t_string!(
                i18n,
                ui.the_dns_txt_record_was_found_but_its_value_does_not_match_this_domain_s_verifica
            )
        }
        "domain_challenge_expired" => {
            leptos_i18n::t_string!(
                i18n,
                ui.the_previous_proof_expired_request_a_new_check_and_publish_the_current_txt_value
            )
        }
        "domain_destination_denied" => {
            leptos_i18n::t_string!(
                i18n,
                ui.dns_resolves_to_a_restricted_network_ask_your_installation_administrator_to_revi
            )
        }
        "domain_https_unavailable" => {
            leptos_i18n::t_string!(
                i18n,
                ui.https_could_not_be_established_check_the_listener_firewall_certificate_hostname
            )
        }
        "domain_routing_mismatch" | "domain_routing_unavailable" => {
            leptos_i18n::t_string!(
                i18n,
                ui.https_did_not_return_this_instance_s_proof_check_dns_and_reverse_proxy_routing_r
            )
        }
        "domain_dns_unavailable" | "domain_dns_address_limit" => {
            leptos_i18n::t_string!(
                i18n,
                ui.check_the_hostname_s_a_and_aaaa_records_every_address_must_be_reachable_and_perm
            )
        }
        "domain_claim_conflict" => {
            leptos_i18n::t_string!(
                i18n,
                ui.this_hostname_is_assigned_to_another_account_use_that_account_s_domain_or_ask_it
            )
        }
        "domain_worker_timeout" => {
            leptos_i18n::t_string!(
                i18n,
                ui.the_worker_stopped_responding_before_completion_verification_was_restarted
            )
        }
        "domain_check_timeout" => {
            leptos_i18n::t_string!(i18n, ui.verification_timed_out_checks_retry_automatically)
        }
        _ => leptos_i18n::t_string!(
            i18n,
            ui.verification_is_pending_publish_the_dns_proof_and_configure_https_routing
        ),
    }
}

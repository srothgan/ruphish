//! Domain management uses the shared resource shell, forms and confirmation dialog.
use crate::{account::Api, feedback::Message, resources::ResourceAccess, shared::components::*};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_contracts::domains::{Domain, Edit};
use serde_json::json;

fn editable() -> bool {
    use_context::<ResourceAccess>().is_some_and(|v| v.0)
}
fn state(d: &Domain) -> &'static str {
    match d.status.as_str() {
        "verified" => "Verified",
        "expired" => "Expired",
        "failed" => "Failed",
        _ => "Not verified",
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
    if seconds == 0 {
        "Not recorded".into()
    } else {
        crate::campaigns::instant(&json!(seconds))
    }
}
fn outcome(value: &str) -> &'static str {
    match value {
        "passed" => "Passed",
        "failed" => "Failed",
        "timed_out" => "Timed out",
        _ => "Not recorded",
    }
}
fn check_result(d: &Domain, kind: &str, passed: bool) -> &'static str {
    if d.checked_at == 0 {
        return "Not checked";
    }
    if passed {
        return "Passed";
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
        "control" if control_failed => "Failed",
        _ if control_failed => "Not checked",
        "DNS" if dns_failed => "Failed",
        "HTTPS" | "Instance routing" if dns_failed => "Not checked",
        "HTTPS" if code == "domain_https_unavailable" => "Failed",
        "Instance routing" if code == "domain_https_unavailable" => "Not checked",
        "Instance routing"
            if matches!(
                code,
                "domain_routing_mismatch" | "domain_routing_unavailable"
            ) =>
        {
            "Failed"
        }
        _ => "Not confirmed",
    }
}
fn decode(value: serde_json::Value) -> Result<Domain, String> {
    serde_json::from_value(value).map_err(|_| "Invalid domain response".into())
}
pub(super) fn list(api: Api) -> impl IntoView {
    let rows = RwSignal::new(Vec::<Domain>::new());
    let failed = RwSignal::new(false);
    let loading = RwSignal::new(false);
    let search = RwSignal::new(String::new());
    let refresh = RwSignal::new(0u64);
    let message = Message::new(String::new());
    let can_edit = editable();
    Effect::new(move |_| {
        refresh.get();
        loading.set(true);
        failed.set(false);
        spawn_local(async move {
            match api
                .get("/api/domains/")
                .await
                .and_then(|v| serde_json::from_value(v).map_err(|_| "Invalid domain list".into()))
            {
                Ok(v) => {
                    rows.set(v);
                    message.clear();
                }
                Err(e) => {
                    failed.set(true);
                    message.error(e);
                }
            }
            loading.set(false);
        });
    });
    let visible = Signal::derive(move || {
        let query = search.get().to_lowercase();
        rows.get()
            .into_iter()
            .filter(|d| {
                format!("{} {}", d.name, d.hostname)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect::<Vec<_>>()
    });
    view! {<ManagementPage><PageHeader title="Domains" subtitle="Verified destinations for campaign links and landing pages.">
        <ActionButton disabled=Signal::derive(move||loading.get()) on_click=move |_|refresh.update(|v|*v+=1)>"Refresh"</ActionButton>
        {can_edit.then(||view!{<ActionLink style=ButtonStyle::Primary href="/domains/new">"Create domain"</ActionLink>})}
    </PageHeader>{message.view()}<ListToolbar search=search count=Signal::derive(move || visible.with(Vec::len)) label="Search by name or hostname" noun="domains"/>
    <Show when=move||loading.get()><p role="status">"Loading domains…"</p></Show>
    <DataTable label="Domains" empty=Signal::derive(move || visible.with(Vec::is_empty)) headers=vec!["Name","Public origin","Status","Enabled","Campaigns","Actions"]>
        <For each=move||visible.get() key=|d| (d.id,d.revision,d.checked_at,d.eligible,d.checking,d.status.clone(),d.code.clone(),d.campaigns) children=move|d|{let id=d.id;let label=state(&d);view!{<tr><td><a href=format!("/domains/{id}")>{d.name.clone()}</a></td><td>{d.origin.clone()}</td><td><StatusBadge tone=status_tone(&d.status)>{label}</StatusBadge></td><td>{if d.enabled{"Yes"}else{"No"}}</td><td>{d.campaigns}</td><td><a href=format!("/domains/{id}")>"View"</a>{can_edit.then(||view!{<a href=format!("/domains/{id}/edit")>"Edit"</a>})}</td></tr>}}/>
    </DataTable><ListEmptyState ready=Signal::derive(move || !loading.get() && !failed.get()) count=Signal::derive(move || visible.with(Vec::len)) search=search title="No domains found" guidance="Add a hostname, prove DNS control and check routing and HTTPS before using it in a campaign."/></ManagementPage>}
}
pub(super) fn detail(api: Api, token: String, editing: bool) -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    view! {<For each=move||vec![params.with(|p|p.get("id")).unwrap_or_default()] key=|s|s.clone() children=move|s|{
        if s=="new"{form(api,token.clone(),None).into_any()}
        else if let Ok(id)=s.parse::<i64>() {if editing{form(api,token.clone(),Some(id)).into_any()}else{show(api,token.clone(),id).into_any()}}
        else{view!{<ManagementPage><p role="alert">"Invalid domain address"</p><a href="/domains">"Back to domains"</a></ManagementPage>}.into_any()}
    }/>}
}
fn form(api: Api, token: String, id: Option<i64>) -> impl IntoView {
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
    let loading = RwSignal::new(id.is_some());
    if let Some(id) = id {
        Effect::new(move |_| {
            reload.get();
            loading.set(true);
            message.clear();
            spawn_local(async move {
                match api
                    .get(&format!("/api/domains/{id}"))
                    .await
                    .and_then(decode)
                {
                    Ok(d) => {
                        baseline.set((d.name.clone(), d.origin.clone(), d.enabled));
                        name.set(d.name);
                        origin.set(d.origin);
                        enabled.set(d.enabled);
                        revision.set(Some(d.revision));
                        loaded.set(true)
                    }
                    Err(e) => message.error(e),
                }
                loading.set(false);
            });
        });
    }
    let navigate = leptos_router::hooks::use_navigate();
    let cancel = navigate.clone();
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() || !loaded.get_untracked() || !can_edit {
            return;
        }
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
                .resource(
                    if id.is_some() { "PUT" } else { "POST" },
                    &path,
                    &token.get_value(),
                    Some(&json!(edit)),
                    None,
                )
                .await
                .and_then(|(v, _)| decode(v))
            {
                Ok(d) => {
                    loaded.set(false);
                    busy.set(false);
                    navigate(&format!("/domains/{}", d.id), Default::default());
                }
                Err(e) => {
                    busy.set(false);
                    message.error(e)
                }
            }
        });
    };
    view! {<ManagementPage><BackLink href="/domains">"Back to domains"</BackLink><PageHeader title=if id.is_some(){"Edit domain"}else{"Create domain"} subtitle=Signal::derive(move||if dirty.get(){"Unsaved changes"}else{"Verify domain control, routing and HTTPS before campaign use."}.to_owned())>
    </PageHeader>{message.view()}<Show when=move||!loaded.get()&&!loading.get()><ActionButton on_click=move |_|reload.update(|n|*n+=1)>"Retry loading domain"</ActionButton></Show><form id="domain-form" on:submit=submit><FormFields disabled=Signal::derive(move||busy.get()||!loaded.get()||!can_edit)>
        <Field label="Name"><TextInput required=true maxlength=200 value=Signal::derive(move||name.get()) on_input=move|e|name.set(event_target_value(&e))/></Field>
        <Field label="Public HTTPS origin"><TextInput input_type="url" required=true placeholder="https://training.example.com" readonly=id.is_some() value=Signal::derive(move||origin.get()) on_input=move|e|origin.set(event_target_value(&e))/></Field>
        <p>"Use the hostname participants will visit. Paths belong to the campaign. A saved hostname cannot be changed."</p>
        <Checkbox label="Enable this domain" checked=enabled on_toggle=Callback::new(move|v|enabled.set(v))/>
        <p>"Disabling stops new deliveries and public campaign handling for this domain. Existing campaign history is retained."</p>
    </FormFields><FormFooter spread=true><ActionButton kind="button" style=ButtonStyle::Ghost disabled=Signal::derive(move||busy.get()) on_click=move |_|{loaded.set(false);cancel(&id.map(|id|format!("/domains/{id}")).unwrap_or_else(||"/domains".into()),Default::default());}>"Cancel changes"</ActionButton><SaveButton kind="submit" label=if id.is_some(){"Save changes"}else{"Create domain"} disabled=Signal::derive(move||busy.get()||!loaded.get()||!can_edit)/></FormFooter></form></ManagementPage>}
}
fn show(api: Api, token: String, id: i64) -> impl IntoView {
    use ruphish_ui::domain_state::Details;
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let detail = RwSignal::new(Details::default());
    let domain = Memo::new(move |_| detail.with(|s| s.domain.clone()));
    let busy = Signal::derive(move || detail.with(|s| s.mutation.is_some()));
    let deleting = RwSignal::new(false);
    let token = StoredValue::new(token);
    let message = Message::new(String::new());
    let refresh = RwSignal::new(0u64);
    let clock = RwSignal::new(js_sys::Date::now() / 1000.0);
    let can_edit = editable();
    Effect::new(move |_| {
        refresh.get();
        let Some(ticket) = detail.try_update(Details::refresh).flatten() else {
            return;
        };
        tasks.spawn(async move {
            let result = api
                .get(&format!("/api/domains/{id}"))
                .await
                .and_then(decode);
            detail.update(|s| s.received(ticket, result, false));
        });
    });
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
            {
                refresh.update(|v| *v += 1);
            }
        },
        std::time::Duration::from_secs(1),
    ) {
        on_cleanup(move || timer.clear());
    }
    let focus = window_event_listener(leptos::ev::focus, move |_| refresh.update(|v| *v += 1));
    on_cleanup(move || focus.remove());
    let check = move |_| {
        let Some(ticket) = detail.try_update(Details::verify).flatten() else {
            return;
        };
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
                .and_then(|(v, _)| decode(v));
            detail.update(|s| s.received(ticket, result, true));
            refresh.update(|v| *v += 1);
        });
    };
    let navigate = leptos_router::hooks::use_navigate();
    let remove = Callback::new(move |()| {
        let Some(d) = domain.get_untracked() else {
            return;
        };
        if detail
            .try_update(Details::begin_mutation)
            .flatten()
            .is_none()
        {
            return;
        }
        let navigate = navigate.clone();
        tasks.spawn(async move {
            match api
                .resource(
                    "DELETE",
                    &format!("/api/domains/{id}"),
                    &token.get_value(),
                    None,
                    Some(&format!("\"{}\"", d.revision)),
                )
                .await
            {
                Ok(_) => {
                    deleting.set(false);
                    navigate("/domains", Default::default());
                }
                Err(e) => {
                    message.error(e);
                    detail.update(|s| s.mutation = None);
                }
            }
        });
    });
    view! {<ManagementPage><BackLink href="/domains">"Back to domains"</BackLink>
        <PageHeader title=Signal::derive(move||domain.get().map(|d|d.name).unwrap_or_else(||"Domain".into())) subtitle=Signal::derive(move||domain.get().map(|d|d.origin).unwrap_or_else(||"Loading domain".into()))>
            <ActionButton disabled=Signal::derive(move||busy.get()) busy=Signal::derive(move||detail.with(|s|s.reading.is_some()).to_string()) on_click=move |_|refresh.update(|v|*v+=1)>{move||if detail.with(|s|s.reading.is_some()){"Refreshing…"}else{"Refresh status"}}</ActionButton>
            {can_edit.then(||view!{<ActionButton style=ButtonStyle::Primary disabled=Signal::derive(move||busy.get()||domain.with(|d|d.as_ref().is_none_or(|d|!d.enabled||d.active_check.is_some()))) on_click=check>{move||if busy.get()&&!deleting.get(){"Requesting verification"}else{"Verify domain"}}</ActionButton><ActionLink href=format!("/domains/{id}/edit")>"Edit"</ActionLink><ActionButton style=ButtonStyle::Danger disabled=Signal::derive(move||busy.get()||domain.with(|d|d.as_ref().is_none_or(|d|d.campaigns>0))) on_click=move |_|deleting.set(true)>"Delete"</ActionButton>})}
        </PageHeader>{message.view()}
        {move||detail.with(|s|s.error.clone()).map(|error|view!{<p class="form-message error" role="alert">{error}</p>})}
        {move||domain.get().and_then(|d|d.active_check).map(|run|{
            let running=run.state=="running";
            let started=run.started_at.unwrap_or(run.scheduled_at);
            view!{<section class="domain-verification-progress" aria-label="Active verification" aria-busy=running.to_string()>
                <div class="domain-progress-title">{running.then(||view!{<span class="domain-spinner" aria-hidden="true"></span>})}<strong role="status">{if running{"Verifying domain"}else{"Verification queued"}}</strong></div>
                {if running {view!{<p>{move||format!("Running for {} seconds. Results appear below when the check finishes.",(clock.get() as i64-started).max(0))}</p>}.into_any()}else{view!{<p>{move||if run.scheduled_at>clock.get() as i64 {format!("Scheduled to start no earlier than {}.",instant(run.scheduled_at))}else{"Waiting for the verification worker.".into()}}</p>}.into_any()}}
                <p>"This check continues if you leave or refresh the page."</p>
            </section>}
        })}
        {move||domain.get().map(|d|{
            let has_result=d.checked_at>0;
            let domain_state=state(&d);
            let domain_tone=status_tone(&d.status);
            let checking=d.active_check.is_some();
            let control_result=check_result(&d,"control",d.control);
            let check_rows=[("DNS",d.dns,"Resolve to permitted destinations."),("Instance routing",d.routing,"Reach this Ruphish instance."),("HTTPS",d.tls,"Present a trusted certificate for this hostname.")].map(|(name,passed,help)|(name,check_result(&d,name,passed),help));
            view!{<Card class="resource-read-panel domain-context" label="Domain status"><h2>"Domain status"</h2>
                <p>{if !d.enabled {"Disabled: unavailable for campaigns."}else if d.eligible{"Available for campaigns."}else{"Unavailable for campaigns until verification passes."}}</p>
                <dl class="resource-facts"><div><dt>"Status"</dt><dd><StatusBadge tone=domain_tone>{domain_state}</StatusBadge></dd></div><div><dt>"Enabled"</dt><dd>{if d.enabled{"Yes"}else{"No"}}</dd></div><div><dt>"Last completed check"</dt><dd class="data-value">{instant(d.checked_at)}</dd></div><div><dt>"Verified until"</dt><dd>{if d.verified_until>0{instant(d.verified_until)}else{"Not verified".into()}}</dd></div><div><dt>"Next automatic check"</dt><dd>{if !d.enabled{"Disabled".into()}else if checking{"After the active check finishes".into()}else{instant(d.next_check_at)}}</dd></div></dl>
            </Card><section class="resource-section"><h2>"Verification requirements"</h2>
                <p>{format!("Used by {} campaigns. Verification checks DNS control, permitted destinations, instance routing and HTTPS.",d.campaigns)}</p>
                {checking.then(||view!{<p>"The table shows the previous completed result while verification is in progress."</p>})}
                <DataTable label="Domain verification" class="domain-verification" headers=vec!["Check","Latest result","Requirement","DNS record"]>
                    <tr><th scope="row">"Domain control"</th><td>{control_result}</td><td>"Publish this TXT record at your DNS provider and keep it for periodic checks."</td><td><div class="domain-copy-actions"><CopyButton label="record name" value=d.txt_name.clone() message=message/><CopyButton label="TXT value" value=d.txt_value.clone() message=message/></div></td></tr>
                    {check_rows.into_iter().map(|(name,result,help)|view!{<tr><th scope="row">{name}</th><td>{result}</td><td>{help}</td><td></td></tr>}).collect_view()}
                </DataTable>
                <p role="status">{if has_result{check_explanation(&d.code)}else{"Publish the DNS proof, configure routing and HTTPS, then verify the domain."}}</p>
            </section>
            <section class="resource-section domain-history"><h2>"Verification history"</h2><p>"Latest ten completed checks. A failed or timed-out check immediately makes the domain unavailable for campaigns."</p>
                <DataTable label="Verification history" headers=vec!["Finished","Outcome","Duration","Details"]>
                    {d.history.into_iter().map(|r|view!{<tr><td class="data-value">{r.finished_at.map(instant).unwrap_or_else(||"Not recorded".into())}</td><td><StatusBadge tone=status_tone(&r.state)>{outcome(&r.state)}</StatusBadge></td><td>{match(r.started_at,r.finished_at){(Some(start),Some(end))=>format!("{} seconds",(end-start).max(0)),_=>"Not recorded".into()}}</td><td>{check_explanation(&r.code)}</td></tr>}).collect_view()}
                </DataTable>
                {(!has_result).then(||view!{<p>"No completed checks yet."</p>})}
            </section>}
        })}
        {move||deleting.get().then(||view!{<DeleteConfirmation title="Delete domain?".to_owned() description="Only unused domains can be deleted. This removes the domain configuration and its verification history." confirm_label="Delete domain" busy=busy on_confirm=remove on_cancel=Callback::new(move|()|deleting.set(false)) message=message/>})}
    </ManagementPage>}
}

fn check_explanation(code: &str) -> &'static str {
    match code {
        "verified" => "Control, DNS, routing and HTTPS passed.",
        "domain_txt_unavailable" => {
            "The DNS TXT record could not be retrieved. Publish the record from the Domain control row and check that this Ruphish server's configured DNS resolver can resolve it."
        }
        "domain_txt_mismatch" => {
            "The DNS TXT record was found, but its value does not match this domain's verification proof. Publish the TXT value from the Domain control row, then allow DNS caches to refresh."
        }
        "domain_challenge_expired" => {
            "The previous proof expired. Request a new check and publish the current TXT value."
        }
        "domain_destination_denied" => {
            "DNS resolves to a restricted network. Ask your installation administrator to review the destination policy."
        }
        "domain_https_unavailable" => {
            "HTTPS could not be established. Check the listener, firewall, certificate hostname, chain and expiry."
        }
        "domain_routing_mismatch" | "domain_routing_unavailable" => {
            "HTTPS did not return this instance's proof. Check DNS and reverse-proxy routing; redirects are not followed."
        }
        "domain_dns_unavailable" | "domain_dns_address_limit" => {
            "Check the hostname's A and AAAA records. Every address must be reachable and permitted."
        }
        "domain_claim_conflict" => {
            "This hostname is assigned to another account. Use that account's domain or ask its owner to release an unused domain."
        }
        "domain_worker_timeout" => {
            "The worker stopped responding before completion. Verification was restarted."
        }
        "domain_check_timeout" => "Verification timed out. Checks retry automatically.",
        _ => "Verification is pending. Publish the DNS proof and configure HTTPS routing.",
    }
}

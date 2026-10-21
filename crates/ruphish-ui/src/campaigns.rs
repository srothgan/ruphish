//! Campaign preparation and operational delivery views within the shared shell.
use crate::{account::Api, feedback::Message, resources::ResourceAccess, shared::components::*};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use ruphish_ui::campaign_state::Draft;
use ruphish_ui::pagination::CursorPages;
use ruphish_ui::{tr, trs};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::time_display::{instant, zone_name as local_zone};
#[wasm_bindgen(inline_js = "export function campaignKey(){return crypto.randomUUID()}")]
extern "C" {
    #[wasm_bindgen(catch,js_name=campaignKey)]
    pub(super) fn random_key() -> Result<String, JsValue>;
}
pub(super) fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
pub(super) fn array(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
pub(super) fn editable() -> bool {
    use_context::<ResourceAccess>().is_some_and(|v| v.0)
}

pub(super) fn list(api: Api, token: String) -> impl IntoView {
    list_mode(api, false, token)
}
pub(super) fn analysis_directory(api: Api) -> impl IntoView {
    list_mode(api, true, String::new())
}
fn list_mode(api: Api, analysis: bool, token: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let token = StoredValue::new(token);
    let filter = RwSignal::new(String::from(if analysis { "executions" } else { "all" }));
    let rows = RwSignal::new(Vec::<Value>::new());
    let cursors = RwSignal::new(vec![String::new()]);
    let after = Memo::new(move |_| cursors.with(|v| v.last().cloned().unwrap_or_default()));
    let next = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    let message = Message::new(String::new());
    let delete_message = Message::new(String::new());
    let can_edit = editable() && !analysis;
    let refresh = RwSignal::new(0u64);
    let deleting = RwSignal::new(None::<Value>);
    let mutating = RwSignal::new(false);
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    Effect::new(move |_| {
        let generation = refresh.get();
        let selected = filter.get();
        let cursor = after.get();
        busy.set(true);
        loaded.set(false);
        next.set(None);
        tasks.spawn(async move {
            let result = api
                .get(&format!(
                    "/api/campaigns/directory?after={cursor}&filter={selected}"
                ))
                .await;
            if cursor != after.get_untracked()
                || generation != refresh.get_untracked()
                || selected != filter.get_untracked()
            {
                return;
            }
            match result {
                Ok(value) => {
                    rows.set(array(&value, "items"));
                    next.set(value["next_after"].as_str().map(str::to_owned));
                    loaded.set(true);
                    message.clear();
                }
                Err(e) => {
                    rows.set(Vec::new());
                    message.error(e);
                }
            }
            busy.set(false);
        });
    });
    let confirm = Callback::new(move |()| {
        let Some(target) = deleting.get_untracked() else {
            return;
        };
        if mutating.get_untracked() {
            return;
        }
        mutating.set(true);
        delete_message.clear();
        tasks.spawn(async move {
            let id = text(&target, "id");
            let path = if target["kind"] == "draft" {
                format!("/api/campaign-drafts/{id}?revision={}", target["revision"])
            } else {
                format!("/api/campaigns/{id}")
            };
            match api
                .resource("DELETE", &path, &token.get_value(), None, None)
                .await
            {
                Ok(_) => {
                    rows.update(|v| {
                        v.retain(|r| r["kind"] != target["kind"] || r["id"] != target["id"])
                    });
                    deleting.set(None);
                    // Deleting the last row on a later page returns to the previous page.
                    batch(move || {
                        if rows.with_untracked(Vec::is_empty)
                            && cursors.with_untracked(|c| c.len() > 1)
                        {
                            cursors.update(|c| {
                                c.pop();
                            });
                        }
                        refresh.update(|v| *v += 1);
                    });
                }
                Err(e) => delete_message.error(e),
            }
            mutating.set(false);
        });
    });
    view! {<ManagementPage><PageHeader title=Signal::derive(move || (if analysis { leptos_i18n::t_string!(i18n, ui.campaign_analysis) } else { leptos_i18n::t_string!(i18n, ui.campaigns) }).to_owned()) subtitle=Signal::derive(move || (if analysis { leptos_i18n::t_string!(i18n, ui.select_a_campaign_to_inspect_its_recorded_events_and_recipient_outcomes) } else { leptos_i18n::t_string!(i18n, ui.prepare_a_simulation_and_follow_its_delivery_progress) }).to_owned())>
        <ActionButton disabled=Signal::derive(move||busy.get()||mutating.get()) on_click=move |_|refresh.update(|v|*v+=1)>{tr!(ui.refresh)}</ActionButton>
        <ActionLink style=ButtonStyle::Primary hidden={!can_edit} href="/campaigns/new">{tr!(ui.create_campaign)}</ActionLink>
    </PageHeader>{message.view()}<Show when=move||busy.get()><p role="status">{tr!(ui.loading_campaigns)}</p></Show>
    <TableToolbar title=trs!(ui.campaigns)><Show when=move||!analysis><Select label=trs!(ui.campaign_filter) value=Signal::derive(move||filter.get()) on_change=Callback::new(move|v:String|batch(move||{cursors.set(vec![String::new()]);filter.set(v);}))><option value="all">{tr!(ui.all_campaigns)}</option><option value="drafts">{tr!(ui.drafts)}</option><option value="active">{tr!(ui.active_campaigns_filter)}</option><option value="completed">{tr!(ui.completed_campaigns_filter)}</option></Select></Show></TableToolbar>
    <DataTable label=trs!(ui.campaigns) empty=Signal::derive(move||rows.get().is_empty()) headers=vec![trs!(ui.campaign),trs!(ui.lifecycle),trs!(ui.recipients),trs!(ui.launch),trs!(ui.actions)]>
        {move||rows.get().into_iter().map(|r|{
            let draft = r["kind"] == "draft";
            let id = text(&r,"id");
            let href = if draft {format!("/campaigns/drafts/{id}")}else{format!("/campaigns/{id}")};
            let analysis_href = format!("/analysis/{id}");
            let view_href = if analysis {analysis_href.clone()}else{href.clone()};
            let name = text(&r,"name");
            let target=r.clone();
            view!{<tr><th scope="row"><a href=view_href.clone()>{if name.is_empty(){leptos_i18n::t_string!(i18n, ui.untitled_campaign).to_owned()}else{name}}</a></th>
                <td>{if draft {view!{<StatusBadge tone=StatusTone::Warning>{tr!(ui.draft)}</StatusBadge>}.into_any()}else{ruphish_ui::localization::event_label(i18n,&text(&r,"status")).into_any()}}</td>
                <td>{r["total"].as_i64().map(|n|n.to_string()).unwrap_or_else(||leptos_i18n::t_string!(i18n,ui.not_set).to_owned())}</td>
                <td>{if draft {leptos_i18n::t_string!(i18n,ui.not_scheduled).to_owned()}else{instant(&r["launch_date"])}}</td>
                <td><ResourceActions view_href=view_href edit_href=format!("{href}/edit") can_edit=can_edit hide_edit=!draft disabled=Signal::derive(move||mutating.get()) on_delete=Callback::new(move|()|{delete_message.clear();deleting.set(Some(target.clone()));})>
                    <ActionLink hidden={draft||analysis} href=analysis_href>{tr!(ui.view_analysis)}</ActionLink>
                </ResourceActions></td></tr>}
        }).collect_view()}
    </DataTable>
    <Show when=move||loaded.get()&&!busy.get()&&rows.get().is_empty()><EmptyState title=Signal::derive(move||if filter.get()=="drafts"{leptos_i18n::t_string!(i18n,ui.no_drafts).to_owned()}else{leptos_i18n::t_string!(i18n,ui.no_campaigns_on_this_page).to_owned()})><p>{tr!(ui.create_a_campaign_to_select_an_audience_review_content_and_choose_a_schedule)}</p></EmptyState></Show>
    <PaginationControls disabled=Signal::derive(move||busy.get()||mutating.get()) has_previous=Signal::derive(move||cursors.with(|c|c.len()>1)) has_next=Signal::derive(move||next.get().is_some()) on_previous=Callback::new(move|()|cursors.update(|c|{c.pop();})) on_next=Callback::new(move|()|{if let Some(cursor)=next.get_untracked(){cursors.update(|c|c.push(cursor));}})/>
    {move||deleting.get().map(|target|{let draft=target["kind"]=="draft";view!{<DeleteConfirmation title=if draft {trs!(ui.delete_draft)}else{trs!(ui.delete_campaign)} description=if draft {trs!(ui.delete_draft_description)}else{trs!(ui.campaign_history_will_be_removed_already_submitted_mail_cannot_be_recalled)} confirm_label=trs!(ui.delete) busy=Signal::derive(move||mutating.get()) message=delete_message on_confirm=confirm on_cancel=Callback::new(move|()|deleting.set(None))/>}})}
    </ManagementPage>}
}
pub(super) fn detail(api: Api, token: String) -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    let drafts = use_context::<ruphish_ui::import_drafts::ImportDrafts>();
    let token = StoredValue::new(token);
    view! {<For each=move||vec![params.with(|p|p.get("id").unwrap_or_default())] key=|id|id.clone() children=move|id|{
        if id=="new" {{
            let key=query.with(|q|q.get("draft"));
            let initial=key.as_ref().and_then(|key|drafts.and_then(|store|store.get("campaigns",key))).map(|stored|stored.draft.get_untracked().value().clone());
            if let Some(key)=key {on_cleanup(move||{if let Some(store)=drafts{store.remove(&key);}}); if initial.is_none(){return view!{<ManagementPage><h1>{tr!(ui.draft_unavailable)}</h1><a href="/campaigns">{tr!(ui.return_to_campaigns)}</a></ManagementPage>}.into_any();}}
            create(api,token.get_value(),initial,None).into_any()
        }} else {show(api,token.get_value(),id).into_any()}
    }/>}
}
fn input(draft: RwSignal<Draft>, key: &'static str, label: Signal<String>) -> impl IntoView {
    view! {<Field label=label><TextInput value=Signal::derive(move||text(&draft.get().value,key)) on_input=move|e|draft.update(|d|d.edit(|v|v[key]=json!(event_target_value(&e))))/></Field>}
}
fn selector(
    draft: RwSignal<Draft>,
    resources: RwSignal<Value>,
    kind: &'static str,
    label: Signal<String>,
) -> impl IntoView {
    view! {<Select label=label value=Signal::derive(move||draft.get().value["selection"][kind]["id"].to_string()) on_change=Callback::new(move|v:String|{let id=v.parse::<i64>().unwrap_or(0);draft.update(|d|d.edit(|node|node["selection"][kind]=json!({"id":id})));})>
        <option value="0">{tr!(ui.choose_a_resource)}</option>{move||array(&resources.get(),kind).into_iter().map(|r|view!{<option value=r["id"].to_string()>{text(&r,"name")}</option>}).collect_view()}
    </Select>}
}
fn domain_selector(draft: RwSignal<Draft>, resources: RwSignal<Value>) -> impl IntoView {
    let selected = move || {
        let value = text(&draft.get().value, "url");
        url::Url::parse(&value)
            .ok()
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_default()
    };
    let path = move || {
        let value = text(&draft.get().value, "url");
        // Preserve authored Go expressions and escaping; URL::path() percent-encodes
        // template delimiters and must not become the editor's source of truth.
        let suffix = value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"))
            .map(|rest| rest.find(['/', '?']).map(|at| &rest[at..]).unwrap_or(""))
            .unwrap_or(value.as_str());
        if suffix.starts_with('/') {
            suffix.to_owned()
        } else {
            format!("/{suffix}")
        }
    };
    view! {<div><Select label=trs!(ui.verified_domain) value=Signal::derive(selected) on_change=Callback::new(move|origin:String|{let path=path();draft.update(|d|d.edit(|node|node["url"]=json!(format!("{origin}{path}"))));})><option value="">{tr!(ui.choose_a_verified_domain)}</option>{move||array(&resources.get(),"domains").into_iter().map(|d|view!{<option value=text(&d,"origin")>{format!("{} ({})",text(&d,"name"),text(&d,"hostname"))}</option>}).collect_view()}</Select>
    <Field label=trs!(ui.landing_path_optional)><TextInput value=path on_input=move|e|{let path=event_target_value(&e);let origin=selected();let path=if path.starts_with('/'){path}else{format!("/{path}")};draft.update(|d|d.edit(|v|v["url"]=json!(format!("{origin}{path}"))));}/></Field>
    <p>{move||text(&draft.get().value,"url")}</p><a href="/domains" target="_blank" rel="noopener">{tr!(ui.manage_and_verify_domains)}</a></div>}
}
fn load_page_defaults(
    tasks: ruphish_ui::view_tasks::ViewTasks,
    api: Api,
    draft: RwSignal<Draft>,
    id: i64,
    message: Message,
) {
    let i18n = ruphish_ui::localization::use_i18n();

    message.clear();
    let Some(request) = draft.try_update(|d| d.select_page(id)).flatten() else {
        return;
    };
    tasks.spawn(async move {
        let result = api.get(&format!("/api/pages/{id}")).await;
        let accepted = draft
            .try_update(|d| d.page_loaded(request, result.as_ref().ok()))
            .unwrap_or(false);
        if accepted && !draft.with(Draft::policy_ready) {
            message.error(match result {
                Err(e) => {
                    leptos_i18n::t_string!(
                        i18n,
                        messages
                            .cannot_load_landing_page_defaults_arg0_retry_or_select_another_page,
                        arg0 = (e).to_string()
                    )
                }
                Ok(_) => leptos_i18n::t_string!(
                    i18n,
                    ui.landing_page_defaults_are_incomplete_retry_or_select_another_page
                )
                .into(),
            });
        }
    });
}
fn page_selector(
    tasks: ruphish_ui::view_tasks::ViewTasks,
    api: Api,
    draft: RwSignal<Draft>,
    resources: RwSignal<Value>,
    message: Message,
) -> impl IntoView {
    view! {<Select label=trs!(ui.landing_page) value=Signal::derive(move||draft.get().value["selection"]["page"]["id"].to_string()) on_change=Callback::new(move|v:String|load_page_defaults(tasks,api,draft,v.parse::<i64>().unwrap_or(0),message))>
        <option value="0">{tr!(ui.choose_a_resource)}</option>{move||array(&resources.get(),"page").into_iter().map(|r|view!{<option value=r["id"].to_string()>{text(&r,"name")}</option>}).collect_view()}
    </Select>}
}
fn flag(draft: RwSignal<Draft>, key: &'static str, label: Signal<String>) -> impl IntoView {
    view! {<Checkbox label=label
    checked=Signal::derive(move||draft.get().value["policy"][key].as_bool().unwrap_or(false))
    on_toggle=Callback::new(move|checked|draft.update(|d|d.set_tracking(key=="capture_passwords",checked)))/>}
}
fn schedule(
    draft: RwSignal<Draft>,
    key: &'static str,
    label: Signal<String>,
    zone: StoredValue<String>,
) -> impl IntoView {
    view! {<section class="resource-section"><h2>{move || label.get()}</h2><p>{tr!(ui.leave_blank_for_no_explicit_time_daylight_saving_changes_are_handled_automatical)}</p>
        <div class="resource-columns"><Field label=trs!(ui.local_date_and_time)><TextInput input_type="datetime-local" value=Signal::derive(move||text(&draft.get().value[key],"local")) on_input=move|e|{let local=event_target_value(&e);draft.update(|d|d.edit(|v|{if local.is_empty(){if let Some(v)=v.as_object_mut(){v.remove(key);}}else{let selected_zone=v[key]["time_zone"].as_str().unwrap_or(&zone.get_value()).to_owned();v[key]=json!({"local":local,"time_zone":selected_zone});}}));}/></Field>
        <Field label=trs!(ui.time_zone)><TextInput disabled=Signal::derive(move||draft.get().value[key].is_null()) placeholder=zone.get_value() value=Signal::derive(move||text(&draft.get().value[key],"time_zone")) on_input=move|e|draft.update(|d|d.edit(|v|v[key]["time_zone"]=json!(event_target_value(&e))))/></Field></div>
    </section>}
}
pub(super) fn create(
    api: Api,
    token: String,
    initial: Option<Value>,
    saved: Option<Value>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let token = StoredValue::new(token);
    let navigate = use_navigate();
    let zone = StoredValue::new(local_zone());
    let generation = u64::from(initial.is_some() && saved.is_none());
    let persistence = crate::campaign_drafts::Persistence::new(saved.as_ref());
    let initial_step = saved
        .as_ref()
        .and_then(|s| s["step"].as_u64())
        .unwrap_or(0)
        .min(3) as usize;
    let draft = RwSignal::new(Draft {
        generation,
        value: initial.unwrap_or_else(||json!({"name":"","url":"","selection":{"groups":[],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}},"policy":{"capture_credentials":false,"capture_passwords":false,"redirect_url":""}})),
        ..Default::default()
    });
    let resources = RwSignal::new(json!({}));
    let busy = RwSignal::new(false);
    let loading = RwSignal::new(true);
    let step = RwSignal::new(initial_step);
    let message = Message::new(String::new());
    let can_edit = editable();
    crate::navigation::protect_draft(move || {
        draft.with(|d| d.generation != persistence.saved_generation.get() || d.launch.is_some())
    });
    let reload = RwSignal::new(0u64);
    let recipient = RwSignal::new(String::new());
    Effect::new(move |_| {
        let _ = reload.get();
        loading.set(true);
        spawn_local(async move {
            for (family, key) in [
                ("groups", "groups"),
                ("templates", "template"),
                ("pages", "page"),
                ("smtp", "smtp"),
            ] {
                match api.get(&format!("/api/{family}/summary")).await {
                    Ok(v) => resources.update(|r| {
                        r[key] = json!(array(
                            &v,
                            if family == "groups" {
                                "groups"
                            } else {
                                "items"
                            }
                        ))
                    }),
                    Err(e) => message.error(e),
                }
            }
            match api.get("/api/domains/validated/").await {
                Ok(v) => resources.update(|r| r["domains"] = v),
                Err(e) => message.error(e),
            }
            loading.set(false);
        });
    });
    let retry_review = RwSignal::new(0u64);
    let review_flow = StoredValue::new(ruphish_ui::campaign_state::ReviewFlow::default());
    // Review output/loading/error updates must not retrigger their own request.
    let review_inputs = Memo::new(move |_| {
        (
            step.get(),
            retry_review.get(),
            loading.get(),
            draft.with(|d| (d.generation, d.policy_ready(), d.launch.is_some())),
        )
    });
    Effect::new(move |_| {
        let (step, retry, loading, _) = review_inputs.get();
        let mut request = None;
        draft.update(|state| {
            review_flow.update_value(|flow| {
                request = flow.update(state, step == 3, loading, can_edit, retry);
            })
        });
        let Some((request, body)) = request else {
            return;
        };
        spawn_local(async move {
            let result = api
                .resource(
                    "POST",
                    "/api/campaign-drafts/prepare",
                    &token.get_value(),
                    Some(&body),
                    None,
                )
                .await
                .map(|(value, _)| value);
            draft.update(|state| {
                state.reviewed(request, result);
            });
        });
    });
    let send_test = move |_| {
        if busy.get_untracked() {
            return;
        }
        let d = draft.get_untracked();
        let Some(review) = d.review else {
            return;
        };
        let mut campaign = d.value;
        campaign["selection"] = review["selection"].clone();
        campaign["policy"] = review["policy"].clone();
        busy.set(true);
        tasks.spawn(async move {
            match api
                .resource(
                    "POST",
                    "/api/campaigns/test",
                    &token.get_value(),
                    Some(&json!({"campaign":campaign,"email":recipient.get_untracked()})),
                    None,
                )
                .await
            {
                Ok(_) => message.status(
                    leptos_i18n::t_string!(i18n, ui.test_accepted_by_smtp_server).to_owned(),
                ),
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    };
    let save_navigate = navigate.clone();
    let save = Callback::new(move |destination: Option<usize>| {
        if busy.get_untracked()
            || draft.with_untracked(|d| d.launch.is_some() || d.page_request.is_some())
        {
            return;
        }
        busy.set(true);
        message.clear();
        let snapshot = draft.get_untracked();
        let next = destination.unwrap_or(step.get_untracked());
        let first = persistence.revision.get_untracked().is_none();
        let navigate = save_navigate.clone();
        tasks.spawn(async move {
            match persistence
                .save(api, &token.get_value(), &snapshot, next)
                .await
            {
                Ok(_) => {
                    if destination.is_none() {
                        navigate(
                            &format!("/campaigns/drafts/{}", persistence.id.get_untracked()),
                            Default::default(),
                        );
                    } else if first && draft.with_untracked(|d| d.generation == snapshot.generation)
                    {
                        navigate(
                            &format!("/campaigns/drafts/{}/edit", persistence.id.get_untracked()),
                            Default::default(),
                        );
                    } else {
                        step.set(next);
                    }
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    });
    let launch = move |_| {
        if busy.get_untracked() {
            return;
        }
        let key = match random_key() {
            Ok(v) => v,
            Err(_) => {
                message.error(
                    leptos_i18n::t_string!(i18n, ui.cannot_generate_a_secure_launch_key).to_owned(),
                );
                return;
            }
        };
        let Some((key, body)) = draft.try_update(|d| d.launch(key)).flatten() else {
            return;
        };
        busy.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            let snapshot = draft.get_untracked();
            if (snapshot.generation != persistence.saved_generation.get_untracked()
                || persistence.revision.get_untracked().is_none())
                && let Err(e) = persistence
                    .save(api, &token.get_value(), &snapshot, step.get_untracked())
                    .await
            {
                draft.update(Draft::rejected);
                message.error(e);
                busy.set(false);
                return;
            }
            let request = json!({"revision":persistence.revision.get_untracked(),"campaign":body});
            let path = format!(
                "/api/campaign-drafts/{}/launch",
                persistence.id.get_untracked()
            );
            match api.launch(&path, &token.get_value(), &key, &request).await {
                Ok(v) => {
                    draft.update(|d| {
                        d.generation = persistence.saved_generation.get_untracked();
                        d.launch = None;
                    });
                    navigate(&format!("/campaigns/{}", v["id"]), Default::default());
                }
                Err((status, e)) => {
                    if (400..500).contains(&status) {
                        draft.update(Draft::rejected);
                        retry_review.update(|generation| *generation += 1);
                    }
                    message.error(e);
                }
            }
            busy.set(false);
        });
    };
    view! {<ManagementPage><BackLink href="/campaigns">{tr!(ui.back_to_campaigns)}</BackLink><PageHeader title=Signal::derive(move||if persistence.revision.get().is_some(){leptos_i18n::t_string!(i18n,ui.edit_campaign_draft).to_string()}else{leptos_i18n::t_string!(i18n,ui.create_campaign).to_string()}) subtitle=trs!(ui.review_the_audience_content_and_schedule_before_launching)><span role="status">{move||if persistence.saving.get(){leptos_i18n::t_string!(i18n,ui.draft_saving).to_string()}else if draft.with(|d|d.generation!=persistence.saved_generation.get())||persistence.revision.get().is_none(){leptos_i18n::t_string!(i18n,ui.draft_unsaved).to_string()}else{format!("{}: {}",leptos_i18n::t_string!(i18n,ui.draft_saved),instant(&persistence.modified.get()))}}</span><ActionButton disabled=Signal::derive(move||busy.get()||loading.get()||draft.with(|d|d.launch.is_some())) on_click=move |_|reload.update(|v|*v+=1)>{tr!(ui.refresh_resources)}</ActionButton></PageHeader>
        {message.view()}<Show when=move||loading.get()><p role="status">{tr!(ui.loading_resources)}</p></Show>
        <Show when=move||!loading.get() && ["template","page","smtp","groups"].into_iter().any(|key| {
            let value=draft.get().value;
            let selections=if key=="groups"{array(&value["selection"],"groups")}else{vec![value["selection"][key].clone()]};
            selections.iter().any(|selected|selected["id"].as_i64().is_some_and(|id|id>0)&&!array(&resources.get(),key).iter().any(|r|r["id"]==selected["id"]))
        })><p class="form-message error">{tr!(ui.missing_draft_resource)}</p></Show>
        <ol class="campaign-steps">{[trs!(ui.audience),trs!(ui.content_and_destination),trs!(ui.schedule),trs!(ui.review_and_launch)].into_iter().enumerate().map(|(index,label)|view!{<li aria-current=move||(step.get()==index).then_some("step")>{move || label.get()}</li>}).collect_view()}</ol>
        <FormFields disabled=Signal::derive(move||busy.get()||loading.get()||!can_edit||draft.with(|d|d.launch.is_some()))>
        <section hidden=move||step.get()!=0><h2>{tr!(ui.audience)}</h2>{input(draft, "name", trs!(ui.campaign_name))}
        <p>{tr!(ui.choose_recipient_groups_overlaps_are_counted_once_by_exact_email_address_the_fir)}</p>
        {move||array(&resources.get(),"groups").into_iter().map(|r|{let id=r["id"].as_i64().unwrap_or(0);view!{<Checkbox label=leptos_i18n::t_string!(i18n, messages.group_count, name = text(&r,"name"), count = r["num_targets"].to_string()) checked=Signal::derive(move||draft.with(|d|array(&d.value["selection"],"groups").iter().any(|r|r["id"]==id))) on_toggle=Callback::new(move|checked|draft.update(|d|d.edit(|v|{let mut groups=array(&v["selection"],"groups");groups.retain(|r|r["id"]!=id);if checked{groups.push(json!({"id":id}));}v["selection"]["groups"]=json!(groups);})))/>}}).collect_view()}
        {move||array(&draft.get().value["selection"],"groups").into_iter().filter(|selected|!loading.get()&&!array(&resources.get(),"groups").iter().any(|r|r["id"]==selected["id"])).map(|selected|{let id=selected["id"].as_i64().unwrap_or(0);view!{<Checkbox label=leptos_i18n::t_string!(i18n,messages.unavailable_draft_group,id=id.to_string()) checked=true on_toggle=Callback::new(move|_:bool|draft.update(|d|d.edit(|v|{let mut groups=array(&v["selection"],"groups");groups.retain(|r|r["id"]!=id);v["selection"]["groups"]=json!(groups);}))) />}}).collect_view()}
        <a href="/groups/new" target="_blank" rel="noopener">{tr!(ui.create_a_group_in_a_new_tab)}</a></section>
        <section hidden=move||step.get()!=1><h2>{tr!(ui.content_and_destination)}</h2><div class="resource-columns">{selector(draft, resources, "template", trs!(ui.email_template))}{selector(draft, resources, "smtp", trs!(ui.sending_profile))}{page_selector(tasks,api,draft,resources,message)}{domain_selector(draft,resources)}</div>
        <ul class="campaign-resource-links" aria-label=tr!(ui.create_resources_in_a_new_tab)><li><a href="/templates/new" target="_blank" rel="noopener">{tr!(ui.create_template)}</a></li><li><a href="/pages/new" target="_blank" rel="noopener">{tr!(ui.create_landing_page)}</a></li><li><a href="/smtp/new" target="_blank" rel="noopener">{tr!(ui.create_sending_profile)}</a></li></ul><p class="resource-usage">{tr!(ui.changes_to_saved_templates_or_sending_profiles_can_affect_scheduled_emails_and_r)}</p>
        <section class="resource-section"><h2>{tr!(ui.event_tracking_and_redirect_optional)}</h2><p>{tr!(ui.selecting_a_landing_page_fills_these_fields_with_its_defaults_edit_them_for_this)}</p><p>{tr!(ui.track_submission_and_password_presence_only_entered_usernames_passwords_and_othe)}</p>
        <Show when=move||draft.with(|d|d.page_request.is_some())><p role="status">{tr!(ui.loading_landing_page_defaults)}</p></Show><Show when=move||draft.with(|d|d.page_request.is_none()&&!d.policy_ready())><ActionButton kind="button" on_click=move |_|load_page_defaults(tasks,api,draft,draft.get_untracked().value["selection"]["page"]["id"].as_i64().unwrap_or(0),message)>{tr!(ui.retry_loading_defaults)}</ActionButton></Show><FormFields disabled=Signal::derive(move||!draft.with(Draft::policy_ready))>{flag(draft, "capture_credentials", trs!(ui.record_form_submission))}{flag(draft, "capture_passwords", trs!(ui.record_password_presence))}
        <Field label=trs!(ui.redirect_url_optional_leave_empty_for_no_redirect)><TextInput value=Signal::derive(move||text(&draft.get().value["policy"],"redirect_url")) on_input=move|e|draft.update(|d|d.edit(|v|v["policy"]["redirect_url"]=json!(event_target_value(&e))))/></Field></FormFields></section></section>
        <section hidden=move||step.get()!=2>{schedule(draft, "launch_local", trs!(ui.launch_time), zone)}{schedule(draft, "send_by_local", trs!(ui.initial_distribution_end_optional), zone)}<p>{tr!(ui.with_no_launch_time_delivery_starts_after_launch_the_distribution_end_spaces_ini)}</p></section>
        <section hidden=move||step.get()!=3 aria-busy=move||draft.with(|d|d.review_request.is_some()).to_string()><h2>{tr!(ui.review)}</h2><p>{tr!(ui.check_the_validated_audience_content_and_schedule_before_launching)}</p>
        <Show when=move||draft.with(|d|d.review_request.is_some())><p role="status">{tr!(ui.validating_campaign_and_preparing_preview)}</p></Show>
        <Show when=move||!loading.get()&&!draft.with(Draft::policy_ready)><p role="status">{tr!(ui.landing_page_defaults_must_finish_loading_before_review_if_loading_failed_return)}</p></Show>
        {move||draft.get().review_error.map(|error|view!{<div class="form-message error" role="alert"><p>{error}</p><p>{tr!(ui.go_back_to_correct_the_campaign_or_retry_if_the_request_failed)}</p><ActionButton kind="button" on_click=move |_|retry_review.update(|n|*n+=1)>{tr!(ui.retry_validation)}</ActionButton></div>})}
        {move||draft.get().review.map(|r|view!{<div class="campaign-review"><h3>{leptos_i18n::t!(i18n, messages.arg0_unique_recipients, arg0 = (r["total"]).to_string())}</h3><p>{leptos_i18n::t!(i18n, messages.arg0_overlapping_recipient_entries_removed, arg0 = (r["duplicate_count"]).to_string())}</p>
        {(r["conflict_count"].as_u64().unwrap_or(0)>0).then(||view!{<details><summary>{leptos_i18n::t!(i18n, messages.arg0_conflicting_duplicate_entries_first_group_wins, arg0 = (r["conflict_count"]).to_string())}</summary><p>{tr!(ui.showing_up_to_100_conflicts_review_the_retained_and_discarded_recipient_attribut)}</p><ul>{array(&r,"conflicts").into_iter().map(|c|view!{<li>{leptos_i18n::t_string!(i18n, messages.email_retained_kept_discarded_discarded, email = text(&c,"email"), kept = c["kept"].to_string(), discarded = c["discarded"].to_string())}</li>}).collect_view()}</ul></details>})}
        <dl class="resource-facts">{["template","page","smtp"].into_iter().map(|key|view!{<div><dt>{match key { "template" => leptos_i18n::t_string!(i18n, ui.email_template), "page" => leptos_i18n::t_string!(i18n, ui.landing_page), _ => leptos_i18n::t_string!(i18n, ui.sending_profile) }}</dt><dd><a target="_blank" rel="noopener" href=format!("/{}/{}",match key{"template"=>"templates","page"=>"pages",_=>"smtp"},r["selection"][key]["id"])>{text(&r["selection"][key],"name")}</a></dd></div>}).collect_view()}
        <div><dt>{tr!(ui.launch)}</dt><dd class="data-value">{instant(&r["launch_date"])}</dd></div><div><dt>{tr!(ui.distribution_end)}</dt><dd class="data-value">{instant(&r["send_by_date"])}</dd></div><div><dt>{tr!(ui.submission_tracking)}</dt><dd>{if r["policy"]["capture_credentials"] == true {leptos_i18n::t_string!(i18n, ui.yes)} else {leptos_i18n::t_string!(i18n, ui.no)}}</dd></div><div><dt>{tr!(ui.password_presence_only)}</dt><dd>{if r["policy"]["capture_passwords"] == true {leptos_i18n::t_string!(i18n, ui.yes)} else {leptos_i18n::t_string!(i18n, ui.no)}}</dd></div><div><dt>{tr!(ui.redirect)}</dt><dd>{text(&r["policy"],"redirect_url")}</dd></div></dl>
        <section class="resource-section"><h3>{tr!(ui.rendered_sample)}</h3><p>{tr!(ui.uses_the_first_recipient_s_details_test_emails_use_sample_recipient_preview_link)}</p><dl class="resource-facts"><div><dt>{tr!(ui.visible_from)}</dt><dd>{text(&r["preview"],"visible_sender")}</dd></div><div><dt>{tr!(ui.transport_sender)}</dt><dd>{text(&r["preview"],"transport_sender")}</dd></div><div><dt>{tr!(ui.subject)}</dt><dd>{text(&r["preview"],"subject")}</dd></div></dl><iframe class="saved-content-preview" title=trs!(ui.rendered_email_sample) sandbox="" referrerpolicy="no-referrer" srcdoc=ruphish_ui::preview_document(&text(&r["preview"],"html"))></iframe><details><summary>{tr!(ui.plain_text)}</summary><pre>{text(&r["preview"],"text")}</pre></details><Field label=trs!(ui.test_recipient_email)><TextInput input_type="email" value=Signal::derive(move||recipient.get()) on_input=move|e|recipient.set(event_target_value(&e))/></Field><ActionButton disabled=Signal::derive(move||busy.get()||recipient.get().is_empty()) on_click=send_test>{tr!(ui.send_test_email)}</ActionButton></section>
        {(!r["mailer_enabled"].as_bool().unwrap_or(false)).then(||view!{<p class="form-message error">{tr!(ui.the_mailer_is_disabled_this_campaign_will_remain_queued_until_an_enabled_admin_p)}</p>})}</div>})}</section>
        </FormFields>
        <FormFooter><ActionLink href="/campaigns" style=ButtonStyle::Ghost>{tr!(ui.cancel)}</ActionLink><ActionButton disabled=Signal::derive(move||busy.get()||!can_edit||step.get()==0||draft.with(|d|d.launch.is_some()||d.page_request.is_some())) on_click=move |_|save.run(Some(step.get_untracked().saturating_sub(1)))>{tr!(ui.back)}</ActionButton>
        <ActionButton disabled=Signal::derive(move||busy.get()||!can_edit||draft.with(|d|d.launch.is_some()||d.page_request.is_some())) on_click=move |_|save.run(None)>{move||if persistence.revision.get().is_some(){leptos_i18n::t_string!(i18n,ui.save_changes)}else{leptos_i18n::t_string!(i18n,ui.save_as_draft)}}</ActionButton>
        <ActionButton style=ButtonStyle::Primary hidden=Signal::derive(move||step.get()==3) disabled=Signal::derive(move||busy.get()||loading.get()||!can_edit||draft.with(|d|d.page_request.is_some())) on_click=move |_|save.run(Some((step.get_untracked()+1).min(3)))>{tr!(ui.continue_action)}</ActionButton>
        <ActionButton style=ButtonStyle::Primary hidden=Signal::derive(move||step.get()!=3) disabled=Signal::derive(move||busy.get()||loading.get()||!can_edit||draft.with(|d|d.review.is_none()||d.review_request.is_some())) on_click=launch>{move||if draft.with(|d|d.launch.is_some()){ leptos_i18n::t_string!(i18n, ui.retry_same_launch) }else if !draft.get().value["launch_local"].is_null(){leptos_i18n::t_string!(i18n,ui.schedule_campaign)}else{leptos_i18n::t_string!(i18n, ui.launch_campaign)}}</ActionButton></FormFooter>
    </ManagementPage>}
}

fn show(api: Api, token: String, id: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let id = StoredValue::new(id);
    let drafts = use_context::<ruphish_ui::import_drafts::ImportDrafts>();
    let duplicate_navigate = use_navigate();
    let token = StoredValue::new(token);
    let navigate = use_navigate();
    let can_edit = editable();
    let data = RwSignal::new(Value::Null);
    let pages = RwSignal::new(CursorPages::default());
    let after = Memo::new(move |_| pages.with(CursorPages::after));
    let refresh = RwSignal::new(0u64);
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let loading = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let updated = RwSignal::new(String::new());
    let failures = RwSignal::new(0u32);
    let next_poll = RwSignal::new(0f64);
    let action = RwSignal::new(None::<(String, Value)>);
    Effect::new(move |_| {
        let generation = refresh.get();
        let cursor = after.get();
        if loading.get_untracked() {
            return;
        }
        loading.set(true);
        tasks.spawn(async move {
            let result = api
                .get(&format!(
                    "/api/campaigns/{}/delivery?after={cursor}",
                    id.get_value()
                ))
                .await;
            if generation == refresh.get_untracked() && cursor == after.get_untracked() {
                match result {
                    Ok(v) => {
                        pages.update(|p| {
                            p.received(cursor, v["next_after"].as_i64());
                        });
                        data.set(v);
                        updated.set(instant(&Value::from(
                            ruphish_domain::datetime::now().as_second(),
                        )));
                        if failures.get_untracked() > 0 {
                            message.clear();
                        }
                        failures.set(0);
                    }
                    Err(e) => {
                        failures.update(|f| *f = (*f + 1).min(5));
                        message.error(leptos_i18n::t_string!(
                            i18n,
                            messages.refresh_failed_displayed_data_may_be_stale_arg0,
                            arg0 = (e).to_string()
                        ));
                    }
                }
            }
            loading.set(false);
            if generation != refresh.get_untracked() || cursor != after.get_untracked() {
                refresh.update(|v| *v += 1);
            }
            next_poll
                .set(js_sys::Date::now() + 5000.0 * 2f64.powi(failures.get_untracked() as i32));
        });
    });
    let focus = window_event_listener(leptos::ev::focus, move |_| {
        refresh.update(|v| *v += 1);
    });
    on_cleanup(move || focus.remove());
    if let Ok(timer) = set_interval_with_handle(
        move || {
            if !loading.get_untracked()
                && !busy.get_untracked()
                && js_sys::Date::now() >= next_poll.get_untracked()
                && web_sys::window()
                    .and_then(|w| w.document())
                    .is_some_and(|d| !d.hidden())
            {
                refresh.update(|r| *r += 1);
            }
        },
        std::time::Duration::from_secs(1),
    ) {
        on_cleanup(move || timer.clear());
    }
    let duplicate = move |_| {
        if busy.get_untracked() {
            return;
        }
        let navigate = duplicate_navigate.clone();
        busy.set(true);
        tasks.spawn(async move {
            match api
                .get(&format!("/api/campaigns/{}/draft", id.get_value()))
                .await
            {
                Ok(value) => {
                    let inserted = drafts
                        .ok_or_else(|| {
                            leptos_i18n::t_string!(i18n, ui.draft_store_unavailable).to_owned()
                        })
                        .and_then(|store| {
                            store
                                .insert(
                                    "campaigns",
                                    ruphish_ui::resource_state::ResourceDraft::new(value, None),
                                    ruphish_ui::import_state::ImportReview {
                                        notes: vec![],
                                        rejected_rows: vec![],
                                        requires_acknowledgement: false,
                                        acknowledged: true,
                                    },
                                )
                                .map_err(|e| ruphish_ui::localization::import_error(i18n, &e))
                        });
                    match inserted {
                        Ok(key) => {
                            navigate(&format!("/campaigns/new?draft={key}"), Default::default())
                        }
                        Err(e) => message.error(e),
                    }
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    };
    let confirm = Callback::new(move |()| {
        let Some((kind, row)) = action.get_untracked() else {
            return;
        };
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            let (method, path, body) = match kind.as_str() {
                "delete" => ("DELETE", format!("/api/campaigns/{}", id.get_value()), None),
                "complete" => (
                    "POST",
                    format!("/api/campaigns/{}/complete", id.get_value()),
                    None,
                ),
                _ => (
                    "POST",
                    format!("/api/campaigns/{}/delivery/{}", id.get_value(), row["id"]),
                    Some(json!({"action":kind,"generation":row["generation"]})),
                ),
            };
            match api
                .resource(method, &path, &token.get_value(), body.as_ref(), None)
                .await
            {
                Ok(_) => {
                    action.set(None);
                    message.status(leptos_i18n::t_string!(i18n, ui.campaign_updated).to_owned());
                    if kind == "delete" {
                        navigate("/campaigns", Default::default());
                    } else {
                        refresh.update(|r| *r += 1);
                    }
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    });
    view! {<ManagementPage><BackLink href="/campaigns">{tr!(ui.back_to_campaigns)}</BackLink><PageHeader title=Signal::derive(move||text(&data.get()["campaign"],"name")) subtitle=Signal::derive(move||leptos_i18n::t_string!(i18n, messages.arg0_last_refreshed_arg1, arg0 = ruphish_ui::localization::event_label(i18n, &text(&data.get()["campaign"],"status")), arg1 = (updated.get()).to_string()))>
        <ActionLink href=format!("/analysis/{}",id.get_value())>{tr!(ui.view_analysis)}</ActionLink><ActionButton disabled=Signal::derive(move||busy.get()) busy=Signal::derive(move||loading.get().to_string()) on_click=move |_|refresh.update(|r|*r+=1)>{move||if loading.get(){leptos_i18n::t_string!(i18n, ui.refreshing)}else{leptos_i18n::t_string!(i18n, ui.refresh)}}</ActionButton><ActionButton hidden=!can_edit disabled=Signal::derive(move||busy.get()) on_click=duplicate>{tr!(ui.duplicate)}</ActionButton><ActionButton hidden=!can_edit disabled=Signal::derive(move||busy.get()||data.get()["campaign"]["status"]== "Completed") on_click=move |_|action.set(Some(("complete".into(),Value::Null)))>{tr!(ui.complete_campaign)}</ActionButton><ActionButton style=ButtonStyle::Ghost hidden=!can_edit disabled=Signal::derive(move||busy.get()) on_click=move |_|action.set(Some(("delete".into(),Value::Null)))>{tr!(ui.delete)}</ActionButton>
    </PageHeader>{message.view()}<Show when=move||data.get()["mailer_enabled"]==false><p class="form-message error">{tr!(ui.delivery_processing_is_disabled_in_this_process)}</p></Show>
    <p>{tr!(ui.acceptance_means_the_smtp_server_accepted_the_message_participant_credentials_ar)}</p>
    {move||campaign_context(&data.get()["campaign"])}
    {delivery_progress(data)}
    <DataTable label=trs!(ui.campaign_delivery) headers=vec![trs!(ui.history), trs!(ui.recipient), trs!(ui.delivery), trs!(ui.next_last_due), trs!(ui.attempts_and_actions)]>
        <For each=move||{array(&data.get(),"items").into_iter().filter(|_|pages.with(CursorPages::loaded)).filter_map(|r|r["id"].as_i64()).collect::<Vec<_>>()} key=|id|*id children=move|result|{crate::campaign_delivery::recipient_row(api,id,result,data,crate::campaign_delivery::Controls {busy,can_edit,action})}/>
    </DataTable><PageControls pages=pages disabled=Signal::derive(move||loading.get()||busy.get())/>
    <section class="resource-section"><h2>{tr!(ui.recent_activity)}</h2><p>{tr!(ui.latest_50_campaign_events_expand_a_recipient_above_for_their_timeline_and_earlie)}</p><DataTable label=trs!(ui.campaign_activity) headers=vec![trs!(ui.time),trs!(ui.recipient),trs!(ui.event)]>{move||array(&data.get(),"activity").into_iter().map(|e|view!{<tr><td>{instant(&e["time"])}</td><td>{text(&e,"email")}</td><td>{ruphish_ui::localization::event_label(i18n,&text(&e,"message"))}</td></tr>}).collect_view()}</DataTable></section>
    {move||action.get().map(|(kind,_)|{let (title,description,label)=match kind.as_str(){"delete"=>(leptos_i18n::t_string!(i18n, ui.delete_campaign),leptos_i18n::t_string!(i18n, ui.campaign_history_will_be_removed_already_submitted_mail_cannot_be_recalled),leptos_i18n::t_string!(i18n, ui.delete_campaign_2)),"complete"=>(leptos_i18n::t_string!(i18n, ui.complete_campaign_2),leptos_i18n::t_string!(i18n, ui.pending_delivery_will_stop_already_armed_or_in_flight_submissions_may_still_fini),leptos_i18n::t_string!(i18n, ui.complete_campaign)),"resend_unknown"=>(leptos_i18n::t_string!(i18n, ui.resend_despite_an_unknown_outcome),leptos_i18n::t_string!(i18n, ui.check_your_mail_server_first_the_earlier_attempt_may_have_been_accepted_resendin),leptos_i18n::t_string!(i18n, ui.resend)),"confirm_accepted"=>(leptos_i18n::t_string!(i18n, ui.confirm_smtp_acceptance),leptos_i18n::t_string!(i18n, ui.use_the_attempt_message_id_to_verify_acceptance_in_your_mail_server_before_recor),leptos_i18n::t_string!(i18n, ui.confirm_accepted)),_=>(leptos_i18n::t_string!(i18n, ui.retry_this_delivery),leptos_i18n::t_string!(i18n, ui.correct_the_reported_content_account_or_sending_profile_problem_before_retrying),leptos_i18n::t_string!(i18n, ui.retry))};let counts=data.get_untracked();let description=if matches!(kind.as_str(),"complete"|"delete"){leptos_i18n::t_string!(i18n, messages.arg0_pending_arg1_preparing_arg2_submitting_arg3, arg0 = (description).to_string(), arg1 = (["ready","retry","blocked"].iter().map(|k|counts["counts"][*k].as_i64().unwrap_or(0)).sum::<i64>()).to_string(), arg2 = (counts["counts"]["preparing"].as_i64().unwrap_or(0)).to_string(), arg3 = (counts["counts"]["submitting"].as_i64().unwrap_or(0)).to_string())}else{description.to_owned()};view!{<DeleteConfirmation title=title description=description confirm_label=label busy=Signal::derive(move||busy.get()) on_confirm=confirm on_cancel=Callback::new(move|()|action.set(None)) message=message/>}})}
    </ManagementPage>}
}

fn delivery_progress(data: RwSignal<Value>) -> impl IntoView {
    let states = [
        (
            "ready",
            trs!(ui.ready),
            "neutral",
            trs!(ui.queued_waiting_for_its_scheduled_time_or_an_available_worker),
        ),
        (
            "preparing",
            trs!(ui.preparing),
            "active",
            trs!(
                ui.a_worker_claimed_this_recipient_and_is_preparing_the_sending_attempt_after_a_cra
            ),
        ),
        (
            "submitting",
            trs!(ui.submitting),
            "active",
            trs!(
                ui.submission_has_been_authorized_but_smtp_acceptance_is_not_yet_recorded_after_a_c
            ),
        ),
        (
            "retry",
            trs!(ui.retry),
            "warning",
            trs!(ui.a_retryable_failure_was_recorded_another_attempt_is_scheduled_with_a_delay),
        ),
        (
            "accepted",
            trs!(ui.accepted),
            "success",
            trs!(
                ui.smtp_acceptance_was_recorded_or_explicitly_confirmed_by_an_operator_using_mail_s
            ),
        ),
        (
            "failed",
            trs!(ui.failed),
            "failure",
            trs!(
                ui.content_preparation_failed_the_message_was_permanently_rejected_or_the_retry_lim
            ),
        ),
        (
            "blocked",
            trs!(ui.blocked),
            "warning",
            trs!(
                ui.an_account_resource_or_smtp_configuration_problem_prevents_automatic_progress_co
            ),
        ),
        (
            "unknown",
            trs!(ui.unknown),
            "warning",
            trs!(
                ui.the_message_may_have_been_accepted_but_its_outcome_could_not_be_confirmed_check
            ),
        ),
        (
            "cancelled",
            trs!(ui.cancelled),
            "neutral",
            trs!(ui.pending_work_was_cancelled_messages_already_submitted_cannot_be_recalled),
        ),
    ];
    view! {<Card class="resource-read-panel delivery-progress" label=trs!(ui.delivery_progress)>
        <h2>{tr!(ui.delivery_progress)}</h2>
        <dl class="delivery-metrics">{states.into_iter().map(|(state,label,tone,explanation)|view!{
            <div class=format!("delivery-metric {tone}") class:has-count={move||data.get()["counts"][state].as_i64().unwrap_or(0)>0}>
                <dt><span>{move || label.get()}</span><InfoPopover id=format!("delivery-help-{state}") label=label explanation=explanation/></dt>
                <dd>{move||data.get()["counts"][state].as_i64().unwrap_or(0)}</dd>
            </div>
        }).collect_view()}</dl>
    </Card>}
}

fn campaign_context(c: &Value) -> impl IntoView + use<> {
    let i18n = ruphish_ui::localization::use_i18n();

    let resource = |key: &str, label: Signal<String>, route: &str| {
        let id = c[key]["id"].as_i64().unwrap_or(0);
        let name = c[key]["name"].as_str().map(str::to_owned);
        view! {<div><dt>{move || label.get()}</dt><dd>{match name {
            Some(name)=>view!{<a href=format!("/{route}/{id}")>{name}</a>}.into_any(),
            None=>view!{<span>{move || if id==0 {leptos_i18n::t_string!(i18n, ui.not_selected).into()} else {leptos_i18n::t_string!(i18n, messages.unavailable_id_arg0, arg0 = (id).to_string())}}</span>}.into_any()
        }}</dd></div>}
    };
    let resources = view! {{resource("template",trs!(ui.email_template),"templates")}{resource("page",trs!(ui.landing_page),"pages")}{resource("smtp",trs!(ui.sending_profile),"smtp")}};
    let c = c.clone();
    let template_sender = text(&c, "template_sender");
    view! {<Card class="resource-read-panel campaign-context" label=trs!(ui.campaign_details)><h2>{tr!(ui.campaign_details)}</h2>
        {(c["domain_block"].is_string() && c["status"]!= "Completed").then(||view!{<p role="status" class="form-message">{tr!(ui.delivery_is_waiting_for_domain_verification_queued_recipients_are_retained_sendi)} <a href=c["domain_id"].as_i64().map(|id|format!("/domains/{id}")).unwrap_or_else(||"/domains".into())>{tr!(ui.review_domain)}</a></p>})}
        <dl class="resource-facts">
            {resources}
            <div><dt>{tr!(ui.email_subject_template)}</dt><dd>{c["subject"].as_str().map(|s|if s.is_empty(){leptos_i18n::t_string!(i18n, ui.no_subject)}else{s}).unwrap_or(leptos_i18n::t_string!(i18n, ui.unavailable)).to_owned()}</dd></div>
            <div><dt>{tr!(ui.sending_profile_from_address)}</dt><dd>{c["profile_sender"].as_str().unwrap_or(leptos_i18n::t_string!(i18n, ui.unavailable)).to_owned()}</dd></div>
            <div><dt>{tr!(ui.template_from_override)}</dt><dd>{move || if template_sender.is_empty(){leptos_i18n::t_string!(i18n, ui.use_sending_profile_address).into()}else{template_sender.clone()}}</dd></div>
            <div><dt>{tr!(ui.scheduled_launch)}</dt><dd class="data-value">{instant(&c["launch_date"])}</dd></div>
            <div><dt>{tr!(ui.initial_distribution_end)}</dt><dd>{if text(&c,"send_by_date").starts_with("0001-"){leptos_i18n::t_string!(i18n, ui.no_distribution_window).into()}else{instant(&c["send_by_date"])}}</dd></div>
            <div><dt>{tr!(ui.public_campaign_url)}</dt><dd>{text(&c,"url")}</dd></div>
        </dl>
        <p class="resource-usage">{leptos_i18n::t!(i18n, messages.times_shown_in_arg0_template_and_sending_profile_details_reflect_the_current_sav, arg0 = (local_zone()).to_string())}</p>
    </Card>}
}

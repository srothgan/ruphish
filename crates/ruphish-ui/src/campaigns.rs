//! Campaign preparation and operational delivery views within the shared shell.
use crate::{account::Api, feedback::Message, resources::ResourceAccess, shared::components::*};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use ruphish_ui::campaign_state::Draft;
use ruphish_ui::pagination::CursorPages;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(
    inline_js = "export function campaignKey(){return crypto.randomUUID()} export function campaignZone(){return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'} export function campaignTime(json){const v=JSON.parse(json); if(v==null || v==='' || (typeof v==='string' && v.startsWith('0001-')))return 'Not set'; const d=new Date(typeof v==='number'?v*1000:v); return Number.isNaN(d.getTime())?'Time not recorded':new Intl.DateTimeFormat(undefined,{dateStyle:'medium',timeStyle:'long'}).format(d);}"
)]
extern "C" {
    #[wasm_bindgen(catch,js_name=campaignKey)]
    fn random_key() -> Result<String, JsValue>;
    #[wasm_bindgen(js_name=campaignZone)]
    fn local_zone() -> String;
    #[wasm_bindgen(js_name=campaignTime)]
    fn campaign_time(value: &str) -> String;
}
pub(super) fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
pub(super) fn array(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
fn editable() -> bool {
    use_context::<ResourceAccess>().is_some_and(|v| v.0)
}

pub(super) fn list(api: Api) -> impl IntoView {
    list_mode(api, false)
}
pub(super) fn analysis_directory(api: Api) -> impl IntoView {
    list_mode(api, true)
}
fn list_mode(api: Api, analysis: bool) -> impl IntoView {
    let rows = RwSignal::new(Vec::<Value>::new());
    let pages = RwSignal::new(CursorPages::default());
    let after = Memo::new(move |_| pages.with(CursorPages::after));
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let can_edit = editable();
    let refresh = RwSignal::new(0u64);
    let failed = RwSignal::new(false);
    Effect::new(move |_| {
        refresh.get();
        let cursor = after.get();
        busy.set(true);
        failed.set(false);
        spawn_local(async move {
            let result = api
                .get(&format!("/api/campaigns/overview?after={cursor}"))
                .await;
            if cursor != after.get_untracked() {
                return;
            }
            match result {
                Ok(value) => {
                    rows.set(array(&value, "items"));
                    pages.update(|p| {
                        p.received(cursor, value["next_after"].as_i64());
                    });
                    message.clear();
                }
                Err(e) => {
                    failed.set(true);
                    message.error(e);
                }
            }
            busy.set(false);
        });
    });
    view! {<ManagementPage><PageHeader title=if analysis { "Campaign analysis" } else { "Campaigns" } subtitle=if analysis { "Select a campaign to inspect its recorded events and recipient outcomes." } else { "Prepare a simulation and follow its delivery progress." }>
        <ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|refresh.update(|v|*v+=1)>"Refresh"</ActionButton>
        <ActionLink style=ButtonStyle::Primary hidden={ !can_edit || analysis } href="/campaigns/new">"Create campaign"</ActionLink>
    </PageHeader>{message.view()}<Show when=move||busy.get()><p role="status">"Loading campaigns…"</p></Show>
    <DataTable label="Campaigns" headers=vec!["Campaign","Lifecycle","Recipients","Launch"]>
        {move||rows.get().into_iter().filter(|_|pages.with(CursorPages::loaded)).map(|r|view!{<tr><th scope="row"><a href=format!("/{}/{}",if analysis { "analysis" } else { "campaigns" },r["id"])>{text(&r,"name")}</a></th><td>{text(&r,"status")}</td><td>{r["total"].to_string()}</td><td>{text(&r,"launch_date")}</td></tr>}).collect_view()}
    </DataTable><Show when=move||!busy.get()&&!failed.get()&&rows.get().is_empty()><EmptyState title="No campaigns on this page"><p>"Create a campaign to select an audience, review content and choose a schedule."</p></EmptyState></Show>
    <PageControls pages=pages disabled=Signal::derive(move||busy.get())/>
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
            if let Some(key)=key {on_cleanup(move||{if let Some(store)=drafts{store.remove(&key);}}); if initial.is_none(){return view!{<ManagementPage><h1>"Draft unavailable"</h1><a href="/campaigns">"Return to campaigns"</a></ManagementPage>}.into_any();}}
            create(api,token.get_value(),initial).into_any()
        }} else {show(api,token.get_value(),id).into_any()}
    }/>}
}
fn input(draft: RwSignal<Draft>, key: &'static str, label: &'static str) -> impl IntoView {
    view! {<Field label=label><input prop:value=move||text(&draft.get().value,key) on:input=move|e|draft.update(|d|d.edit(|v|v[key]=json!(event_target_value(&e))))/></Field>}
}
fn selector(
    draft: RwSignal<Draft>,
    resources: RwSignal<Value>,
    kind: &'static str,
    label: &'static str,
) -> impl IntoView {
    view! {<Field label=label><select prop:value=move||draft.get().value["selection"][kind]["id"].to_string() on:change=move|e|{let id=event_target_value(&e).parse::<i64>().unwrap_or(0);draft.update(|d|d.edit(|v|v["selection"][kind]=json!({"id":id})));}>
        <option value="0">"Choose a resource"</option>{move||array(&resources.get(),kind).into_iter().map(|r|view!{<option value=r["id"].to_string()>{text(&r,"name")}</option>}).collect_view()}
    </select></Field>}
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
    view! {<div><Field label="Verified domain"><select prop:value=selected on:change=move|e|{
        let origin=event_target_value(&e);let path=path();draft.update(|d|d.edit(|v|v["url"]=json!(format!("{origin}{path}"))));
    }><option value="">"Choose a verified domain"</option>{move||array(&resources.get(),"domains").into_iter().map(|d|view!{<option value=text(&d,"origin")>{format!("{} ({})",text(&d,"name"),text(&d,"hostname"))}</option>}).collect_view()}</select></Field>
    <Field label="Landing path (optional)"><input prop:value=path on:input=move|e|{let path=event_target_value(&e);let origin=selected();let path=if path.starts_with('/'){path}else{format!("/{path}")};draft.update(|d|d.edit(|v|v["url"]=json!(format!("{origin}{path}"))));}/></Field>
    <p>{move||text(&draft.get().value,"url")}</p><a href="/domains" target="_blank" rel="noopener">"Manage and verify domains"</a></div>}
}
fn load_page_defaults(
    tasks: ruphish_ui::view_tasks::ViewTasks,
    api: Api,
    draft: RwSignal<Draft>,
    id: i64,
    message: Message,
) {
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
                    format!("Cannot load landing-page defaults: {e}. Retry or select another page.")
                }
                Ok(_) => {
                    "Landing-page defaults are incomplete. Retry or select another page.".into()
                }
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
    view! {<Field label="Landing page"><select prop:value=move||draft.get().value["selection"]["page"]["id"].to_string() on:change=move|e|load_page_defaults(tasks,api,draft,event_target_value(&e).parse::<i64>().unwrap_or(0),message)>
        <option value="0">"Choose a resource"</option>{move||array(&resources.get(),"page").into_iter().map(|r|view!{<option value=r["id"].to_string()>{text(&r,"name")}</option>}).collect_view()}
    </select></Field>}
}
fn flag(draft: RwSignal<Draft>, key: &'static str, label: &'static str) -> impl IntoView {
    view! {<Field checkbox=true label=label><input type="checkbox" prop:checked=move||draft.get().value["policy"][key].as_bool().unwrap_or(false) on:change=move|e|{
        let checked=event_target_checked(&e);
        draft.update(|d|d.set_tracking(key=="capture_passwords",checked));
    }/></Field>}
}
fn schedule(
    draft: RwSignal<Draft>,
    key: &'static str,
    label: &'static str,
    zone: StoredValue<String>,
) -> impl IntoView {
    view! {<section class="resource-section"><h2>{label}</h2><p>"Leave blank for no explicit time. Daylight-saving changes are handled automatically; review shows the scheduled instant."</p>
        <div class="resource-columns"><Field label="Local date and time"><input type="datetime-local" prop:value=move||text(&draft.get().value[key],"local") on:input=move|e|{let local=event_target_value(&e);draft.update(|d|d.edit(|v|{if local.is_empty(){if let Some(v)=v.as_object_mut(){v.remove(key);}}else{let selected_zone=v[key]["time_zone"].as_str().unwrap_or(&zone.get_value()).to_owned();v[key]=json!({"local":local,"time_zone":selected_zone});}}));}/></Field>
        <Field label="Time zone"><input disabled=move||draft.get().value[key].is_null() placeholder=zone.get_value() prop:value=move||text(&draft.get().value[key],"time_zone") on:input=move|e|draft.update(|d|d.edit(|v|v[key]["time_zone"]=json!(event_target_value(&e))))/></Field></div>
    </section>}
}
fn create(api: Api, token: String, initial: Option<Value>) -> impl IntoView {
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let token = StoredValue::new(token);
    let navigate = use_navigate();
    let zone = StoredValue::new(local_zone());
    let generation = u64::from(initial.is_some());
    let draft = RwSignal::new(Draft {
        generation,
        value: initial.unwrap_or_else(||json!({"name":"","url":"","groups":[],"selection":{"groups":[],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}},"policy":{"capture_credentials":false,"capture_passwords":false,"redirect_url":""}})),
        ..Default::default()
    });
    let resources = RwSignal::new(json!({}));
    let busy = RwSignal::new(false);
    let loading = RwSignal::new(true);
    let step = RwSignal::new(0usize);
    let message = Message::new(String::new());
    let can_edit = editable();
    crate::navigation::protect_draft(move || draft.with(|d| d.generation > 0));
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
                    "/api/campaigns/prepare",
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
                Ok(_) => message.status("Test accepted by SMTP server.".into()),
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    };
    let launch = move |_| {
        if busy.get_untracked() {
            return;
        }
        let key = match random_key() {
            Ok(v) => v,
            Err(_) => {
                message.error("Cannot generate a secure launch key".into());
                return;
            }
        };
        let Some((key, body)) = draft.try_update(|d| d.launch(key)).flatten() else {
            return;
        };
        busy.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            match api.launch(&token.get_value(), &key, &body).await {
                Ok(v) => {
                    draft.update(|d| d.generation = 0);
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
    view! {<ManagementPage><a class="breadcrumb" href="/campaigns">"Back to campaigns"</a><PageHeader title="Create campaign" subtitle="Review the audience, content and schedule before launching."><span>"Unsaved draft"</span><ActionButton disabled=Signal::derive(move||busy.get()||loading.get()||draft.with(|d|d.launch.is_some())) on_click=move |_|reload.update(|v|*v+=1)>"Refresh resources"</ActionButton></PageHeader>
        {message.view()}<Show when=move||loading.get()><p role="status">"Loading resources…"</p></Show>
        <ol class="campaign-steps">{["Audience","Content and destination","Schedule","Review and launch"].into_iter().enumerate().map(|(index,label)|view!{<li aria-current=move||(step.get()==index).then_some("step")>{label}</li>}).collect_view()}</ol>
        <FormFields disabled=Signal::derive(move||busy.get()||loading.get()||!can_edit||draft.with(|d|d.launch.is_some()))>
        <section hidden=move||step.get()!=0><h2>"Audience"</h2>{input(draft,"name","Campaign name")}
        <p>"Choose recipient groups. Overlaps are counted once by exact email address; the first selected group supplies recipient details."</p>
        {move||array(&resources.get(),"groups").into_iter().map(|r|{let id=r["id"].as_i64().unwrap_or(0);view!{<Field checkbox=true label=format!("{} ({} recipients)",text(&r,"name"),r["num_targets"])><input type="checkbox" prop:checked=move||draft.with(|d|array(&d.value["selection"],"groups").iter().any(|r|r["id"]==id)) on:change=move|e|{let checked=event_target_checked(&e);draft.update(|d|d.edit(|v|{let mut groups=array(&v["selection"],"groups");groups.retain(|r|r["id"]!=id);if checked{groups.push(json!({"id":id}));}v["selection"]["groups"]=json!(groups);}));}/></Field>}}).collect_view()}
        <a href="/groups/new" target="_blank" rel="noopener">"Create a group in a new tab"</a></section>
        <section hidden=move||step.get()!=1><h2>"Content and destination"</h2><div class="resource-columns">{selector(draft,resources,"template","Email template")}{selector(draft,resources,"smtp","Sending profile")}{page_selector(tasks,api,draft,resources,message)}{domain_selector(draft,resources)}</div>
        <ul class="campaign-resource-links" aria-label="Create resources in a new tab"><li><a href="/templates/new" target="_blank" rel="noopener">"Create template"</a></li><li><a href="/pages/new" target="_blank" rel="noopener">"Create landing page"</a></li><li><a href="/smtp/new" target="_blank" rel="noopener">"Create sending profile"</a></li></ul><p class="resource-usage">"Changes to saved templates or sending profiles can affect scheduled emails and retries in existing campaigns."</p>
        <section class="resource-section"><h2>"Event tracking and redirect (optional)"</h2><p>"Selecting a landing page fills these fields with its defaults. Edit them for this campaign; choosing another page replaces them with that page's defaults."</p><p>"Track submission and password presence only. Entered usernames, passwords and other form values are never collected."</p>
        <Show when=move||draft.with(|d|d.page_request.is_some())><p role="status">"Loading landing-page defaults..."</p></Show><Show when=move||draft.with(|d|d.page_request.is_none()&&!d.policy_ready())><ActionButton kind="button" on_click=move |_|load_page_defaults(tasks,api,draft,draft.get_untracked().value["selection"]["page"]["id"].as_i64().unwrap_or(0),message)>"Retry loading defaults"</ActionButton></Show><FormFields disabled=Signal::derive(move||!draft.with(Draft::policy_ready))>{flag(draft,"capture_credentials","Record form submission")}{flag(draft,"capture_passwords","Record password presence")}
        <Field label="Redirect URL (optional; leave empty for no redirect)"><input prop:value=move||text(&draft.get().value["policy"],"redirect_url") on:input=move|e|draft.update(|d|d.edit(|v|v["policy"]["redirect_url"]=json!(event_target_value(&e))))/></Field></FormFields></section></section>
        <section hidden=move||step.get()!=2>{schedule(draft,"launch_local","Launch time",zone)}{schedule(draft,"send_by_local","Initial distribution end (optional)",zone)}<p>"With no launch time, delivery starts after launch. The distribution end spaces initial attempts; retries may continue afterward. SMTP acceptance does not guarantee inbox delivery."</p></section>
        <section hidden=move||step.get()!=3 aria-busy=move||draft.with(|d|d.review_request.is_some()).to_string()><h2>"Review"</h2><p>"Check the validated audience, content and schedule before launching."</p>
        <Show when=move||draft.with(|d|d.review_request.is_some())><p role="status">"Validating campaign and preparing preview..."</p></Show>
        <Show when=move||!loading.get()&&!draft.with(Draft::policy_ready)><p role="status">"Landing-page defaults must finish loading before review. If loading failed, return to Content and destination to retry."</p></Show>
        {move||draft.get().review_error.map(|error|view!{<div class="form-message error" role="alert"><p>{error}</p><p>"Go back to correct the campaign, or retry if the request failed."</p><ActionButton kind="button" on_click=move |_|retry_review.update(|n|*n+=1)>"Retry validation"</ActionButton></div>})}
        {move||draft.get().review.map(|r|view!{<div class="campaign-review"><h3>{format!("{} unique recipients",r["total"])}</h3><p>{format!("{} overlapping recipient entries removed",r["duplicate_count"])}</p>
        {(r["conflict_count"].as_u64().unwrap_or(0)>0).then(||view!{<details><summary>{format!("{} conflicting duplicate entries; first group wins",r["conflict_count"])}</summary><p>"Showing up to 100 conflicts. Review the retained and discarded recipient attributes."</p><ul>{array(&r,"conflicts").into_iter().map(|c|view!{<li>{format!("{}: retained {}; discarded {}",text(&c,"email"),c["kept"],c["discarded"])}</li>}).collect_view()}</ul></details>})}
        <dl class="resource-facts">{["template","page","smtp"].into_iter().map(|key|view!{<div><dt>{key}</dt><dd><a target="_blank" rel="noopener" href=format!("/{}/{}",match key{"template"=>"templates","page"=>"pages",_=>"smtp"},r["selection"][key]["id"])>{text(&r["selection"][key],"name")}</a></dd></div>}).collect_view()}
        <div><dt>"Launch"</dt><dd>{instant(&r["launch_date"])}</dd></div><div><dt>"Distribution end"</dt><dd>{instant(&r["send_by_date"])}</dd></div><div><dt>"Submission tracking"</dt><dd>{r["policy"]["capture_credentials"].to_string()}</dd></div><div><dt>"Password presence only"</dt><dd>{r["policy"]["capture_passwords"].to_string()}</dd></div><div><dt>"Redirect"</dt><dd>{text(&r["policy"],"redirect_url")}</dd></div></dl>
        <section class="resource-section"><h3>"Rendered sample"</h3><p>"Uses the first recipient's details. Test emails use Sample Recipient. Preview links are inactive and no interaction events are generated."</p><dl class="resource-facts"><div><dt>"Visible From"</dt><dd>{text(&r["preview"],"visible_sender")}</dd></div><div><dt>"Transport sender"</dt><dd>{text(&r["preview"],"transport_sender")}</dd></div><div><dt>"Subject"</dt><dd>{text(&r["preview"],"subject")}</dd></div></dl><iframe class="saved-content-preview" title="Rendered email sample" sandbox="" referrerpolicy="no-referrer" srcdoc=ruphish_ui::preview_document(&text(&r["preview"],"html"))></iframe><details><summary>"Plain text"</summary><pre>{text(&r["preview"],"text")}</pre></details><Field label="Test recipient email"><input type="email" prop:value=move||recipient.get() on:input=move|e|recipient.set(event_target_value(&e))/></Field><ActionButton disabled=Signal::derive(move||busy.get()||recipient.get().is_empty()) on_click=send_test>"Send test email"</ActionButton></section>
        {(!r["mailer_enabled"].as_bool().unwrap_or(false)).then(||view!{<p class="form-message error">"The mailer is disabled. This campaign will remain queued until an enabled admin process runs."</p>})}</div>})}</section>
        </FormFields>
        <FormFooter><a class="text-danger" href="/campaigns">"Cancel"</a><ActionButton disabled=Signal::derive(move||busy.get()||step.get()==0||draft.with(|d|d.launch.is_some())) on_click=move |_|step.update(|s|*s=s.saturating_sub(1))>"Back"</ActionButton>
        <ActionButton style=ButtonStyle::Primary hidden=Signal::derive(move||step.get()==3) disabled=Signal::derive(move||busy.get()||loading.get()||!can_edit) on_click=move |_|step.update(|s|*s=(*s+1).min(3))>"Continue"</ActionButton>
        <ActionButton style=ButtonStyle::Primary hidden=Signal::derive(move||step.get()!=3) disabled=Signal::derive(move||busy.get()||loading.get()||!can_edit||draft.with(|d|d.review.is_none()||d.review_request.is_some())) on_click=launch>{move||if draft.with(|d|d.launch.is_some()){ "Retry same launch" }else{"Launch campaign"}}</ActionButton></FormFooter>
    </ManagementPage>}
}

fn show(api: Api, token: String, id: String) -> impl IntoView {
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
                        updated.set(instant(&Value::from(js_sys::Date::now() / 1000.0)));
                        if failures.get_untracked() > 0 {
                            message.clear();
                        }
                        failures.set(0);
                    }
                    Err(e) => {
                        failures.update(|f| *f = (*f + 1).min(5));
                        message.error(format!("Refresh failed; displayed data may be stale. {e}"));
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
                        .ok_or_else(|| "Draft store unavailable".to_owned())
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
                                .map_err(|e| e.to_string())
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
                    message.status("Campaign updated.".into());
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
    view! {<ManagementPage><a class="breadcrumb" href="/campaigns">"Back to campaigns"</a><PageHeader title=Signal::derive(move||text(&data.get()["campaign"],"name")) subtitle=Signal::derive(move||format!("{} · Last refreshed {}",text(&data.get()["campaign"],"status"),updated.get()))>
        <ActionLink href=format!("/analysis/{}",id.get_value())>"View analysis"</ActionLink><ActionButton disabled=Signal::derive(move||busy.get()) busy=Signal::derive(move||loading.get().to_string()) on_click=move |_|refresh.update(|r|*r+=1)>{move||if loading.get(){"Refreshing..."}else{"Refresh"}}</ActionButton><ActionButton hidden=!can_edit disabled=Signal::derive(move||busy.get()) on_click=duplicate>"Duplicate"</ActionButton><ActionButton hidden=!can_edit disabled=Signal::derive(move||busy.get()||data.get()["campaign"]["status"]=="Completed") on_click=move |_|action.set(Some(("complete".into(),Value::Null)))>"Complete campaign"</ActionButton><ActionButton style=ButtonStyle::Cancel hidden=!can_edit disabled=Signal::derive(move||busy.get()) on_click=move |_|action.set(Some(("delete".into(),Value::Null)))>"Delete"</ActionButton>
    </PageHeader>{message.view()}<Show when=move||data.get()["mailer_enabled"]==false><p class="form-message error">"Delivery processing is disabled in this process."</p></Show>
    <p>"Acceptance means the SMTP server accepted the message. Participant credentials are never collected."</p>
    {move||campaign_context(&data.get()["campaign"])}
    {delivery_progress(data)}
    <DataTable label="Campaign delivery" headers=vec!["History", "Recipient", "Delivery", "Next/last due", "Attempts and actions"]>
        <For each=move||{array(&data.get(),"items").into_iter().filter(|_|pages.with(CursorPages::loaded)).filter_map(|r|r["id"].as_i64()).collect::<Vec<_>>()} key=|id|*id children=move|result|{crate::campaign_delivery::recipient_row(api,id,result,data,crate::campaign_delivery::Controls {busy,can_edit,action})}/>
    </DataTable><PageControls pages=pages disabled=Signal::derive(move||loading.get()||busy.get())/>
    <section class="resource-section"><h2>"Recent activity"</h2><p>"Latest 50 campaign events. Expand a recipient above for their timeline and earlier history."</p><DataTable label="Campaign activity" headers=vec!["Time","Recipient","Event"]>{move||array(&data.get(),"activity").into_iter().map(|e|view!{<tr><td>{text(&e,"time")}</td><td>{text(&e,"email")}</td><td>{text(&e,"message")}</td></tr>}).collect_view()}</DataTable></section>
    {move||action.get().map(|(kind,_)|{let (title,description,label)=match kind.as_str(){"delete"=>("Delete campaign?","Campaign history will be removed. Already submitted mail cannot be recalled.","Delete campaign"),"complete"=>("Complete campaign?","Pending delivery will stop. Already armed or in-flight submissions may still finish.","Complete campaign"),"resend_unknown"=>("Resend despite an unknown outcome?","Check your mail server first. The earlier attempt may have been accepted; resending can deliver a duplicate.","Resend"),"confirm_accepted"=>("Confirm SMTP acceptance?","Use the attempt Message-ID to verify acceptance in your mail server before recording this outcome.","Confirm accepted"),_=>("Retry this delivery?","Correct the reported content, account or sending-profile problem before retrying.","Retry")};let counts=data.get_untracked();let description=if matches!(kind.as_str(),"complete"|"delete"){format!("{description} Pending: {}. Preparing: {}. Submitting: {}.",["ready","retry","blocked"].iter().map(|k|counts["counts"][*k].as_i64().unwrap_or(0)).sum::<i64>(),counts["counts"]["preparing"].as_i64().unwrap_or(0),counts["counts"]["submitting"].as_i64().unwrap_or(0))}else{description.to_owned()};view!{<DeleteConfirmation title=title.into() description=description confirm_label=label busy=Signal::derive(move||busy.get()) on_confirm=confirm on_cancel=Callback::new(move|()|action.set(None)) message=message/>}})}
    </ManagementPage>}
}

pub(super) fn instant(value: &Value) -> String {
    campaign_time(&value.to_string())
}

fn delivery_progress(data: RwSignal<Value>) -> impl IntoView {
    let states = [
        (
            "ready",
            "Ready",
            "neutral",
            "Queued, waiting for its scheduled time or an available worker.",
        ),
        (
            "preparing",
            "Preparing",
            "active",
            "A worker claimed this recipient and is preparing the sending attempt. After a crash, this state remains until recovery runs.",
        ),
        (
            "submitting",
            "Submitting",
            "active",
            "Submission has been authorized, but SMTP acceptance is not yet recorded. After a crash, recovery marks an unresolved attempt as unknown.",
        ),
        (
            "retry",
            "Retry",
            "warning",
            "A retryable failure was recorded. Another attempt is scheduled with a delay.",
        ),
        (
            "accepted",
            "Accepted",
            "success",
            "SMTP acceptance was recorded, or explicitly confirmed by an operator using mail-server evidence. This does not confirm inbox delivery.",
        ),
        (
            "failed",
            "Failed",
            "failure",
            "Content preparation failed, the message was permanently rejected, or the retry limit was reached. Check the attempt history before retrying.",
        ),
        (
            "blocked",
            "Blocked",
            "warning",
            "An account, resource or SMTP configuration problem prevents automatic progress. Correct the problem before retrying.",
        ),
        (
            "unknown",
            "Unknown",
            "warning",
            "The message may have been accepted, but its outcome could not be confirmed. Check the mail server before resending to avoid duplicates.",
        ),
        (
            "cancelled",
            "Cancelled",
            "neutral",
            "Pending work was cancelled. Messages already submitted cannot be recalled.",
        ),
    ];
    view! {<section class="resource-read-panel delivery-progress" aria-label="Delivery progress">
        <h2>"Delivery progress"</h2>
        <dl class="delivery-metrics">{states.into_iter().map(|(state,label,tone,explanation)|view!{
            <div class=format!("delivery-metric {tone}") class:has-count={move||data.get()["counts"][state].as_i64().unwrap_or(0)>0}>
                <dt><span>{label}</span><InfoPopover id=format!("delivery-help-{state}") label=label explanation=explanation/></dt>
                <dd>{move||data.get()["counts"][state].as_i64().unwrap_or(0)}</dd>
            </div>
        }).collect_view()}</dl>
    </section>}
}

fn campaign_context(c: &Value) -> impl IntoView + use<> {
    let resource = |key: &str, label: &'static str, route: &str| {
        let id = c[key]["id"].as_i64().unwrap_or(0);
        let name = c[key]["name"].as_str().map(str::to_owned);
        view! {<div><dt>{label}</dt><dd>{match name {
            Some(name)=>view!{<a href=format!("/{route}/{id}")>{name}</a>}.into_any(),
            None=>view!{<span>{if id==0 {"Not selected".into()} else {format!("Unavailable (ID {id})")}}</span>}.into_any()
        }}</dd></div>}
    };
    let template_sender = text(c, "template_sender");
    view! {<section class="resource-read-panel campaign-context" aria-label="Campaign details"><h2>"Campaign details"</h2>
        {(c["domain_block"].is_string() && c["status"]!="Completed").then(||view!{<p role="status" class="form-message">"Delivery is waiting for domain verification. Queued recipients are retained; sending resumes after the domain passes its checks." <a href=c["domain_id"].as_i64().map(|id|format!("/domains/{id}")).unwrap_or_else(||"/domains".into())>"Review domain"</a></p>})}
        <dl class="resource-facts">
            {resource("template","Email template","templates")}{resource("page","Landing page","pages")}{resource("smtp","Sending profile","smtp")}
            <div><dt>"Email subject template"</dt><dd>{c["subject"].as_str().map(|s|if s.is_empty(){"No subject"}else{s}).unwrap_or("Unavailable").to_owned()}</dd></div>
            <div><dt>"Sending-profile From address"</dt><dd>{c["profile_sender"].as_str().unwrap_or("Unavailable").to_owned()}</dd></div>
            <div><dt>"Template From override"</dt><dd>{if template_sender.is_empty(){"Use sending-profile address".into()}else{template_sender}}</dd></div>
            <div><dt>"Scheduled launch"</dt><dd>{instant(&c["launch_date"])}</dd></div>
            <div><dt>"Initial distribution end"</dt><dd>{if text(c,"send_by_date").starts_with("0001-"){"No distribution window".into()}else{instant(&c["send_by_date"])}}</dd></div>
            <div><dt>"Public campaign URL"</dt><dd>{text(c,"url")}</dd></div>
        </dl>
        <p class="resource-usage">{format!("Times shown in {}. Template and sending-profile details reflect the current saved resources; recipient delivery history records actual attempts.",local_zone())}</p>
    </section>}
}

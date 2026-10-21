//! Durable campaign drafts use the shared management surface and request lifecycle.
use crate::{
    account::Api,
    campaigns::{array, text},
    feedback::Message,
    shared::components::*,
};
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};
use ruphish_ui::{tr, trs, view_tasks::ViewTasks};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) struct Persistence {
    pub id: RwSignal<String>,
    pub revision: RwSignal<Option<i64>>,
    pub saved_generation: RwSignal<u64>,
    pub modified: RwSignal<Value>,
    pub saving: RwSignal<bool>,
}
impl Persistence {
    pub fn new(saved: Option<&Value>) -> Self {
        Self {
            saving: RwSignal::new(false),
            id: RwSignal::new(saved.map(|s| text(s, "id")).unwrap_or_default()),
            revision: RwSignal::new(saved.and_then(|s| s["revision"].as_i64())),
            saved_generation: RwSignal::new(0),
            modified: RwSignal::new(saved.map(|s| s["modified"].clone()).unwrap_or(Value::Null)),
        }
    }
    pub async fn save(
        self,
        api: Api,
        token: &str,
        state: &ruphish_ui::campaign_state::Draft,
        step: usize,
    ) -> Result<Value, String> {
        if self.id.get_untracked().is_empty() {
            let key = crate::campaigns::random_key()
                .map_err(|_| ruphish_ui::tr!(ui.cannot_generate_a_secure_launch_key)())?;
            self.id.set(key);
        }
        let body = json!({"revision":self.revision.get_untracked(),"configuration":state.value,"step":step});
        self.saving.set(true);
        let result = api
            .resource(
                "PUT",
                &format!("/api/campaign-drafts/{}", self.id.get_untracked()),
                token,
                Some(&body),
                None,
            )
            .await;
        self.saving.set(false);
        let (saved, _) = result?;
        self.revision.set(saved["revision"].as_i64());
        self.saved_generation.set(state.generation);
        self.modified.set(saved["modified"].clone());
        Ok(saved)
    }
}

pub(super) fn page(api: Api, token: String, editing: bool) -> impl IntoView {
    let params = use_params_map();
    let token = StoredValue::new(token);
    view! {<For each=move||vec![params.with(|p|p.get("id").unwrap_or_default())] key=|id|id.clone() children=move|id|load(api,token.get_value(),id,editing)/>}
}
fn load(api: Api, token: String, id: String, editing: bool) -> impl IntoView {
    let id = StoredValue::new(id);
    let token = StoredValue::new(token);
    let tasks = ViewTasks::default();
    let data = RwSignal::new(None::<Value>);
    let message = Message::new(String::new());
    let loading = RwSignal::new(false);
    let refresh = RwSignal::new(0u64);
    let navigate = use_navigate();
    Effect::new(move |_| {
        refresh.get();
        loading.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            match api
                .get(&format!("/api/campaign-drafts/{}", id.get_value()))
                .await
            {
                Ok(v) => {
                    if let Some(campaign) = v["campaign_id"].as_i64() {
                        navigate(&format!("/campaigns/{campaign}"), Default::default());
                    } else {
                        data.set(Some(v));
                        message.clear();
                    }
                }
                Err(e) => message.error(e),
            }
            loading.set(false);
        });
    });
    // Only a newly loaded record should replace the form. Initial preference reads
    // must not subscribe this factory and discard unsaved edits on preference changes.
    view! { {move||data.get().map(|saved|untrack(||if editing {
        crate::campaigns::create(api,token.get_value(),Some(saved["configuration"].clone()),Some(saved)).into_any()
    }else{detail(api,token.get_value(),saved).into_any()}))}
    <Show when=move||data.get().is_none()><ManagementPage><BackLink href="/campaigns">{tr!(ui.back_to_campaigns)}</BackLink>{message.view()}<ActionButton disabled=Signal::derive(move||loading.get()) on_click=move |_|refresh.update(|n|*n+=1)>{tr!(ui.refresh)}</ActionButton></ManagementPage></Show> }
}
fn detail(api: Api, token: String, saved: Value) -> impl IntoView {
    let id = StoredValue::new(text(&saved, "id"));
    let revision = saved["revision"].as_i64().unwrap_or(0);
    let config = StoredValue::new(saved["configuration"].clone());
    let token = StoredValue::new(token);
    let names = RwSignal::new(json!({}));
    let tasks = ViewTasks::default();
    let busy = RwSignal::new(false);
    let deleting = RwSignal::new(false);
    let message = Message::new(String::new());
    let navigate = use_navigate();
    let can_edit = crate::campaigns::editable();
    tasks.spawn(async move {
        for (family, key) in [
            ("groups", "groups"),
            ("templates", "template"),
            ("pages", "page"),
            ("smtp", "smtp"),
        ] {
            match api.get(&format!("/api/{family}/summary")).await {
                Ok(v) => names.update(|n| {
                    n[key] = json!(array(
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
    });
    let name = text(&config.get_value(), "name");
    let i18n = ruphish_ui::localization::use_i18n();
    let title = Signal::derive(move || {
        if name.is_empty() {
            leptos_i18n::t_string!(i18n, ui.untitled_campaign).to_owned()
        } else {
            name.clone()
        }
    });
    let delete = Callback::new(move |()| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            match api
                .resource(
                    "DELETE",
                    &format!(
                        "/api/campaign-drafts/{}?revision={revision}",
                        id.get_value()
                    ),
                    &token.get_value(),
                    None,
                    None,
                )
                .await
            {
                Ok(_) => navigate("/campaigns", Default::default()),
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    });
    view! {<ManagementPage><BackLink href="/campaigns">{tr!(ui.back_to_campaigns)}</BackLink>
        <PageHeader title=title subtitle=trs!(ui.draft_description)><StatusBadge tone=StatusTone::Warning>{tr!(ui.draft)}</StatusBadge>
        <ResourceActions edit_href=format!("/campaigns/drafts/{}/edit",id.get_value()) can_edit=can_edit on_delete=Callback::new(move|()|deleting.set(true))/></PageHeader>
        {message.view()}<Card><dl class="campaign-review-fields">
        <div><dt>{tr!(ui.last_saved)}</dt><dd>{move||crate::time_display::instant(&saved["modified"])}</dd></div>

        {[ ("template",trs!(ui.email_template)),("page",trs!(ui.landing_page)),("smtp",trs!(ui.sending_profile)) ].into_iter().map(move|(key,label)|view!{
            <div><dt>{move||label.get()}</dt><dd>{move||{let selected=config.get_value()["selection"][key]["id"].clone();array(&names.get(),key).iter().find(|v|v["id"]==selected).map(|v|text(v,"name")).unwrap_or_else(||ruphish_ui::tr!(ui.choose_a_resource)())}}</dd></div>
        }).collect_view()}
        <div><dt>{tr!(ui.recipients)}</dt><dd>{move||array(&config.get_value()["selection"],"groups").iter().map(|selected|array(&names.get(),"groups").iter().find(|v|v["id"]==selected["id"]).map(|v|text(v,"name")).unwrap_or_else(||ruphish_ui::tr!(ui.missing_draft_resource)())).collect::<Vec<_>>().join(", ")}</dd></div>
        <div><dt>{tr!(ui.public_url)}</dt><dd>{text(&config.get_value(),"url")}</dd></div>
        </dl></Card>
        <Card><h2>{tr!(ui.schedule)}</h2><p>{tr!(ui.not_scheduled)}</p><dl class="campaign-review-fields">
        {[("launch_local",trs!(ui.launch_time)),("send_by_local",trs!(ui.initial_distribution_end_optional))].into_iter().map(move|(key,label)|view!{<div><dt>{move||label.get()}</dt><dd>{move||{let value=config.get_value();let time=&value[key];if time.is_null(){ruphish_ui::tr!(ui.not_set)()}else{match ruphish_domain::datetime::resolve_local(&text(time,"local"),&text(time,"time_zone")){Ok(time)=>crate::time_display::instant(&json!(ruphish_domain::datetime::utc(time))),Err(_)=>ruphish_ui::tr!(ui.time_not_recorded)()}}}}</dd></div>}).collect_view()}
        </dl></Card>
        <Show when=move||deleting.get()><DeleteConfirmation title=trs!(ui.delete_draft) description=trs!(ui.delete_draft_description) confirm_label=trs!(ui.delete) busy=Signal::derive(move||busy.get()) message=message on_confirm=delete on_cancel=Callback::new(move|()|deleting.set(false))/></Show>
    </ManagementPage>}
}

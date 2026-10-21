//! Resource lists and full-page editors using the preserved HTTP boundary.
use crate::account::Api;
use crate::feedback::Message;
use crate::shared::components::*;
use leptos::{html, prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_ui::{
    import_drafts::{ImportDrafts, ImportedDraft},
    import_state::{ImportError, ImportReview},
    preview_document,
    resource_state::ResourceDraft,
};
use ruphish_ui::{tr, trs};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) struct ResourceAccess(pub bool);

#[derive(Clone, Copy)]
pub(super) enum Screen {
    List,
    View,
    Edit,
}

#[path = "resource_view.rs"]
mod resource_view;

/// Keep page lifetime tied to its ID and duplicate source; the shell stays mounted.
pub(super) fn detail(api: Api, token: String, family: Family, editing: bool) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    use leptos_router::{
        NavigateOptions,
        hooks::{use_navigate, use_params_map, use_query_map},
    };
    let params = use_params_map();
    let query = use_query_map();
    let navigate = use_navigate();
    let drafts = use_context::<ImportDrafts>();
    let replace_url = Callback::new(move |url: String| {
        navigate(
            &url,
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        )
    });
    view! {<For each=move||vec![(params.with(|p|p.get("id")).unwrap_or_default(), query.with(|q|q.get("duplicate")), query.with(|q|q.get("draft")))] key=|key|key.clone() children=move|(id,duplicate,draft_id)|{
        if id == "new" && let Some(draft_id) = draft_id {
            if let Some(imported) = drafts.and_then(|store| store.get(family.route(), &draft_id)) {
                on_cleanup(move || { if let Some(store) = drafts { store.remove(&draft_id); } });
                edit(api,token.clone(),family,id,None,replace_url,Some(imported)).into_any()
            } else {
                view!{<ManagementPage><h1>{tr!(ui.this_draft_is_no_longer_available)}</h1><p class="form-message error" role="alert">{ruphish_ui::localization::import_error(i18n, &ImportError::DraftUnavailable)}</p><p>{tr!(ui.imported_drafts_stay_in_this_tab_until_saved_or_discarded_refreshing_closing_the)}</p><ActionLink style=ButtonStyle::Primary href=format!("/{}?import=1",family.route())>{tr!(ui.import_again)}</ActionLink></ManagementPage>}.into_any()
            }
        } else if editing || id == "new" { edit(api,token.clone(),family,id,duplicate,replace_url,None).into_any() }
        else { resource_view::show(api,token.clone(),family,id).into_any() }
    }/>}
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Family {
    Groups,
    Templates,
    Pages,
    Profiles,
}
impl Family {
    fn new_label(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();
        match self {
            Self::Groups => leptos_i18n::t_string!(i18n, ui.new_group),
            Self::Templates => leptos_i18n::t_string!(i18n, ui.new_template),
            Self::Pages => leptos_i18n::t_string!(i18n, ui.new_page),
            Self::Profiles => leptos_i18n::t_string!(i18n, ui.new_profile),
        }
    }
    fn back_label(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();
        match self {
            Self::Groups => leptos_i18n::t_string!(i18n, ui.back_to_groups),
            Self::Templates => leptos_i18n::t_string!(i18n, ui.back_to_templates),
            Self::Pages => leptos_i18n::t_string!(i18n, ui.back_to_pages),
            Self::Profiles => leptos_i18n::t_string!(i18n, ui.back_to_profiles),
        }
    }
    fn route(self) -> &'static str {
        match self {
            Self::Groups => "groups",
            Self::Templates => "templates",
            Self::Pages => "pages",
            Self::Profiles => "smtp",
        }
    }
    fn title(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Groups => leptos_i18n::t_string!(i18n, ui.recipient_groups),
            Self::Templates => leptos_i18n::t_string!(i18n, ui.email_templates),
            Self::Pages => leptos_i18n::t_string!(i18n, ui.landing_pages),
            Self::Profiles => leptos_i18n::t_string!(i18n, ui.sending_profiles),
        }
    }
    fn singular(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Groups => leptos_i18n::t_string!(i18n, ui.group),
            Self::Templates => leptos_i18n::t_string!(i18n, ui.email_template_2),
            Self::Pages => leptos_i18n::t_string!(i18n, ui.landing_page_2),
            Self::Profiles => leptos_i18n::t_string!(i18n, ui.sending_profile_2),
        }
    }
    fn plural(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Groups => leptos_i18n::t_string!(i18n, ui.recipient_groups_2),
            Self::Templates => leptos_i18n::t_string!(i18n, ui.email_templates_2),
            Self::Pages => leptos_i18n::t_string!(i18n, ui.landing_pages_2),
            Self::Profiles => leptos_i18n::t_string!(i18n, ui.sending_profiles_2),
        }
    }
    fn empty_title(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Groups => leptos_i18n::t_string!(i18n, ui.no_recipient_groups_yet),
            Self::Templates => leptos_i18n::t_string!(i18n, ui.no_email_templates_yet),
            Self::Pages => leptos_i18n::t_string!(i18n, ui.no_landing_pages_yet),
            Self::Profiles => leptos_i18n::t_string!(i18n, ui.no_sending_profiles_yet),
        }
    }
    fn empty_guidance(self) -> &'static str {
        let i18n = ruphish_ui::localization::use_i18n();

        match self {
            Self::Groups => leptos_i18n::t_string!(
                i18n,
                ui.create_a_group_or_import_recipients_to_use_as_a_campaign_audience
            ),
            Self::Templates => leptos_i18n::t_string!(
                i18n,
                ui.create_or_import_an_email_template_to_send_in_a_campaign
            ),
            Self::Pages => leptos_i18n::t_string!(
                i18n,
                ui.create_or_import_a_landing_page_for_recipients_who_follow_a_link
            ),
            Self::Profiles => leptos_i18n::t_string!(
                i18n,
                ui.add_a_sending_profile_and_verify_it_before_launching_a_campaign
            ),
        }
    }
    fn empty(self) -> Value {
        match self {
            Self::Groups => json!({"id":0,"name":"","targets":[]}),
            Self::Templates => {
                json!({"id":0,"name":"","subject":"","envelope_sender":"","html":"","text":"","attachments":[]})
            }
            Self::Pages => {
                json!({"id":0,"name":"","html":"","capture_credentials":false,"capture_passwords":false,"redirect_url":""})
            }
            Self::Profiles => {
                json!({"id":0,"name":"","host":"","from_address":"","username":"","password":"","ignore_cert_errors":false,"interface_type":"SMTP","headers":[]})
            }
        }
    }
}
fn download(value: &Value, name: &str, message: Message) {
    let i18n = ruphish_ui::localization::use_i18n();

    match serde_json::to_vec_pretty(value) {
        Ok(bytes) => {
            if let Err(e) = crate::editor::download_template(&bytes, name) {
                message.error(leptos_i18n::t_string!(
                    i18n,
                    messages.download_failed_arg0,
                    arg0 = format!("{:?}", e)
                ));
            }
        }
        Err(e) => message.error(e.to_string()),
    }
}
pub(super) fn list(api: Api, token: String, family: Family) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let token = StoredValue::new(token);
    let query = leptos_router::hooks::use_query_map();
    let import_open = RwSignal::new(
        can_edit && query.with_untracked(|q| q.get("import").as_deref() == Some("1")),
    );
    let rows = RwSignal::new(Vec::<Value>::new());
    let message = Message::new(String::new());
    let loading = RwSignal::new(true);
    let failed = RwSignal::new(false);
    let search = RwSignal::new(String::new());
    let refresh = RwSignal::new(0u64);
    let deletion = ResourceDeletion::new();
    let generation = StoredValue::new(0u64);
    Effect::new(move |_| {
        refresh.get();
        generation.update_value(|n| *n += 1);
        let ticket = generation.get_value();
        loading.set(true);
        failed.set(false);
        spawn_local(async move {
            let result = api.get(&format!("/api/{}/summary", family.route())).await;
            if generation.get_value() != ticket {
                return;
            }
            match result {
                Ok(v) => rows.set(
                    v[if family == Family::Groups {
                        "groups"
                    } else {
                        "items"
                    }]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
                ),
                Err(e) => {
                    failed.set(true);
                    message.error(e);
                }
            }
            loading.set(false);
        });
    });
    let deleted = Callback::new(move |id| {
        generation.update_value(|n| *n += 1);
        loading.set(false);
        failed.set(false);
        rows.update(|rows| rows.retain(|row| row["id"].as_i64() != Some(id)));
        message.status(leptos_i18n::t_string!(i18n, ui.resource_deleted).to_owned());
    });
    let visible = Signal::derive(move || {
        let needle = search.get().to_lowercase();
        rows.get()
            .into_iter()
            .filter(|v| {
                v["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
            })
            .collect::<Vec<_>>()
    });
    view! {<ManagementPage>
    <PageHeader title=Signal::derive(move || family.title().to_owned()) subtitle=match family{Family::Groups=>leptos_i18n::t_string!(i18n, ui.organize_recipients_and_review_imported_data_before_saving),Family::Templates=>leptos_i18n::t_string!(i18n, ui.compose_messages_manage_attachments_and_send_a_test),Family::Pages=>leptos_i18n::t_string!(i18n, ui.prepare_landing_page_content_and_optional_interaction_defaults),Family::Profiles=>leptos_i18n::t_string!(i18n, ui.configure_mail_connections_and_verify_them_before_use)}.to_owned()>
        <ActionButton hidden=!can_edit on_click=move |_|import_open.set(true)>{tr!(ui.import)}</ActionButton><ActionLink hidden=!can_edit style=ButtonStyle::Primary href=format!("/{}/new",family.route())>{leptos_i18n::t!(i18n, messages.create_arg0, arg0 = (family.singular()).to_string())}</ActionLink>
    </PageHeader>
    {move||(can_edit && import_open.get()).then(||import_panel(api,token.get_value(),family,import_open))}
    {message.view()}
    <ResourceDeleteDialog deletion=deletion api=api token=token.get_value() on_deleted=deleted/>
    <ListToolbar search=search count=Signal::derive(move || visible.with(Vec::len)) noun=Signal::derive(move || family.plural().to_owned())/>
    <Show when=move||loading.get()><p role="status">{leptos_i18n::t!(i18n, messages.loading_arg0, arg0 = (family.plural()).to_string())}</p></Show>
    <Show when=move||!loading.get()&&failed.get()><ActionButton on_click=move |_|refresh.update(|v|*v+=1)>{leptos_i18n::t!(i18n, messages.retry_loading_arg0, arg0 = (family.plural()).to_string())}</ActionButton></Show>
    <DataTable label=Signal::derive(move || family.title().to_owned()) empty=Signal::derive(move || visible.with(Vec::is_empty)) headers=vec![trs!(ui.name),if family==Family::Groups{trs!(ui.recipients)}else{trs!(ui.active_campaigns)},trs!(ui.last_modified),trs!(ui.actions)]>
    {move||visible.get().into_iter().map(|v|{let id=v["id"].as_i64().unwrap_or(0);let name=v["name"].as_str().unwrap_or(leptos_i18n::t_string!(i18n, ui.unnamed)).to_owned();let delete_value=v.clone();view!{<tr><th scope="row"><a href=format!("/{}/{id}",family.route())>{name}</a></th><td>{v[if family==Family::Groups{"num_targets"}else{"active_campaigns"}].as_i64().unwrap_or(0)}</td><td>{crate::time_display::instant(&v["modified_date"])}</td><td><ResourceActions view_href=format!("/{}/{id}",family.route()) edit_href=format!("/{}/{id}/edit",family.route()) can_edit=can_edit size=ControlSize::Small disabled=deletion.busy on_delete=Callback::new(move |()| deletion.select(DeleteTarget { id, name:delete_value["name"].as_str().unwrap_or("resource").to_owned(), path:format!("/api/{}/{id}",family.route()), revision:None, description:leptos_i18n::t_string!(i18n, ui.this_permanently_removes_the_saved_resource_resources_used_by_unfinished_campaig) }))><ActionLink hidden=!can_edit size=ControlSize::Small disabled=deletion.busy href=format!("/{}/new?duplicate={id}",family.route())>{tr!(ui.duplicate)}</ActionLink></ResourceActions></td></tr>}}).collect_view()}
    </DataTable>
    <ListEmptyState ready=Signal::derive(move || !loading.get() && !failed.get()) count=Signal::derive(move || visible.with(Vec::len)) search=search title=Signal::derive(move || family.empty_title().to_owned()) guidance=Signal::derive(move || family.empty_guidance().to_owned())/>
    </ManagementPage>}
}

#[derive(Clone, Copy)]
struct Editor {
    draft: RwSignal<ResourceDraft>,
    message: Message,
    epoch: RwSignal<u64>,
    busy: RwSignal<bool>,
    visual_pending: RwSignal<bool>,
    readonly: bool,
    replace_url: Callback<String>,
    import_review: Option<RwSignal<ImportReview>>,
}
impl Editor {
    fn text(self, key: &str) -> String {
        self.draft
            .with(|d| d.value()[key].as_str().unwrap_or("").to_owned())
    }
    fn set(self, key: &str, value: Value) {
        self.draft.update(|d| d.edit(|v| v[key] = value));
    }
    fn field(
        self,
        key: &'static str,
        label: Signal<String>,
        input_type: &'static str,
    ) -> impl IntoView {
        view! {<Field label=label><TextInput id=key name=key input_type=input_type autocomplete="off" value=Signal::derive(move||self.text(key)) on_input=move|e|self.set(key,json!(event_target_value(&e)))/></Field>}
    }
    fn flag(self, key: &'static str, label: Signal<String>) -> impl IntoView {
        view! {<Checkbox label=label checked=Signal::derive(move||self.draft.with(|d|d.value()[key].as_bool().unwrap_or(false))) on_toggle=Callback::new(move|v|self.set(key,json!(v)))/>}
    }
}
fn edit(
    api: Api,
    token: String,
    family: Family,
    id: String,
    duplicate: Option<String>,
    replace_url: Callback<String>,
    imported: Option<ImportedDraft>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(token);
    let requested = StoredValue::new(id);
    let loading = RwSignal::new(true);
    let load_failed = RwSignal::new(false);
    let is_imported = imported.is_some();
    let import_review = imported.as_ref().map(|i| RwSignal::from(i.review.clone()));
    let draft = imported
        .map(|i| RwSignal::from(i.draft))
        .unwrap_or_else(|| RwSignal::new(ResourceDraft::new(family.empty(), None)));
    let editor = Editor {
        draft,
        import_review,
        message: Message::new(String::new()),
        epoch: RwSignal::new(0),
        busy: RwSignal::new(false),
        visual_pending: RwSignal::new(false),
        readonly: !use_context::<ResourceAccess>().is_some_and(|access| access.0),
        replace_url,
    };
    let test_open = RwSignal::new(false);
    let reload = RwSignal::new(0u64);
    Effect::new(move |_| {
        reload.get();
        loading.set(true);
        load_failed.set(false);
        editor.message.clear();
        let duplicate = duplicate.clone();
        spawn_local(async move {
            if is_imported {
                loading.set(false);
                return;
            }
            let id = requested.get_value();
            let target = if id == "new" {
                duplicate.clone()
            } else {
                Some(id)
            };
            if let Some(target) = target {
                match api
                    .resource(
                        "GET",
                        &format!("/api/{}/{target}", family.route()),
                        &token.get_value(),
                        None,
                        None,
                    )
                    .await
                {
                    Ok((mut value, revision)) => {
                        if duplicate.is_some() {
                            value["id"] = json!(0);
                            value["name"] = json!(leptos_i18n::t_string!(
                                i18n,
                                messages.copy_name,
                                name = value["name"]
                                    .as_str()
                                    .unwrap_or(leptos_i18n::t_string!(i18n, ui.resource))
                            ));
                            value.as_object_mut().map(|v| v.remove("modified_date"));
                        }
                        let loaded = if duplicate.is_some() {
                            let mut draft = ResourceDraft::new(family.empty(), None);
                            draft.edit(|draft| *draft = value);
                            draft
                        } else {
                            ResourceDraft::new(value, revision)
                        };
                        editor.draft.set(loaded);
                        editor.epoch.update(|v| *v += 1);
                    }
                    Err(e) => {
                        editor.message.error(e);
                        load_failed.set(true);
                    }
                }
            }
            loading.set(false);
        });
    });
    crate::navigation::protect_draft(move || {
        editor.draft.with(ResourceDraft::dirty) || editor.visual_pending.get() || editor.busy.get()
    });
    let save = move |_| {
        spawn_local(async move {
            if save_draft(api, token.get_value(), family, editor).await {
                finish_edit(editor, family);
            }
        });
    };
    let content = match family {
        Family::Groups => group_fields(editor).into_any(),
        Family::Templates | Family::Pages => content_fields(api, token, editor, family).into_any(),
        Family::Profiles => profile_fields(editor).into_any(),
    };
    view! {<ManagementPage><BackLink href=format!("/{}",family.route())>{move || family.back_label()}</BackLink>
    <PageHeader title=Signal::derive(move||{let name=editor.text("name");if name.is_empty(){family.new_label().to_owned()}else{name}})
        status=true subtitle=Signal::derive(move||if editor.busy.get(){leptos_i18n::t_string!(i18n, ui.saving_or_checking)}else if loading.get(){leptos_i18n::t_string!(i18n, ui.loading_resource)}else if load_failed.get(){leptos_i18n::t_string!(i18n, ui.resource_could_not_be_loaded)}else if editor.readonly{leptos_i18n::t_string!(i18n, ui.read_only_access)}else if editor.visual_pending.get(){leptos_i18n::t_string!(i18n, ui.visual_editor_not_ready)}else if editor.draft.with(ResourceDraft::dirty){leptos_i18n::t_string!(i18n, ui.unsaved_changes)}else if editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){leptos_i18n::t_string!(i18n, ui.not_saved_yet)}else{leptos_i18n::t_string!(i18n, ui.no_unsaved_changes)}.to_owned())>
    {(family==Family::Templates||family==Family::Profiles).then(||view!{<ActionButton disabled=Signal::derive(move||loading.get()||load_failed.get()||editor.readonly||editor.busy.get()||editor.visual_pending.get()) on_click=move |_|test_open.set(true)>{tr!(ui.send_test)}</ActionButton>})}

    </PageHeader>
    {matches!(family,Family::Templates|Family::Pages|Family::Profiles).then(||view!{<p class="resource-usage">{tr!(ui.campaigns_use_the_current_saved_version_of_this_resource_editing_it_can_affect_u)}</p>})}
    {editor.import_review.map(|review|import_review_panel(review,editor.message))}
    {editor.message.view()}<Show when=move||!loading.get()&&load_failed.get()><ActionButton on_click=move |_|reload.update(|n|*n+=1)>{tr!(ui.retry_loading_resource)}</ActionButton></Show><Show when=move||loading.get()><p>{tr!(ui.loading_resource)}</p></Show>
    <FormFields disabled=Signal::derive(move||load_failed.get()||editor.readonly||loading.get()||editor.busy.get())>{editor.field("name", trs!(ui.name), "text")}{content}</FormFields>
    <FormFooter spread=true><ActionButton style=ButtonStyle::Ghost disabled=Signal::derive(move||loading.get()||load_failed.get()||editor.busy.get()) on_click=move |_|{editor.draft.update(ResourceDraft::cancel);editor.epoch.update(|v|*v+=1);let id=editor.draft.with_untracked(|d|d.value()["id"].as_i64().unwrap_or(0)); editor.replace_url.run(if id>0{format!("/{}/{id}",family.route())}else{format!("/{}",family.route())});}>{tr!(ui.cancel_changes)}</ActionButton><SaveButton disabled=Signal::derive(move||loading.get()||load_failed.get()||editor.readonly||editor.busy.get()||editor.visual_pending.get()||editor.import_review.is_some_and(|r|!r.with(ImportReview::ready))) on_click=Callback::new(save) label=Signal::derive(move||if editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){leptos_i18n::t_string!(i18n, messages.create_arg0, arg0 = (family.singular()).to_string())}else{leptos_i18n::t_string!(i18n, ui.save_changes).into()})/></FormFooter>
    {move||test_open.get().then(||test_panel(api,token.get_value(),family,editor,test_open))}
    </ManagementPage>}
}
fn finish_edit(editor: Editor, family: Family) {
    if let Some(id) = editor.draft.with_untracked(ResourceDraft::completed_id) {
        editor.replace_url.run(format!("/{}/{id}", family.route()));
    }
}

async fn save_draft(api: Api, token: String, family: Family, editor: Editor) -> bool {
    let i18n = ruphish_ui::localization::use_i18n();

    if editor.readonly
        || editor.busy.get_untracked()
        || editor.visual_pending.get_untracked()
        || editor
            .import_review
            .is_some_and(|r| !r.with_untracked(ImportReview::ready))
    {
        return false;
    }
    editor.busy.set(true);
    editor.message.clear();
    let current = editor.draft.get_untracked();
    let submitted = current.value().clone();
    let id = submitted["id"].as_i64().unwrap_or(0);
    let path = if id == 0 {
        format!("/api/{}/", family.route())
    } else {
        format!("/api/{}/{id}", family.route())
    };
    let result = api
        .resource(
            if id == 0 { "POST" } else { "PUT" },
            &path,
            &token,
            Some(&submitted),
            current.revision(),
        )
        .await;
    editor.busy.set(false);
    match result {
        Ok((value, revision)) => {
            editor
                .draft
                .update(|d| d.saved(&submitted, value, revision));
            editor.epoch.update(|v| *v += 1);
            editor.message.status(
                if editor.draft.with_untracked(ResourceDraft::dirty) {
                    leptos_i18n::t_string!(
                        i18n,
                        ui.saved_the_submitted_version_newer_changes_remain_unsaved
                    )
                } else {
                    leptos_i18n::t_string!(i18n, ui.changes_saved)
                }
                .into(),
            );
            true
        }
        Err(e) => {
            editor.message.error(e);
            false
        }
    }
}
fn group_fields(editor: Editor) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    view! {<section class="resource-section"><div class="management-heading"><h2>{tr!(ui.recipients)}</h2><ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["targets"].as_array_mut(){rows.push(json!({"email":"","first_name":"","last_name":"","position":""}));}}))>{tr!(ui.add_recipient)}</ActionButton></div><DataTable label=trs!(ui.recipients) headers=vec![trs!(ui.email),trs!(ui.first_name),trs!(ui.last_name),trs!(ui.position),trs!(ui.action)] class="recipient-table">
    <For each=move||0..editor.draft.with(|d|d.value()["targets"].as_array().map_or(0,Vec::len)) key=|i|*i children=move|index|view!{<tr>{["email","first_name","last_name","position"].into_iter().map(move|key|view!{<td><TextInput aria_label=leptos_i18n::t_string!(i18n, messages.recipient_arg0_arg1, arg0 = (index+1).to_string(), arg1 = match key { "email" => leptos_i18n::t_string!(i18n, ui.email), "first_name" => leptos_i18n::t_string!(i18n, ui.first_name), "last_name" => leptos_i18n::t_string!(i18n, ui.last_name), _ => leptos_i18n::t_string!(i18n, ui.position) }) value=Signal::derive(move||editor.draft.with(|d|d.value()["targets"][index][key].as_str().unwrap_or("").to_owned())) on_input=move|e|editor.draft.update(|d|d.edit(|v|v["targets"][index][key]=json!(event_target_value(&e))))/></td>}).collect_view()}<td><ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["targets"].as_array_mut() && index<rows.len(){rows.remove(index);}}))>{tr!(ui.remove)}</ActionButton></td></tr>}/>
    </DataTable></section>}
}
fn content_workspace(
    api: Api,
    token: StoredValue<String>,
    editor: Editor,
    family: Family,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    use ruphish_ui::preview_state::ContentPreview;
    let selected_tab = RwSignal::new(0usize);
    let showing_preview = Signal::derive(move || selected_tab.get() == 1);
    let preview = RwSignal::new(ContentPreview::default());
    let html_source = Memo::new(move |_| editor.text("html"));
    let retry = RwSignal::new(0u64);
    Effect::new(move |_| {
        let visible = showing_preview.get();
        let source = html_source.get();
        retry.get();
        if !visible {
            preview.update(ContentPreview::clear);
            return;
        }
        let Some(generation) = preview.try_update(|p| p.begin(source.clone())) else {
            return;
        };
        spawn_local(async move {
            let body = json!({"html":source,"recipient":{"email":"alice@example.test","first_name":"Alice","last_name":"Example","position":"Analyst"}});
            let result = api
                .resource(
                    "POST",
                    "/api/util/preview",
                    &token.get_value(),
                    Some(&body),
                    None,
                )
                .await
                .and_then(|(value, _)| {
                    value["html"].as_str().map(str::to_owned).ok_or_else(|| {
                        leptos_i18n::t_string!(i18n, ui.the_preview_response_did_not_contain_html)
                            .into()
                    })
                });
            if showing_preview.get_untracked() && html_source.get_untracked() == source {
                preview.update(|p| p.finish(generation, &source, result));
            }
        });
    });
    view! {<section class="content-workspace">
        <div class="content-heading"><h2>{move || if family==Family::Templates {leptos_i18n::t_string!(i18n, ui.email_html)} else {leptos_i18n::t_string!(i18n, ui.landing_page_html)}}</h2>
        <Tabs label=trs!(ui.html_editing_and_preview) selected=selected_tab tabs=vec![
            Tab { id: "content-edit", label: trs!(ui.edit), disabled: Signal::derive(||false) },
            Tab { id: "content-preview", label: trs!(ui.preview), disabled: editor.visual_pending.into() },
        ]/></div>
        <p class="content-tab-help" hidden=move||!editor.visual_pending.get()>{tr!(ui.visual_editor_not_ready)}</p>
        <TabPanel id="content-edit" selected=Signal::derive(move||!showing_preview.get())>
            {crate::editor::content_editor(Signal::derive(move||editor.text("html")),Callback::new(move|value|editor.set("html",json!(value))),editor.epoch,editor.message,editor.visual_pending,editor.readonly,family==Family::Pages)}
        </TabPanel>
        <TabPanel id="content-preview" class="content-preview-panel" selected=showing_preview busy=Signal::derive(move||preview.with(|p|p.loading))>
            <p class="content-tab-help">{tr!(ui.current_draft_with_sample_recipient_alice_example_scripts_external_resources_and)}{(family==Family::Templates).then_some(leptos_i18n::t_string!(i18n, ui.email_clients_may_render_it_differently))}</p>
            <Show when=move||preview.with(|p|p.loading)><p role="status">{tr!(ui.rendering_preview)}</p></Show>
            {move||preview.get().error.map(|error|view!{<p class="form-message error" role="alert">{error}</p><ActionButton kind="button" on_click=move |_|retry.update(|v|*v+=1)>{tr!(ui.retry_preview)}</ActionButton>})}
            {move||preview.get().html.map(|html|view!{<iframe title=trs!(ui.current_html_draft_preview) sandbox="" referrerpolicy="no-referrer" srcdoc=preview_document(&html)></iframe>})}
        </TabPanel>
    </section>}
}
fn content_fields(
    api: Api,
    token: StoredValue<String>,
    editor: Editor,
    family: Family,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let files = NodeRef::<html::Input>::new();
    let add = move |event| {
        let input = event_target::<web_sys::HtmlInputElement>(&event);
        let Some(files) = input.files() else {
            return;
        };
        let epoch = editor.epoch.get_untracked();
        spawn_local(async move {
            for index in 0..files.length() {
                if let Some(file) = files.get(index) {
                    if file.size() > 16.0 * 1024.0 * 1024.0 {
                        editor.message.error({
                            leptos_i18n::t_string!(i18n, ui.each_attachment_must_be_at_most_16_mib)
                                .to_owned()
                        });
                        continue;
                    }
                    match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                        Ok(buffer) => {
                            if editor.epoch.get_untracked() != epoch {
                                return;
                            }
                            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                            let mime = if file.type_().is_empty() {
                                "application/octet-stream".into()
                            } else {
                                file.type_()
                            };
                            editor.draft.update(|d| d.attach(file.name(), mime, &bytes));
                        }
                        Err(_) => editor.message.error(
                            leptos_i18n::t_string!(i18n, ui.could_not_read_attachment).to_owned(),
                        ),
                    }
                }
            }
            input.set_value("");
        });
    };
    view! {<section class="resource-section">
    {(family==Family::Templates).then(||view!{<div class="resource-columns">{editor.field("subject", trs!(ui.subject), "text")}{editor.field("envelope_sender", trs!(ui.visible_from_blank_uses_sending_profile), "text")}</div>})}
    {content_workspace(api,token,editor,family)}
    {(family==Family::Templates).then(||view!{<label class="text-alternative">{tr!(ui.plain_text_alternative)}<Textarea rows=6 value=Signal::derive(move||editor.text("text")) on_input=move|e|editor.set("text",json!(event_target_value(&e)))/></label><div class="resource-section"><h2>{tr!(ui.attachments)}</h2><input node_ref=files hidden type="file" multiple on:change=add/><ActionButton kind="button" on_click=move |_|{if let Some(node)=files.get(){node.click();}}>{tr!(ui.add_attachments)}</ActionButton><ul>{move||editor.draft.with(|d|d.value()["attachments"].as_array().cloned().unwrap_or_default()).into_iter().enumerate().map(|(i,a)|view!{<li>{a["name"].as_str().unwrap_or("").to_owned()}<ActionButton kind="button" on_click=move |_|editor.draft.update(|d|{d.remove_attachment(i);})>{tr!(ui.remove)}</ActionButton></li>}).collect_view()}</ul></div>})}
    {(family==Family::Pages).then(||view!{<section class="resource-section"><h2>{tr!(ui.optional_campaign_defaults)}</h2><p>{tr!(ui.public_pages_preserve_field_names_and_styling_but_block_authored_scripts_embedde)}</p><p>{tr!(ui.suggested_submission_settings_for_campaigns_using_this_page_leave_tracking_off_a)}</p>{editor.flag("capture_credentials", trs!(ui.record_form_submission))}{editor.flag("capture_passwords", trs!(ui.record_password_presence))}{editor.field("redirect_url", trs!(ui.redirect_url_after_submission_optional), "text")}<p>{tr!(ui.track_only_whether_fields_were_filled_and_submitted_entered_usernames_passwords)}</p></section>})}
    </section>}
}
fn profile_fields(editor: Editor) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    view! {<section class="resource-section"><h2>{tr!(ui.connection)}</h2><div class="resource-columns">{editor.field("host", trs!(ui.smtp_host_and_port), "text")}{editor.field("from_address", trs!(ui.sender_address), "text")}{editor.field("username", trs!(ui.username_optional), "text")}{editor.field("password", trs!(ui.password), "password")}</div><p>{tr!(ui.port_defaults_to_25_port_465_uses_implicit_tls_starttls_is_used_when_advertised)}</p>{editor.flag("ignore_cert_errors", trs!(ui.ignore_smtp_certificate_errors))}</section><section class="resource-section"><div class="management-heading"><h2>{tr!(ui.additional_headers)}</h2><ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["headers"].as_array_mut(){rows.push(json!({"key":"","value":""}));}}))>{tr!(ui.add_header)}</ActionButton></div><For each=move||0..editor.draft.with(|d|d.value()["headers"].as_array().map_or(0,Vec::len)) key=|i|*i children=move|index|view!{<div class="header-row">{["key","value"].into_iter().map(move|key|view!{<label>{move || if key=="key"{leptos_i18n::t_string!(i18n, ui.header_name)}else{leptos_i18n::t_string!(i18n, ui.value)}}<TextInput value=Signal::derive(move||editor.draft.with(|d|d.value()["headers"][index][key].as_str().unwrap_or("").to_owned())) on_input=move|e|editor.draft.update(|d|d.edit(|v|v["headers"][index][key]=json!(event_target_value(&e))))/></label>}).collect_view()}<ActionButton kind="button" on_click=move |_|editor.draft.update(|d|d.edit(|v|{if let Some(rows)=v["headers"].as_array_mut() && index<rows.len(){rows.remove(index);}}))>{tr!(ui.remove)}</ActionButton></div>}/></section>}
}

fn import_panel(api: Api, token: String, family: Family, open: RwSignal<bool>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(token);
    let content = RwSignal::new(String::new());
    let mode = RwSignal::new("json".to_owned());
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let file = NodeRef::<html::Input>::new();
    let convert = RwSignal::new(false);
    let columns = RwSignal::new([
        "Email".to_owned(),
        "First Name".to_owned(),
        "Last Name".to_owned(),
        "Position".to_owned(),
    ]);
    let drafts = use_context::<ImportDrafts>();
    let navigate = leptos_router::hooks::use_navigate();
    // Scoped async work is canceled when the dialog or authenticated shell disappears.
    crate::navigation::protect_draft(move || !content.get().is_empty() || busy.get());
    let read = move |event| {
        let input = event_target::<web_sys::HtmlInputElement>(&event);
        let Some(file) = input.files().and_then(|f| f.get(0)) else {
            return;
        };
        input.set_value("");
        message.clear();
        if file.size() > 8.0 * 1024.0 * 1024.0 {
            message.error(ruphish_ui::localization::import_error(
                i18n,
                &ImportError::TooLarge,
            ));
            return;
        }
        let extension = file
            .name()
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let kind = match extension.as_str() {
            "json" => "json",
            "csv" if family == Family::Groups => "csv",
            "eml" if family == Family::Templates => "email",
            "html" | "htm" if matches!(family, Family::Templates | Family::Pages) => "html",
            _ => {
                message.error(ruphish_ui::localization::import_error(
                    i18n,
                    &ImportError::UnsupportedFormat,
                ));
                return;
            }
        };
        busy.set(true);
        spawn_local(async move {
            match wasm_bindgen_futures::JsFuture::from(file.text()).await {
                Ok(value) => {
                    content.set(value.as_string().unwrap_or_default());
                    mode.set(kind.into());
                }
                Err(_) => message.error(ruphish_ui::localization::import_error(
                    i18n,
                    &ImportError::ReadFailed,
                )),
            }
            busy.set(false);
        });
    };
    let review = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        let source = content.get_untracked();
        let kind = mode.get_untracked();
        let columns = columns.get_untracked();
        let convert = convert.get_untracked();
        let navigate = navigate.clone();
        spawn_local(async move {
            let result = if source.len() > 8 * 1024 * 1024 {
                Err(ImportError::TooLarge)
            } else if kind == "json" {
                ruphish_ui::import_state::parse_json(&source)
            } else if kind == "html" {
                Ok(json!({"html":source}))
            } else {
                let (path, payload) = match kind.as_str() {
                    "csv" => (
                        "/api/import/group/review",
                        json!({"content":source,"columns":columns}),
                    ),
                    "email" => (
                        "/api/import/email",
                        json!({"content":source,"convert_links":convert}),
                    ),
                    _ => ("/api/import/site", json!({"url":source})),
                };
                api.resource("POST", path, &token.get_value(), Some(&payload), None)
                    .await
                    .map(|(v, _)| v)
                    .map_err(ImportError::RequestFailed)
            }
            .and_then(|value| {
                ruphish_ui::import_state::prepare_creation(family.route(), &kind, value)
            });
            match result {
                Ok(imported) => {
                    let mut draft = ResourceDraft::new(family.empty(), None);
                    draft.merge_import(&imported.fields);
                    let result = drafts
                        .ok_or(ImportError::DraftUnavailable)
                        .and_then(|store| store.insert(family.route(), draft, imported.review));
                    match result {
                        Ok(id) => {
                            content.set(String::new());
                            busy.set(false);
                            open.set(false);
                            navigate(
                                &format!("/{}/new?draft={id}", family.route()),
                                Default::default(),
                            );
                            return;
                        }
                        Err(error) => {
                            message.error(ruphish_ui::localization::import_error(i18n, &error))
                        }
                    }
                }
                Err(error) => message.error(ruphish_ui::localization::import_error(i18n, &error)),
            }
            busy.set(false);
        });
    };
    view! {<Dialog title=leptos_i18n::t_string!(i18n, messages.import_arg0, arg0 = (family.singular()).to_string()) on_cancel=Callback::new(move |_|open.set(false))>
        <div class="management-heading"><h2 id="import-heading">{leptos_i18n::t!(i18n, messages.import_arg0, arg0 = (family.singular()).to_string())}</h2></div>
        <p>{tr!(ui.load_content_into_a_new_create_form_to_review_and_finish_nothing_is_saved_yet)}</p>
        <fieldset disabled=move||busy.get() class="import-fields">
        <input hidden node_ref=file type="file" accept=match family{Family::Groups=>".json,.csv",Family::Templates=>".json,.eml,.html,.htm",Family::Pages=>".json,.html,.htm",Family::Profiles=>".json"} on:change=read/>
        <ActionButton on_click=move |_|{if let Some(file)=file.get(){file.click();}}>{tr!(ui.choose_file)}</ActionButton>
        <Select label=trs!(ui.import_format) value=mode on_change=Callback::new(move|v:String|{mode.set(v);content.set(String::new());message.clear();})>
        <option value="json">{tr!(ui.resource_json)}</option>
        {(family==Family::Groups).then(||view!{<option value="csv">{tr!(ui.recipients_csv)}</option>})}
        {(family==Family::Templates).then(||view!{<option value="email">{tr!(ui.email_eml)}</option>})}
        {matches!(family,Family::Templates|Family::Pages).then(||view!{<option value="html">{tr!(ui.html_document)}</option>})}
        {(family==Family::Pages).then(||view!{<option value="site">{tr!(ui.website_url)}</option>})}
        </Select>
        <label>{move||if mode.get()=="site"{leptos_i18n::t_string!(i18n, ui.website_url)}else{leptos_i18n::t_string!(i18n, ui.file_content_or_pasted_content)}}<Textarea rows=7 value=Signal::derive(move||content.get()) on_input=move|e|content.set(event_target_value(&e))/></label>
        <Show when=move||mode.get()=="csv"><div class="resource-columns">{[trs!(ui.email_column),trs!(ui.first_name_column),trs!(ui.last_name_column),trs!(ui.position_column)].into_iter().enumerate().map(|(i,label)|view!{<label>{label}<TextInput value=Signal::derive(move||columns.get()[i].clone()) on_input=move|e|columns.update(|v|v[i]=event_target_value(&e))/></label>}).collect_view()}</div></Show>
        <Show when=move||mode.get()=="email"><Checkbox label=trs!(ui.replace_links_with_the_recipient_url) checked=convert on_toggle=Callback::new(move|v|convert.set(v))/></Show>
        </fieldset>{message.view()}
        <div class="import-actions"><ActionButton style=ButtonStyle::Ghost on_click=move |_|open.set(false)>{tr!(ui.cancel_import)}</ActionButton><ActionButton style=ButtonStyle::Primary disabled=Signal::derive(move||busy.get()||content.get().trim().is_empty()) on_click=review>{move||if busy.get(){leptos_i18n::t_string!(i18n, ui.reading_import)}else{leptos_i18n::t_string!(i18n, ui.continue_to_create)}}</ActionButton></div>
    </Dialog>}
}

fn import_review_panel(review: RwSignal<ImportReview>, message: Message) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    view! {<section class="import-summary" aria-labelledby="import-summary-heading"><h2 id="import-summary-heading">{tr!(ui.review_imported_content)}</h2>
    <ul>{review.with_untracked(|r|r.notes.clone()).into_iter().map(|note|view!{<li>{move || ruphish_ui::localization::import_note(i18n, &note)}</li>}).collect_view()}</ul>
    {review.with_untracked(|r|!r.rejected_rows.is_empty()).then(||view!{<details><summary class="text-danger">{tr!(ui.rejected_csv_rows)}</summary><ul class="text-danger">{review.with_untracked(|r|r.rejected_rows.iter().take(20).cloned().collect::<Vec<_>>()).into_iter().map(|error|view!{<li>{leptos_i18n::t!(i18n, messages.row_arg0_arg1, arg0 = (error["row"]).to_string(), arg1 = ruphish_ui::localization::failure_label(i18n, error["code"].as_str().unwrap_or("CSV_INVALID")))}</li>}).collect_view()}</ul><p>{tr!(ui.showing_up_to_20_rows_download_the_report_for_all_rejected_rows)}</p><ActionButton on_click=move |_|download(&json!(review.with_untracked(|r|r.rejected_rows.clone())),"import-errors",message)>{tr!(ui.download_error_report)}</ActionButton></details>})}
    {review.with_untracked(|r|r.requires_acknowledgement).then(||view!{<Checkbox label=trs!(ui.i_reviewed_the_rejected_or_duplicate_rows_and_accept_these_omissions) checked=Signal::derive(move||review.with(|r|r.acknowledged)) on_toggle=Callback::new(move|v|review.update(|r|r.acknowledged=v))/>})}
    <p>{tr!(ui.this_draft_is_kept_only_in_this_tab_refreshing_or_closing_it_clears_unsaved_cont)}</p>
    </section>}
}

fn test_panel(
    api: Api,
    token: String,
    family: Family,
    editor: Editor,
    open: RwSignal<bool>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let token = StoredValue::new(token);
    let profiles = RwSignal::new(Vec::<Value>::new());
    let pages = RwSignal::new(Vec::<Value>::new());
    let selected = RwSignal::new(String::new());
    let page = RwSignal::new(String::new());
    let recipient = RwSignal::new(String::new());
    let first = RwSignal::new("Alice".to_owned());
    let last = RwSignal::new("Example".to_owned());
    let position = RwSignal::new("Analyst".to_owned());
    let url = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    spawn_local(async move {
        for (kind, signal) in [("smtp", profiles), ("pages", pages)] {
            match api.get(&format!("/api/{kind}/summary")).await {
                Ok(v) => signal.set(v["items"].as_array().cloned().unwrap_or_default()),
                Err(e) => message.error(e),
            }
        }
    });
    let close = move || {
        open.set(false);
        finish_edit(editor, family);
    };
    let send = move |connection: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        message.clear();
        spawn_local(async move {
            if !connection
                && (editor.draft.with_untracked(ResourceDraft::dirty)
                    || editor
                        .draft
                        .with_untracked(|d| d.value()["id"].as_i64().unwrap_or(0) == 0))
                && !save_draft(api, token.get_value(), family, editor).await
            {
                message.error({
                    leptos_i18n::t_string!(
                        i18n,
                        ui.the_resource_could_not_be_saved_no_test_email_was_sent
                    )
                    .to_owned()
                });
                busy.set(false);
                return;
            }
            let smtp = if family == Family::Profiles {
                editor.draft.get_untracked().value().clone()
            } else {
                json!({"name":selected.get_untracked()})
            };
            let (path, payload) = if connection {
                ("/api/util/test_connection", smtp)
            } else {
                (
                    "/api/util/send_test_email",
                    json!({"smtp":smtp,"template":{"name":if family==Family::Templates{editor.text("name")}else{String::new()}},"page":{"name":page.get_untracked()},"url":url.get_untracked(),"email":recipient.get_untracked(),"first_name":first.get_untracked(),"last_name":last.get_untracked(),"position":position.get_untracked()}),
                )
            };
            match api
                .resource("POST", path, &token.get_value(), Some(&payload), None)
                .await
            {
                Ok(_) => message.status(
                    if connection {
                        leptos_i18n::t_string!(
                            i18n,
                            ui.connection_and_authentication_verified_no_email_sent
                        )
                    } else {
                        leptos_i18n::t_string!(
                            i18n,
                            ui.accepted_by_smtp_server_inbox_delivery_is_not_confirmed
                        )
                    }
                    .into(),
                ),
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    };
    view! {<Dialog title=trs!(ui.send_a_test_email) busy=busy on_cancel=Callback::new(move |_|close())><div class="management-heading"><h2 id="test-heading">{tr!(ui.send_a_test_email)}</h2><ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|close()>{tr!(ui.close)}</ActionButton></div><p>{tr!(ui.test_the_selected_saved_content_with_one_recipient_saving_and_delivery_are_separ)}</p>
    {(family==Family::Templates).then(||view!{<Select label=trs!(ui.sending_profile) value=selected on_change=Callback::new(move|v|selected.set(v))><option value="">{tr!(ui.choose_a_profile)}</option>{move||profiles.get().into_iter().map(|v|{let name=v["name"].as_str().unwrap_or("").to_owned();{let label=name.clone();view!{<option value=name>{label}</option>}}}).collect_view()}</Select>})}
    <ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|send(true)>{tr!(ui.test_connection_only)}</ActionButton><div class="resource-columns">
    {[ (recipient,trs!(ui.recipient_email)),(first,trs!(ui.first_name)),(last,trs!(ui.last_name)),(position,trs!(ui.position)),(url,trs!(ui.public_test_url_optional)) ].into_iter().map(|(signal,label)|view!{<label>{label}<TextInput value=Signal::derive(move||signal.get()) on_input=move|e|signal.set(event_target_value(&e))/></label>}).collect_view()}
    <Select label=trs!(ui.preview_landing_page) value=page on_change=Callback::new(move|v|page.set(v))><option value="">{tr!(ui.no_landing_page)}</option>{move||pages.get().into_iter().map(|v|{let name=v["name"].as_str().unwrap_or("").to_owned();{let label=name.clone();view!{<option value=name>{label}</option>}}}).collect_view()}</Select></div>
    {message.view()}<ActionButton style=ButtonStyle::Primary disabled=Signal::derive(move||busy.get()||recipient.get().is_empty()) on_click=move |_|send(false)>{move||if busy.get(){leptos_i18n::t_string!(i18n, ui.working)}else if editor.draft.with(ResourceDraft::dirty)||editor.draft.with(|d|d.value()["id"].as_i64().unwrap_or(0)==0){leptos_i18n::t_string!(i18n, ui.save_and_send_test)}else{leptos_i18n::t_string!(i18n, ui.send_test_email)}}</ActionButton>
    </Dialog>}
}
